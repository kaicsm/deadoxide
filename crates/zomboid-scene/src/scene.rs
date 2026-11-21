//! Window-free scene: turns real cells into sprite instances (world-pixel rects + atlas UVs) and a collision grid.
use crate::sim::{
    Collision, BLOCK, EDGE_N, EDGE_W, HOPPABLE, SOLID, SOLID_TRANS, WATER, WINDOW, WINDOW_N,
    WINDOW_W,
};
use anyhow::{anyhow, Result};
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use zomboid_engine::zombie::iso::streaming_window::ChunkWindow;
use std::path::Path;
use zomboid_content::atlas::{build_atlas_from, build_atlas_and_catalog_from, pack_order, Atlas, AtlasDelta, AtlasPatch, TextureCatalog};
#[cfg(test)]
use zomboid_content::tiledefs::TileDefs;
use zomboid_engine::java;
use zomboid_engine::vfs;
use zomboid_engine::zombie::core::random::rand_abstract::RandAbstract;
use zomboid_engine::zombie::inventory::inventory_item_factory::InventoryItemFactory;
use zomboid_engine::zombie::iso::cell_loader::{self, LoadContext};
use zomboid_engine::zombie::iso::chunk_save::SaveLoadContext;
use zomboid_engine::zombie::iso::iso_chunk::IsoChunk;
use zomboid_engine::zombie::iso::iso_grid_square::{
    line_clear, GetSquare, IsoGridSquare, TestResults,
};
use zomboid_engine::zombie::iso::iso_meta_grid::AlarmSettings;
use zomboid_engine::zombie::iso::iso_object::ObjectKind;
use zomboid_engine::zombie::iso::iso_world::IsoWorld;
use zomboid_engine::zombie::iso::iso_meta_grid::IsoMetaGrid;
use zomboid_engine::zombie::iso::zones::zone::Zone;
use zomboid_engine::zombie::iso::zones::zone_geometry_type::ZoneGeometryType;
use zomboid_engine::zombie::iso::worldgen::zones::world_gen_zone::WorldGenZone;
use zomboid_engine::zombie::iso::sprite::iso_sprite_manager::SpriteRef;
use zomboid_engine::zombie::iso::sprite_details::iso_flag_type::IsoFlagType;
use zomboid_engine::zombie::iso::sprite_details::iso_object_type::IsoObjectType;
use zomboid_engine::zombie::iso::worldgen::world_gen_params::WorldGenParams;
use zomboid_engine::zombie::iso::worldgen::world_gen_chunk::WorldGenCell;
use zomboid_engine::zombie::iso::worldgen::runtime::WorldGenRuntime;
use zomboid_engine::zombie::radio::media::recorded_media::RecordedMedia;
use zomboid_engine::zombie::sandbox_options::SandboxOptions;
use zomboid_engine::zombie::scripting::script_manager::ScriptManager;
use zomboid_engine::zombie::world::dictionary_data::DictionaryData;

/// Instance layout matches the shader: rect (x,y,w,h in world px), uv (u0,v0,u1,v1), atlas layer.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Instance {
    pub rect: [f32; 4],
    pub uv: [f32; 4],
    pub layer: f32,
}

pub struct Scene {
    pub atlas: Atlas,
    pub instances: Vec<Instance>,
    /// instance index where each level starts (len = levels + 1)
    pub level_ranges: Vec<u32>,
    /// World z corresponding to the first entry in level_ranges.
    pub min_level: i32,
    pub unresolved: u64,
    pub collision: Collision,
    /// per instance: (gx, gy, z, room_id), parallel to `instances` (used by `retain_room`)
    pub meta: Vec<(i32, i32, i32, i32)>,
    /// room id of every square that has one (room_id >= 0)
    pub rooms: std::collections::HashMap<(i32, i32, i32), i32>,
    pub clear: [f32; 3],
    /// Present for scenes built by `load_region`.
    pub state: Option<SceneState>,
    /// Per-chunk presentation cache, maintained like IsoChunk.doLoadGridsquare.
    presentations: HashMap<(i32, i32), ChunkPresentation>,
    /// Only these chunks can differ after the last completed WorldStreamer job.
    stream_touched: HashSet<(i32, i32)>,
    /// Append-only atlas changes, transferred separately from IsoChunk objects.
    pending_atlas_delta: AtlasDelta,
    /// Pending wgpu uploads; texture extent changes require a full rebind.
    pub gpu_atlas_patches: Vec<AtlasPatch>,
    pub gpu_atlas_full: bool,
}

pub const TILE_W: i32 = 128;
pub const TILE_H: i32 = 64;
pub const LEVEL_PX: i32 = 192;

/// zombie.iso.IsoGridSquare.CalculateCollide: collect the measured static square flags.
/// Doors, windows, stairs and dynamic objects still need their own collision rules.
fn collision_flags(has: impl Fn(&str) -> bool) -> u16 {
    let mut flags = 0;
    if has("solid") || has("solidtrans") {
        flags |= BLOCK;
    }
    if has("solid") {
        flags |= SOLID;
    }
    if has("solidtrans") {
        flags |= SOLID_TRANS;
    }
    if has("water") {
        flags |= WATER;
    }
    if ["windowN", "windowW", "WindowN", "WindowW"]
        .iter()
        .any(|prop| has(prop))
    {
        flags |= WINDOW;
    }
    if has("windowN") || has("WindowN") {
        flags |= WINDOW_N;
    }
    if has("windowW") || has("WindowW") {
        flags |= WINDOW_W;
    }
    if has("HoppableN") || has("HoppableW") {
        flags |= HOPPABLE;
    }
    if has("collideN") {
        flags |= EDGE_N;
    }
    if has("collideW") {
        flags |= EDGE_W;
    }
    flags
}

/// The same mapping over raw `.tiles.txt` properties (the synthetic oracle tests use it).
#[cfg(test)]
fn tile_collision_flags(defs: &TileDefs, name: &str) -> u16 {
    collision_flags(|prop| defs.has(name, prop))
}

/// Isometric world-pixel position of the top vertex of tile (gx, gy) on level z.
pub fn iso(gx: f32, gy: f32, z: i32) -> [f32; 2] {
    [
        (gx - gy) * (TILE_W as f32 / 2.0),
        (gx + gy) * (TILE_H as f32 / 2.0) - (z * LEVEL_PX) as f32,
    ]
}

impl Scene {
    /// Instance for a solid marker (the reserved `__player` atlas block) standing at tile (x, y).
    pub fn marker(&self, name: &str, x: f32, y: f32, z: i32) -> Instance {
        let s = self.atlas.sprites[name];
        let (aw, ah) = (self.atlas.width as f32, self.atlas.height as f32);
        let p = iso(x, y, z);
        let (w, h) = (24.0, 96.0);
        Instance {
            rect: [p[0] - w / 2.0, p[1] - h, w, h], // iso() of a fractional position is already the tile centre = feet
            uv: [
                (s.ax as f32 + 1.0) / aw,
                (s.ay as f32 + 1.0) / ah,
                (s.ax as f32 + 7.0) / aw,
                (s.ay as f32 + 7.0) / ah,
            ],
            layer: s.layer as f32,
        }
    }
}

/// Collision flags of one object's sprite, read from the properties the ported tile-definition loader derived
/// (`IsoWorld.LoadTileDefinitions`), e.g. `WallN` implies `collideN` there.
fn sprite_collision_flags(world: &IsoWorld, spr: SpriteRef) -> u16 {
    let p = &world.sprites.sprite(spr).properties;
    collision_flags(|name| match IsoFlagType::from_string(name) {
        IsoFlagType::MAX => p.has_str(&world.alias, name),
        f => p.has(f) || p.has_str(&world.alias, name),
    })
}

/// zombie.iso.IsoObject.overlaySprite: so far only objects loaded from a save carry one (loot spawning, which sets it
/// on new chunks, is not ported). Its `overlaySpriteColor` tint is not applied.
fn overlay_sprite(o: &zomboid_engine::zombie::iso::iso_object::IsoObject) -> Option<SpriteRef> {
    o.save.as_ref().and_then(|s| s.overlay_sprite)
}

/// World seed used until the client reads `map_worldgen.bin` of a save or creates worlds itself: the Java hash of
/// the oracle's seed text "DeadOxideOracle", so basements match the reference server world.
const WORLD_SEED: i64 = -1594728997;

struct RegionGrid<'a>(std::collections::HashMap<(i32, i32, i32), &'a IsoGridSquare>);
impl GetSquare for RegionGrid<'_> {
    fn get_grid_square(&self, x: i32, y: i32, z: i32) -> Option<&IsoGridSquare> {
        self.0.get(&(x, y, z)).copied()
    }
}

/// Load cells `x0..=x1` x `y0..=y1` (cell coordinates) into one scene in global painter's order.
/// Squares and objects come from the direct port (`zomboid-engine`): `CellLoader.LoadCellBinaryChunk` per chunk, so
/// object order, overlays, open curtains, removed trees and table stacking are the original's. `Rand` is seeded with a
/// fixed value here; random fields (locks, tints) are therefore the same on every run and tints are not drawn yet.
pub fn load_region(root: &Path, x0: i32, y0: i32, x1: i32, y1: i32) -> Result<Scene> {
    load_region_from_save(root, None, x0, y0, x1, y1)
}

/// What chunk loading reads from a save folder (`Saves/<mode>/<name>`).
struct SaveFiles {
    dir: std::path::PathBuf,
    dictionary: DictionaryData,
    scripts: ScriptManager,
    recorded_media: RecordedMedia,
    recorded_media_lua: String,
}

impl SaveFiles {
    fn open(root: &Path, dir: &Path) -> Result<Self> {
        let read =
            |p: std::path::PathBuf| vfs::read(&p).map_err(|e| anyhow!("{}: {e}", p.display()));
        let dictionary =
            DictionaryData::load_from_byte_buffer(&read(dir.join("WorldDictionary.bin"))?)
                .map_err(|e| anyhow!("WorldDictionary.bin: {e}"))?;
        // A save without recorded_media.bin: RecordedMedia.load logs and keeps an empty table.
        let recorded_media = match read(dir.join("recorded_media.bin")) {
            Ok(b) => RecordedMedia::load(&b).map_err(|e| anyhow!("recorded_media.bin: {e}"))?,
            Err(e) => {
                log::warn!("{e}");
                RecordedMedia::default()
            }
        };
        let lua = read(root.join("media/lua/shared/RecordedMedia/recorded_media.lua"))?;
        Ok(SaveFiles {
            dir: dir.to_path_buf(),
            dictionary,
            scripts: ScriptManager::load(root).map_err(|e| anyhow!(e))?,
            recorded_media,
            recorded_media_lua: String::from_utf8_lossy(&lua).into_owned(),
        })
    }

    /// zombie.iso.worldgen.WorldGenParams.load: the save's world seed (`None` without `map_worldgen.bin`).
    fn world_seed(dir: &Path) -> Option<i64> {
        let data = vfs::read(&dir.join("map_worldgen.bin")).ok()?;
        let (params, err) = WorldGenParams::load(&data);
        if let Some(e) = err {
            log::warn!("map_worldgen.bin: {e}");
        }
        Some(params.get_seed())
    }
}

/// IsoMetaGrid.registerWorldGenZone: preserve WorldGen-only Rocks and shape from
/// registered map objects. The original uses the first containing zone in registration order.
/// The base map's objects.lua is loaded in map-folder priority order.
fn load_worldgen_zones(root: &Path, map_name: &str) -> Result<Vec<WorldGenZone>> {
    let mut zones = Vec::new();
    for name in IsoMetaGrid::lot_directories(&root.join("media"), map_name) {
        let path = root.join("media/maps").join(name).join("objects.lua");
        let bytes = match vfs::read(&path) {
            Ok(b) => b,
            Err(e) if !path.exists() => { log::debug!("no WorldGen zone source {}: {e}", path.display()); continue; }
            Err(e) => return Err(anyhow!("{}: {e}", path.display())),
        };
        for z in zomboid_content::zones::parse_worldgen_zones(&bytes)
            .map_err(|e| anyhow!("{}: {e}", path.display()))? {
            let mut shape = Zone::new(&z.name, "WorldGen", z.x, z.y, z.z, z.w, z.h);
            shape.geometry_type = match z.geometry.as_str() {
                "point" => ZoneGeometryType::Point,
                "polyline" => ZoneGeometryType::Polyline,
                "polygon" => ZoneGeometryType::Polygon,
                _ => ZoneGeometryType::Invalid,
            };
            shape.points = z.points;
            shape.polyline_width = z.polyline_width;
            zones.push(WorldGenZone::new(shape, z.rocks));
        }
    }
    Ok(zones)
}

/// Stream an (odd-sided) chunk window around a tile position, following IsoChunkMap's
/// residency semantics. The window spans cell boundaries and never loads a full 32x32 cell.
/// Without --persist-worldgen the session is read-only on disk; the most recent 256 evicted
/// chunks are retained in memory to avoid immediate regeneration when retracing steps.
pub fn load_streaming(
    root: &Path, save: Option<&Path>, player_x: f32, player_y: f32,
    radius: i32, persist_worldgen: bool,
) -> Result<Scene> {
    if cfg!(target_arch = "wasm32") {
        return Err(anyhow!("native streaming requires disk-backed WorldGen (not wasm32)"));
    }
    if persist_worldgen && save.is_none() {
        return Err(anyhow!("stream persistence requires --save"));
    }
    let window = ChunkWindow::for_player(player_x, player_y, radius).map_err(|e| anyhow!(e))?;
    let (x0, y0, x1, y1) = window.cells();
    load_region_from_save_internal(root, save, x0, y0, x1, y1, persist_worldgen, Some(window))
}

/// Like `load_region`, but as the game opens a save: chunks that the save's `map/<wx>/<wy>.bin` holds are read from it
/// (zombie.iso.IsoChunk.LoadOrCreate → LoadFromDisk), the others run the 2x2
/// WorldGen `LoadBrandNew` pipeline on desktop. A chunk whose file fails to load
/// is regenerated from its map group as `LoadChunk` does after `Blam`. The
/// world seed for basements is the save's (`map_worldgen.bin`).
pub fn load_region_from_save(
    root: &Path,
    save: Option<&Path>,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
) -> Result<Scene> {
    load_region_from_save_internal(root, save, x0, y0, x1, y1, false, None)
}

/// Generate, save and load a region from an existing native save directory.
///
/// This is opt-in: the regular `load_region_from_save` never writes to a player's save.
/// Only newly generated chunks are written. The save must already have its dictionary,
/// sandbox and world metadata; unsupported object state aborts before any chunk is written.
pub fn load_region_from_save_persist(
    root: &Path,
    save: &Path,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
) -> Result<Scene> {
    load_region_from_save_internal(root, Some(save), x0, y0, x1, y1, true, None)
}

fn load_region_from_save_internal(
    root: &Path,
    save: Option<&Path>,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    persist_worldgen: bool,
    stream_window: Option<ChunkWindow>,
) -> Result<Scene> {
    let save = save.map(|d| SaveFiles::open(root, d)).transpose()?;
    if persist_worldgen && save.is_none() {
        return Err(anyhow!("persisting WorldGen requires an existing save directory"));
    }
    let factory = save.as_ref().map(|s| {
        InventoryItemFactory::new(
            &s.dictionary,
            &s.scripts,
            s.recorded_media.clone(),
            &s.recorded_media_lua,
        )
    });
    let world_seed = save
        .as_ref()
        .and_then(|s| SaveFiles::world_seed(&s.dir))
        .unwrap_or(WORLD_SEED);
    let mut rand = RandAbstract::new(java::Random::new(1));
    // zombie.SandboxOptions as its constructor leaves it: declared defaults, then the "Apocalypse" preset.
    let mut sandbox = SandboxOptions::new();
    if let Ok(b) = vfs::read(&root.join("media/lua/shared/Sandbox/Apocalypse.lua")) {
        sandbox.load_game_file(&String::from_utf8_lossy(&b));
    }
    let alarm = AlarmSettings {
        last_stand: false,
        power_grid_exists: true,
        alarm: sandbox.get_int("Alarm").unwrap_or(4),
        alarm_decay: sandbox.get_int("AlarmDecay").unwrap_or(2),
    };
    let mut world = IsoWorld::init(
        root,
        "Muldraugh, KY",
        &|_| true,
        &alarm,
        Some(world_seed),
        &mut rand,
    )
    .map_err(|e| anyhow!(e))?;
    for l in &world.log {
        log::warn!("{l}");
    }
    // IsoChunk.LoadBrandNew: initialise WorldGen once per world, not once per chunk.
    // The browser's VFS currently cannot supply BiomeMap's file-backed PNG loader.
    let mut worldgen = if cfg!(target_arch = "wasm32") {
        None
    } else {
        let mut params = WorldGenParams::default();
        params.seed = world_seed as i32;
        let bounds = (
            world.meta_grid.min_x, world.meta_grid.max_x,
            world.meta_grid.min_y, world.meta_grid.max_y,
        );
        Some(WorldGenRuntime::from_game(
            root, "Muldraugh, KY", params, &sandbox, bounds,
            &world.sprites, &world.alias,
        ).map_err(|e| anyhow!("initialising WorldGen: {e}"))?)
    };
    let IsoWorld {
        sprites,
        alias,
        tile_definitions,
        meta_grid,
        ..
    } = &mut world;
    let mut ctx = LoadContext {
        sprites,
        alias,
        glass_removed_window_sprite_map: &tile_definitions.glass_removed_window_sprite_map,
        smashed_window_sprite_map: &tile_definitions.smashed_window_sprite_map,
        meta_grid,
        rand: &mut rand,
        locked_houses: sandbox.get_int("LockedHouses").unwrap_or(6),
        missing_tiles: Vec::new(),
        lotpack_cache: None,
    };
    let mut special_zones = load_worldgen_zones(root, "Muldraugh, KY")?;
    let mut chunks = Vec::new();
    let mut loaded_from_save = 0usize;
    let mut generated_chunks = HashSet::new();
    let (mut zmin, mut zmax) = (i32::MAX, i32::MIN);
    for cx in x0..=x1 {
        for cy in y0..=y1 {
            if ctx.meta_grid.get_header(cx, cy).is_none() {
                continue;
            }
            for wx in cx * 32..cx * 32 + 32 {
                for wy in cy * 32..cy * 32 + 32 {
                    if stream_window.is_some_and(|w| !w.contains(wx, wy)) { continue; }
                    let from_save = save.as_ref().zip(factory.as_ref()).and_then(|(sv, f)| {
                        let path = sv
                            .dir
                            .join("map")
                            .join(wx.to_string())
                            .join(format!("{wy}.bin"));
                        let data = vfs::read(&path).ok()?;
                        let mut sctx = SaveLoadContext {
                            sprites: &mut *ctx.sprites,
                            alias: ctx.alias,
                            dictionary: &sv.dictionary,
                            items: f,
                            meta_grid: &mut *ctx.meta_grid,
                            rand: &mut *ctx.rand,
                            has_lot_header: true,
                        };
                        match IsoChunk::load_from_disk_or_buffer(&data, wx, wy, &mut sctx) {
                            Ok(ch) => Some(ch),
                            Err(e) => {
                                log::warn!(
                                    "chunk {wx},{wy} from the save: {e}; created from the map"
                                );
                                None
                            }
                        }
                    });
                    let mut ch = match from_save {
                        Some(ch) => {
                            loaded_from_save += 1;
                            if let Some(runtime) = worldgen.as_mut() {
                                runtime.register_saved_chunk(&ch);
                            }
                            ch
                        }
                        None => {
                            generated_chunks.insert((wx, wy));
                            let mut errors = Vec::new();
                            let ch = match worldgen.as_mut() {
                                Some(runtime) => runtime.load_brand_new(
                                    wx, wy, &mut ctx,
                                    WorldGenCell { world_x: 0, world_y: 0 },
                                    &mut special_zones, &mut errors,
                                ).map_err(|e| anyhow!("WorldGen at {wx},{wy}: {e}"))?,
                                None => {
                                    let mut ch = IsoChunk::new(wx, wy);
                                    cell_loader::load_cell_binary_chunk(
                                        &mut ctx, wx, wy, &mut ch, &mut errors,
                                    );
                                    ch
                                }
                            };
                            for e in errors {
                                log::warn!("chunk {wx},{wy}: {e}");
                            }
                            ch
                        }
                    };
                    // ServerMap.ServerCell.RecalcAll2: the post-processors require loaded
                    // neighbours, and freshly created squares inherit this state.
                    ch.set_loaded();
                    zmin = zmin.min(ch.get_min_level());
                    zmax = zmax.max(ch.get_max_level());
                    chunks.push(ch);
                }
            }
        }
    }
    if chunks.is_empty() {
        return Err(anyhow!("no cells in region {x0},{y0}..{x1},{y1}"));
    }
    let missing = ctx.missing_tiles.len() as u64;
    drop(ctx);
    // IsoChunk.update: after all neighbours in this region have been loaded, run
    // blending handshakes followed by floor attachments on actual scene chunks.
    if let Some(runtime) = worldgen.as_mut() {
        runtime.postprocess_loaded(&mut chunks, &mut world.sprites, &world.alias)
            .map_err(|e| anyhow!("WorldGen post-processing: {e}"))?;
    }
    if save.is_some() {
        log::info!(
            "{loaded_from_save} of {} chunks loaded from the save",
            chunks.len()
        );
    }
    if persist_worldgen {
        let sv = save.as_ref().ok_or_else(|| anyhow!("missing save metadata"))?;
        // Preflight every native serialization before touching any file. This is deliberately
        // restricted to newly generated chunks: overwriting a player's existing chunks could
        // discard object state that the Rust loader has not yet ported.
        let mut ready = Vec::with_capacity(generated_chunks.len());
        for chunk in &chunks {
            if generated_chunks.contains(&(chunk.wx, chunk.wy)) {
                let data = chunk.save_to_buffer(&world.sprites, Some(&sv.dictionary))
                    .map_err(|e| anyhow!("saving WorldGen chunk {},{}: {e}", chunk.wx, chunk.wy))?;
                ready.push(((chunk.wx, chunk.wy), data));
            }
        }
        for ((wx, wy), data) in ready {
            IsoChunk::save_bytes_to_disk(&sv.dir, wx, wy, &data)
                .map_err(|e| anyhow!("writing WorldGen chunk {wx},{wy}: {e}"))?;
        }
        log::info!("persisted {} new WorldGen chunks", generated_chunks.len());
    }
    // zombie.iso.IsoGridSquare.RecalcProperties for every square, then the collide matrices over the loaded region.
    for ch in &mut chunks {
        for z in ch.get_min_level()..=ch.get_max_level() {
            for i in 0..64 {
                if let Some(sq) = ch.get_grid_square_mut(i % 8, i / 8, z) {
                    sq.recalc_properties(&world.sprites);
                }
            }
        }
    }

    let mut wanted: HashSet<String> = HashSet::new();
    for ch in &chunks {
        for z in ch.get_min_level()..=ch.get_max_level() {
            for i in 0..64 {
                let Some(sq) = ch.get_grid_square(i % 8, i / 8, z) else {
                    continue;
                };
                for o in &sq.objects {
                    for s in o
                        .sprite
                        .iter()
                        .chain(overlay_sprite(o).iter())
                        .chain(o.attached_anim_sprite.iter().flatten())
                    {
                        wanted.extend(world.sprites.sprite(*s).name.clone());
                    }
                    // both states of a door, so that it can be opened later
                    if let ObjectKind::IsoDoor(d) = &o.kind {
                        for s in d.open_sprite.iter().chain(d.closed_sprite.iter()) {
                            wanted.extend(world.sprites.sprite(*s).name.clone());
                        }
                    }
                }
            }
        }
    }
    // Texture packs through the engine's file access (disk natively, the mounted files in the browser), 2x packs first.
    let pack_dir = root.join("media").join("texturepacks");
    let mut names: Vec<String> = vfs::read_dir(&pack_dir)?
        .into_iter()
        .filter(|n| n.ends_with(".pack"))
        .collect();
    names.sort_by_key(|n| pack_order(n));
    let mut packs = names.into_iter().map(|n| {
        let p = pack_dir.join(&n);
        // In the browser a pack is dropped from memory once read.
        vfs::take(&p)
            .map(Ok)
            .unwrap_or_else(|| vfs::read(&p))
            .map(|b| (n, b))
            .map_err(|e| e.to_string())
    });
    let (mut atlas, texture_catalog) = if stream_window.is_some() {
        let (atlas, catalogue) = build_atlas_and_catalog_from(
            &mut packs, &wanted, 8192, 8192, &pack_dir).map_err(|e| anyhow!(e))?;
        (atlas, Some(catalogue))
    } else {
        (build_atlas_from(&mut packs, &wanted, 8192, 8192).map_err(|e| anyhow!(e))?, None)
    };
    // Reserve stable UV space for incremental streaming. Fixed-region scenes
    // retain their compact atlas and deterministic screenshot representation.
    if stream_window.is_some() { atlas.reserve_height(2048); }
    // The live streamer retains the world, RNG, save dictionary and WorldGen instance.
    // Legacy fixed-region scenes discard these temporary loading resources.
    drop(factory);
    let stream = stream_window.map(|window| WorldStreamState {
        root: root.to_path_buf(), save, rand, locked_houses: sandbox.get_int("LockedHouses").unwrap_or(6),
        window, persist_worldgen, zones: special_zones,
        lotpack_cache: None, texture_catalog,
        parked: HashMap::new(), parking_order: VecDeque::new(), dirty: HashSet::new(),
        pending_save: HashSet::new(),
    });
    let chunk_index = chunks.iter().enumerate()
        .map(|(i, ch)| ((ch.wx, ch.wy), i)).collect();
    let state = SceneState {
        world,
        chunks,
        chunk_index,
        bounds: (x0, y0, x1, y1),
        levels: (zmin, zmax),
        missing,
        loaded_from_save,
        worldgen,
        stream,
    };
    let mut scene = Scene {
        atlas,
        instances: Vec::new(),
        level_ranges: Vec::new(),
        min_level: zmin,
        unresolved: 0,
        collision: Collision::default(),
        meta: Vec::new(),
        rooms: std::collections::HashMap::new(),
        clear: [20.0 / 255.0, 24.0 / 255.0, 20.0 / 255.0],
        state: Some(state),
        presentations: HashMap::new(),
        stream_touched: HashSet::new(), pending_atlas_delta: AtlasDelta::default(),
        gpu_atlas_patches: Vec::new(), gpu_atlas_full: false,
    };
    scene.rebuild();
    Ok(scene)
}

/// The engine-side world a scene was built from, kept so that the scene can change (doors) and be rebuilt.
pub struct SceneState {
    pub world: IsoWorld,
    pub chunks: Vec<IsoChunk>,
    chunk_index: HashMap<(i32, i32), usize>,
    bounds: (i32, i32, i32, i32),
    levels: (i32, i32),
    missing: u64,
    /// Chunks read from the save's `map/*.bin` (the rest were created from the map).
    pub loaded_from_save: usize,
    /// Retained to keep chunk generation and neighbour-handshake state across loads.
    pub worldgen: Option<WorldGenRuntime>,
    /// Present only for bounded runtime chunk streaming.
    stream: Option<WorldStreamState>,
}

/// Bounded resident chunks, backed by the same IsoWorld/WorldGen/RNG for the entire session.
/// Java's IsoChunkMap uses a rotating chunk grid; here the window is explicit because the
/// renderer owns a dynamic Vec rather than IsoChunkMap's two swap buffers.
struct WorldStreamState {
    root: std::path::PathBuf,
    save: Option<SaveFiles>,
    rand: RandAbstract<java::Random>,
    locked_houses: i32,
    window: ChunkWindow,
    persist_worldgen: bool,
    zones: Vec<WorldGenZone>,
    // zombie.iso.IsoLot keeps a pooled file handle and reuses it for the next
    // chunk in the same lotpack. The Rust loader uses bytes, so preserve the
    // last pack across jobs rather than reading it once per requested chunk.
    lotpack_cache: Option<(std::path::PathBuf, Vec<u8>)>,
    texture_catalog: Option<TextureCatalog>,
    // Recently evicted *read-only/unsaved* chunks. An edited chunk is pinned until the
    // session ends; silently dropping the player's door changes would be incorrect.
    parked: HashMap<(i32, i32), IsoChunk>,
    parking_order: VecDeque<(i32, i32)>,
    dirty: HashSet<(i32, i32)>,
    // Fresh generated chunks cannot be persisted until their neighbour handshakes
    // finish. Keep them in memory if they leave a partially loaded window.
    pending_save: HashSet<(i32, i32)>,
}

impl WorldStreamState {
    const CLEAN_CACHE_LIMIT: usize = 256;

    fn park(&mut self, chunk: IsoChunk) {
        let key = (chunk.wx, chunk.wy);
        // A persisted, unmodified chunk can be loaded from disk, not kept twice in RAM.
        let disk_path = self.save.as_ref().map(|s| s.dir.join("map")
            .join(key.0.to_string()).join(format!("{}.bin", key.1)));
        // A key can be unloaded and reloaded repeatedly. Keep the eviction queue
        // bounded too, not just the chunk storage, by removing stale occurrences.
        self.parking_order.retain(|old| *old != key);
        if disk_path.as_ref().is_some_and(|p| p.is_file())
            && !self.dirty.contains(&key) && !self.pending_save.contains(&key) {
            self.parked.remove(&key);
            return;
        }
        self.parked.insert(key, chunk);
        self.parking_order.push_back(key);
        // Bounded clean cache: changed chunks remain pinned to avoid losing unsaved edits.
        let mut attempts = 0;
        while self.parked.len() > Self::CLEAN_CACHE_LIMIT + self.dirty.len() + self.pending_save.len()
            && attempts < self.parking_order.len() {
            attempts += 1;
            let Some(old) = self.parking_order.pop_front() else { break; };
            if self.dirty.contains(&old) || self.pending_save.contains(&old) {
                self.parking_order.push_back(old);
            }
            else { self.parked.remove(&old); }
        }
    }
}

/// Stats for one completed resident-window update.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StreamReport {
    pub loaded: usize,
    pub unloaded: usize,
    pub restored_from_disk: usize,
    pub teleported: bool,
    /// A different texture atlas was required; GPU bind groups must be replaced.
    pub atlas_changed: bool,
}

/// The render/collision work for one chunk. The Java IsoChunk.doLoadGridsquare
/// integrates squares individually; a newly completed chunk does not invalidate the
/// entire IsoChunkMap. Our contiguous wgpu vertex stream is assembled from these
/// per-chunk parts in (z, y, x) painter order, without recalculating existing squares.
#[derive(Clone, Default)]
pub struct ChunkPresentation {
    levels: BTreeMap<i32, BTreeMap<(i32, i32), Vec<(Instance, (i32, i32, i32, i32))>>>,
    collision: Collision,
    unresolved: u64,
}

fn room_index(state: &SceneState) -> HashMap<(i32, i32, i32), i32> {
    let (x0, y0, x1, y1) = state.bounds;
    let mut rooms = HashMap::new();
    for cy in y0..=y1 {
        for cx in x0..=x1 {
            let Some(cell) = state.world.meta_grid.get_cell_data(cx, cy) else { continue; };
            for r in &cell.room_list {
                let def = state.world.meta_grid.room(*r);
                for q in &def.rects {
                    for yy in q.y..q.y + q.h {
                        for xx in q.x..q.x + q.w {
                            rooms.insert((xx, yy, def.level), r.0 as i32);
                        }
                    }
                }
            }
        }
    }
    rooms
}

fn present_chunk(
    state: &SceneState, chunk: &IsoChunk, atlas: &Atlas,
    rooms: &HashMap<(i32, i32, i32), i32>,
) -> (ChunkPresentation, Vec<(i32, i32, i32, i32)>) {
    let mut out = ChunkPresentation::default();
    let mut vision_updates = Vec::new();
    let world = &state.world;
    let (aw, ah) = (atlas.width as f32, atlas.height as f32);
    for z in chunk.get_min_level()..=chunk.get_max_level() {
        for i in 0..64 {
            let Some(sq) = chunk.get_grid_square(i % 8, i / 8, z) else { continue; };
            let (matrix, _path, vision) = sq.calculate_matrices(&world.alias, state);
            out.collision.set_collide_matrix(sq.x, sq.y, sq.z, matrix);
            vision_updates.push((sq.x, sq.y, sq.z, vision));
            let mut mask = 0u16;
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let other = state.get_grid_square(sq.x + dx, sq.y + dy, sq.z);
                    if (dx != 0 || dy != 0)
                        && sq.test_collide_special_objects(&world.alias, other, state).is_some()
                    {
                        mask |= 1 << ((dx + 1) + (dy + 1) * 3);
                    }
                }
            }
            out.collision.set_special_blocks(sq.x, sq.y, sq.z, mask);
            let (gx, gy) = (sq.x, sq.y);
            let p = iso(gx as f32, gy as f32, z);
            let (px, py) = (p[0] as i32, p[1] as i32);
            let room = rooms.get(&(gx, gy, z)).copied().unwrap_or(-1);
            for object in &sq.objects {
                for (k, spr) in object.sprite.iter()
                    .chain(overlay_sprite(object).iter())
                    .chain(object.attached_anim_sprite.iter().flatten()).enumerate()
                {
                    if k == 0 {
                        let flags = sprite_collision_flags(world, *spr);
                        if flags != 0 { out.collision.add(gx, gy, z, flags); }
                    }
                    let Some(sprite) = world.sprites.sprite(*spr).name.as_ref()
                        .and_then(|name| atlas.sprites.get(name)) else {
                        out.unresolved += 1;
                        continue;
                    };
                    let x = (px - sprite.orig_w / 2 + sprite.off_x) as f32;
                    let y = (py + TILE_H - sprite.orig_h + sprite.off_y) as f32
                        - object.render_y_offset * 2.0;
                    out.levels.entry(z).or_default().entry((gy, gx)).or_default().push((Instance {
                        rect: [x, y, sprite.w as f32, sprite.h as f32],
                        uv: [
                            sprite.ax as f32 / aw,
                            sprite.ay as f32 / ah,
                            (sprite.ax as i32 + sprite.w) as f32 / aw,
                            (sprite.ay as i32 + sprite.h) as f32 / ah,
                        ],
                        layer: sprite.layer as f32,
                    }, (gx, gy, z, room)));
                }
            }
        }
    }
    (out, vision_updates)
}

/// Prepared off the World Streamer worker. Only changed chunks and their affected
/// neighbour presentations cross the channel. No full collision grid or full instance
/// vector is serialized with each completed job.
pub struct StreamSnapshot {
    pub report: StreamReport,
    pub chunks: Vec<IsoChunk>,
    pub removed: Vec<(i32, i32)>,
    pub sprite_names: Vec<(SpriteRef, String)>,
    pub atlas: Option<Atlas>,
    pub atlas_delta: Option<AtlasDelta>,
    pub presentations: Vec<((i32, i32), ChunkPresentation)>,
    /// Grid-square vision matrices recalculated when neighbouring chunks change.
    /// These are part of the native IsoGridSquare state, not merely GPU data.
    pub vision_updates: Vec<(i32, i32, i32, i32)>,
    pub min_level: i32,
    pub missing: u64,
    pub rooms: Option<HashMap<(i32, i32, i32), i32>>,
    pub window: ChunkWindow,
    pub loaded_from_save: usize,
}

/// Keep a per-worker copy of last published objects. Unchanged chunks never
/// cross the completion channel; adjacent collision/vision data is refreshed
/// when a neighbour enters or leaves the window.
#[derive(Default)]
pub struct StreamSnapshotCache {
    previous: HashMap<(i32, i32), IsoChunk>,
    previous_bounds: Option<(i32, i32, i32, i32)>,
}

impl Scene {
    fn update_stream_presentations(&mut self, dirty: &HashSet<(i32, i32)>, refresh_rooms: bool)
        -> Vec<(i32, i32, i32, i32)> {
        let Some(state) = self.state.as_ref() else { return Vec::new(); };
        if refresh_rooms { self.rooms = room_index(state); }
        let mut rendered = Vec::new();
        for key in dirty {
            let Some(&ci) = state.chunk_index.get(key) else { continue; };
            let (part, visions) = present_chunk(state, &state.chunks[ci], &self.atlas, &self.rooms);
            rendered.push((*key, part, visions));
        }
        let Some(state) = self.state.as_mut() else { return Vec::new(); };
        let mut updates = Vec::new();
        for (key, part, visions) in rendered {
            for &(x, y, z, vision) in &visions {
                if let Some(sq) = state.square_mut(x, y, z) {
                    sq.vision_matrix = vision;
                    updates.push((x, y, z, vision));
                }
            }
            self.presentations.insert(key, part);
        }
        updates
    }

    /// Stitch already computed chunk instances in global painter order. This is a
    /// linear copy into the wgpu vertex stream, not an IsoGridSquare recalc or
    /// WorldGen pass over all resident squares.
    fn compose_stream_instances(&mut self) {
        let Some(state) = self.state.as_ref() else { return; };
        let (zmin, zmax) = state.levels;
        let mut instances = Vec::new();
        let mut meta = Vec::new();
        let mut ranges = Vec::new();
        for z in zmin..=zmax {
            ranges.push(instances.len() as u32);
            // Original Scene::rebuild iterates every square in (z, y, x) order.
            // Chunk-by-chunk concatenation would incorrectly draw an entire 8x8
            // block before the next block's first row. Merge cached square lists.
            let mut squares = Vec::new();
            for part in self.presentations.values() {
                if let Some(tiles) = part.levels.get(&z) {
                    squares.extend(tiles.iter());
                }
            }
            squares.sort_by_key(|(coordinate, _)| (coordinate.0, coordinate.1));
            for (_, sprites) in squares {
                for &(sprite, location) in sprites {
                    instances.push(sprite);
                    meta.push(location);
                }
            }
        }
        ranges.push(instances.len() as u32);
        self.instances = instances;
        self.meta = meta;
        self.level_ranges = ranges;
        self.min_level = zmin;
        self.unresolved = state.missing + self.presentations.values()
            .map(|part| part.unresolved).sum::<u64>();
    }

    /// IsoChunk.loadGridSquare: publish per-chunk presentation and object changes.
    /// Rebuild an unchanged chunk's collision only when the squares in its one-chunk
    /// neighbourhood have changed (the Java grid matrices consult nearby squares).
    pub fn stream_snapshot(&mut self, cache: &mut StreamSnapshotCache,
        report: StreamReport, include_atlas: bool) -> Result<StreamSnapshot> {
        let state = self.state.as_ref().ok_or_else(|| anyhow!("stream snapshot without state"))?;
        let stream = state.stream.as_ref().ok_or_else(|| anyhow!("stream snapshot without window"))?;
        let window = stream.window;
        let initial = cache.previous_bounds.is_none();
        let keys: HashSet<_> = state.chunks.iter().map(|chunk| (chunk.wx, chunk.wy)).collect();
        let candidates: HashSet<_> = if initial { keys.clone() }
            else { std::mem::take(&mut self.stream_touched) };
        let changed: HashSet<_> = state.chunks.iter()
            .filter(|ch| candidates.contains(&(ch.wx, ch.wy))
                && cache.previous.get(&(ch.wx, ch.wy)) != Some(*ch))
            .map(|ch| (ch.wx, ch.wy)).collect();
        let removed: Vec<_> = cache.previous.keys()
            .filter(|key| !keys.contains(key)).copied().collect();
        // Nearby squares can change their collision matrix even if their own
        // objects did not change, e.g. when a new chunk supplies an adjacent wall.
        let mut dirty = changed.clone();
        for &(wx, wy) in changed.iter().chain(removed.iter()) {
            for dx in -1..=1 {
                for dy in -1..=1 {
                    let neighbour = (wx + dx, wy + dy);
                    if keys.contains(&neighbour) { dirty.insert(neighbour); }
                }
            }
        }
        // A new meta-grid cell can introduce rooms extending into an old chunk.
        // Re-evaluate its room metadata on those rare window-boundary updates.
        let bounds_changed = cache.previous_bounds != Some(state.bounds);
        if initial || bounds_changed {
            dirty.extend(keys.iter().copied());
        }
        let vision_updates = self.update_stream_presentations(&dirty, bounds_changed);
        for key in &removed {
            cache.previous.remove(key);
            self.presentations.remove(key);
        }
        let state = self.state.as_ref().ok_or_else(|| anyhow!("lost streaming state"))?;
        let mut chunk_changes = Vec::new();
        for chunk in &state.chunks {
            let key = (chunk.wx, chunk.wy);
            if changed.contains(&key) {
                cache.previous.insert(key, chunk.clone());
                chunk_changes.push(chunk.clone());
            }
        }
        let mut refs = HashSet::new();
        for chunk in &chunk_changes {
            for z in chunk.get_min_level()..=chunk.get_max_level() {
                for i in 0..64 {
                    let Some(sq) = chunk.get_grid_square(i % 8, i / 8, z) else { continue; };
                    for obj in &sq.objects {
                        refs.extend(obj.sprite);
                        if let Some(a) = &obj.attached_anim_sprite { refs.extend(a.iter().copied()); }
                        match &obj.kind {
                            ObjectKind::IsoDoor(d) => {
                                refs.extend(d.open_sprite); refs.extend(d.closed_sprite);
                            }
                            ObjectKind::IsoWindow(w) => {
                                refs.extend(w.open_sprite); refs.extend(w.closed_sprite);
                                refs.extend(w.smashed_sprite); refs.extend(w.glass_removed_sprite);
                            }
                            ObjectKind::IsoCurtain(c) => {
                                refs.extend(c.open_sprite); refs.extend(c.closed_sprite);
                            }
                            _ => {}
                        }
                        if let Some(save) = &obj.save {
                            refs.extend(save.overlay_sprite);
                            for splat in &save.wall_blood_splats { refs.extend(splat.sprite); }
                            if let zomboid_engine::zombie::iso::chunk_save::ClassSave::Barbecue {
                                normal, no_tank, ..
                            } = &save.class {
                                refs.extend(*normal); refs.extend(*no_tank);
                            }
                        }
                    }
                }
            }
        }
        let sprite_names = refs.into_iter().map(|spr| {
            state.world.sprites.sprite(spr).name.as_ref()
                .map(|name| (spr, name.clone()))
                .ok_or_else(|| anyhow!("worker sprite {:?} without a name", spr))
        }).collect::<Result<Vec<_>>>()?;
        let bounds = state.bounds;
        let rooms_changed = cache.previous_bounds != Some(bounds);
        cache.previous_bounds = Some(bounds);
        let presentations = dirty.into_iter().filter_map(|key| self.presentations.get(&key)
            .map(|part| (key, part.clone()))).collect();
        // Stable UV coordinates mean an atlas extension only transfers the new
        // rectangles. The full RGBA array crosses the channel once at startup.
        let atlas = initial.then(|| self.atlas.clone());
        let atlas_delta = if initial {
            self.pending_atlas_delta = AtlasDelta::default();
            None
        } else if include_atlas && !self.pending_atlas_delta.patches.is_empty() {
            Some(std::mem::take(&mut self.pending_atlas_delta))
        } else { None };
        Ok(StreamSnapshot {
            report, chunks: chunk_changes, removed, sprite_names,
            atlas, atlas_delta, presentations, vision_updates,
            min_level: state.levels.0, missing: state.missing,
            rooms: rooms_changed.then(|| self.rooms.clone()),
            window, loaded_from_save: state.loaded_from_save,
        })
    }

    /// Accept WorldStreamer results without recalculating all resident squares.
    /// Sprite handles are translated by name because the two IsoWorld instances differ.
    pub fn accept_stream_snapshot(&mut self, mut snapshot: StreamSnapshot) -> Result<StreamReport> {
        let state = self.state.as_mut().ok_or_else(|| anyhow!("stream update without scene state"))?;
        let mut sprite_map = HashMap::new();
        for (remote, name) in snapshot.sprite_names {
            sprite_map.insert(remote, state.world.sprites.get_sprite(&name));
        }
        let translate = |value: &mut Option<SpriteRef>| -> Result<()> {
            if let Some(reference) = value {
                *reference = *sprite_map.get(reference)
                    .ok_or_else(|| anyhow!("stream worker sprite {:?} has no name", reference))?;
            }
            Ok(())
        };
        for chunk in &mut snapshot.chunks {
            for z in chunk.get_min_level()..=chunk.get_max_level() {
                for i in 0..64 {
                    let Some(square) = chunk.get_grid_square_mut(i % 8, i / 8, z) else { continue; };
                    for object in &mut square.objects {
                        translate(&mut object.sprite)?;
                        if let Some(attachments) = object.attached_anim_sprite.as_mut() {
                            for spr in attachments {
                                let mut v = Some(*spr);
                                translate(&mut v)?;
                                if let Some(mapped) = v { *spr = mapped; }
                            }
                        }
                        match &mut object.kind {
                            ObjectKind::IsoDoor(d) => {
                                translate(&mut d.open_sprite)?;
                                translate(&mut d.closed_sprite)?;
                            }
                            ObjectKind::IsoWindow(w) => {
                                translate(&mut w.open_sprite)?;
                                translate(&mut w.closed_sprite)?;
                                translate(&mut w.smashed_sprite)?;
                                translate(&mut w.glass_removed_sprite)?;
                            }
                            ObjectKind::IsoCurtain(c) => {
                                translate(&mut c.open_sprite)?;
                                translate(&mut c.closed_sprite)?;
                            }
                            _ => {}
                        }
                        if let Some(save) = object.save.as_mut() {
                            translate(&mut save.overlay_sprite)?;
                            for splat in &mut save.wall_blood_splats { translate(&mut splat.sprite)?; }
                            if let zomboid_engine::zombie::iso::chunk_save::ClassSave::Barbecue {
                                normal, no_tank, ..
                            } = &mut save.class {
                                translate(normal)?; translate(no_tank)?;
                            }
                        }
                    }
                }
            }
        }
        let removed: HashSet<_> = snapshot.removed.into_iter().collect();
        let mut parked = Vec::new();
        state.chunks.retain(|chunk| {
            if removed.contains(&(chunk.wx, chunk.wy)) {
                parked.push(chunk.clone());
                false
            } else { true }
        });
        // The foreground may need to resume synchronous streaming if the
        // worker fails. Keep evicted edited chunks instead of silently losing
        // their object state on that fallback path.
        if let Some(stream) = state.stream.as_mut() {
            for chunk in parked { stream.park(chunk); }
        }
        let existing: HashMap<_, _> = state.chunks.iter().enumerate()
            .map(|(i, ch)| ((ch.wx, ch.wy), i)).collect();
        let changed_keys: HashSet<_> = snapshot.chunks.iter()
            .map(|ch| (ch.wx, ch.wy)).collect();
        for chunk in snapshot.chunks {
            if let Some(&index) = existing.get(&(chunk.wx, chunk.wy)) {
                state.chunks[index] = chunk;
            } else {
                state.chunks.push(chunk);
            }
        }
        state.chunk_index = state.chunks.iter().enumerate()
            .map(|(i, c)| ((c.wx, c.wy), i)).collect();
        // The worker updated these squares while preparing per-chunk collision.
        // Applying only their presentation left the foreground's vision_matrix
        // stale, breaking native-state parity and potentially line-of-sight.
        for (x, y, z, vision) in snapshot.vision_updates {
            if let Some(square) = state.square_mut(x, y, z) {
                square.vision_matrix = vision;
            }
        }
        if let Some(runtime) = state.worldgen.as_mut() {
            runtime.generator.chunk_states.retain(|key, _| state.chunk_index.contains_key(key));
            for chunk in &state.chunks {
                if changed_keys.contains(&(chunk.wx, chunk.wy)) {
                    runtime.register_saved_chunk(chunk);
                }
            }
        }
        state.bounds = snapshot.window.cells();
        state.levels = (
            state.chunks.iter().map(IsoChunk::get_min_level).min().unwrap_or(0),
            state.chunks.iter().map(IsoChunk::get_max_level).max().unwrap_or(0),
        );
        state.missing = snapshot.missing;
        state.loaded_from_save = snapshot.loaded_from_save;
        if let Some(stream) = state.stream.as_mut() {
            stream.window = snapshot.window;
        }
        let mut report = snapshot.report;
        report.atlas_changed = if let Some(atlas) = snapshot.atlas {
            let changed = self.atlas.width != atlas.width
                || self.atlas.height != atlas.height
                || self.atlas.layers != atlas.layers
                || self.atlas.sprites != atlas.sprites
                || self.atlas.rgba != atlas.rgba;
            if changed {
                self.atlas = atlas;
                self.gpu_atlas_full = true;
                self.gpu_atlas_patches.clear();
            }
            changed
        } else if let Some(delta) = snapshot.atlas_delta {
            let has_patches = !delta.patches.is_empty();
            let changed_extent = self.atlas.apply_delta(&delta).map_err(anyhow::Error::msg)?;
            if changed_extent { self.gpu_atlas_full = true; }
            self.gpu_atlas_patches.extend(delta.patches);
            has_patches
        } else { false };
        if let Some(rooms) = snapshot.rooms { self.rooms = rooms; }
        let mut touched = removed;
        for (key, part) in snapshot.presentations {
            touched.insert(key);
            self.presentations.insert(key, part);
        }
        for key in &touched {
            if !state.chunk_index.contains_key(key) { self.presentations.remove(key); }
        }
        self.collision.erase_chunks(&touched);
        for key in &touched {
            if let Some(part) = self.presentations.get(key) {
                self.collision.extend_from(&part.collision);
            }
        }
        self.compose_stream_instances();
        Ok(report)
    }
}

impl Scene {
    /// IsoChunkMap.update: move a bounded residency window around a character.
    /// Existing chunks keep their object state. New chunks follow LoadOrCreate/LoadBrandNew,
    /// are postprocessed with all resident neighbours, then published to the renderer.
    /// The previous scene remains owned and usable if loading returns an error.
    pub fn stream_around(&mut self, x: f32, y: f32) -> Result<Option<StreamReport>> {
        self.stream_around_budget(x, y, usize::MAX)
    }

    /// IsoChunkMap.updateInternal: publish at most `budget` newly prepared chunks per
    /// update, rather than processing an entire incoming edge in one frame. The resident
    /// set may be sparse until subsequent calls finish the requested window.
    pub fn stream_around_budget(
        &mut self, x: f32, y: f32, budget: usize,
    ) -> Result<Option<StreamReport>> {
        self.stream_around_budget_inner(x, y, budget, true, None, None)
    }

    /// WorldStreamer.DoChunkAlways prepares chunks; IsoChunkMap.updateInternal
    /// incorporates their per-square presentation later on the render thread.
    /// The worker must not rebuild the whole scene for every completed chunk.
    pub fn stream_around_budget_unrebuilt(
        &mut self, x: f32, y: f32, budget: usize,
    ) -> Result<Option<StreamReport>> {
        self.stream_around_budget_inner(x, y, budget, false, None, None)
    }

    /// WorldStreamer.ChunkComparator: use the player's previous position to
    /// sort pending jobs by distance from the projected position ten tiles ahead.
    pub fn stream_around_budget_projected(
        &mut self, x: f32, y: f32, last_x: f32, last_y: f32, budget: usize,
    ) -> Result<Option<StreamReport>> {
        self.stream_around_budget_inner(x, y, budget, false, Some((last_x, last_y)), None)
    }

    /// Prioritise a grid square that movement has found missing. The normal
    /// scheduling order remains WorldStreamer.ChunkComparator when no hint exists.
    pub fn stream_around_budget_projected_priority(
        &mut self, x: f32, y: f32, last_x: f32, last_y: f32,
        budget: usize, priority: Option<(i32, i32)>,
    ) -> Result<Option<StreamReport>> {
        self.stream_around_budget_inner(x, y, budget, false,
            Some((last_x, last_y)), priority)
    }

    fn stream_around_budget_inner(
        &mut self, x: f32, y: f32, budget: usize, rebuild: bool,
        last_position: Option<(f32, f32)>, priority: Option<(i32, i32)>,
    ) -> Result<Option<StreamReport>> {
        if budget == 0 { return Ok(None); }
        let Some(mut state) = self.state.take() else { return Ok(None); };
        // Save the resident identities, including states belonging to the player, until
        // the entire update (I/O, WorldGen and texture atlas) has succeeded.
        let started = std::time::Instant::now();
        let old_window = state.stream.as_ref().map(|stream| stream.window);
        let old_keys: HashSet<_> = state.chunks.iter().map(|c| (c.wx, c.wy)).collect();
        let mut detached = Vec::new();
        let result = (|| -> Result<Option<StreamReport>> {
            let Some(stream) = state.stream.as_mut() else { return Ok(None); };
            let next = ChunkWindow::for_player(x, y, stream.window.radius)
                .map_err(|e| anyhow!(e))?;
            let diff = stream.window.diff(next);
            // Include missing chunks of the current window as well. Once a window is
            // published, its remaining queued chunks are still loaded on later ticks.
            let missing = match last_position {
                Some(previous) => next.missing_resident_projected(&old_keys, (x, y), previous),
                None => next.missing_resident(&old_keys),
            };
            let mut missing = missing;
            if let Some(wanted) = priority {
                ChunkWindow::prioritize_missing(&mut missing, wanted);
            }
            let needed: Vec<_> = missing.into_iter()
                .filter(|(wx, wy)| state.world.meta_grid
                    .get_header(wx.div_euclid(32), wy.div_euclid(32)).is_some())
                .collect();
            if next == stream.window && needed.is_empty() { return Ok(None); }
            let mut additions: Vec<IsoChunk> = Vec::new();
            let mut restored = 0;
            let mut generated = HashSet::new();
            {
                let factory = stream.save.as_ref().map(|sv| InventoryItemFactory::new(
                    &sv.dictionary, &sv.scripts, sv.recorded_media.clone(), &sv.recorded_media_lua,
                ));
                let mut ctx = LoadContext {
                    sprites: &mut state.world.sprites,
                    alias: &state.world.alias,
                    glass_removed_window_sprite_map: &state.world.tile_definitions.glass_removed_window_sprite_map,
                    smashed_window_sprite_map: &state.world.tile_definitions.smashed_window_sprite_map,
                    meta_grid: &mut state.world.meta_grid,
                    rand: &mut stream.rand,
                    locked_houses: stream.locked_houses,
                    missing_tiles: Vec::new(),
                    lotpack_cache: stream.lotpack_cache.take(),
                };
                for (wx, wy) in needed.iter().take(budget) {
                    // The original does not enqueue chunks without a lotheader. Negative
                    // coordinates use Euclidean cell division, never C-style truncated division.
                    if ctx.meta_grid.get_header(wx.div_euclid(32), wy.div_euclid(32)).is_none() {
                        continue;
                    }
                    if let Some(mut cached) = stream.parked.remove(&(*wx, *wy)) {
                        stream.parking_order.retain(|old| *old != (*wx, *wy));
                        cached.set_loaded();
                        if let Some(runtime) = state.worldgen.as_mut() {
                            runtime.register_saved_chunk(&cached);
                        }
                        additions.push(cached);
                        continue;
                    }
                    let from_save = stream.save.as_ref().zip(factory.as_ref()).and_then(|(sv, f)| {
                        let path = sv.dir.join("map").join(wx.to_string()).join(format!("{wy}.bin"));
                        let data = vfs::read(&path).ok()?;
                        let mut sctx = SaveLoadContext {
                            sprites: &mut *ctx.sprites,
                            alias: ctx.alias,
                            dictionary: &sv.dictionary,
                            items: f,
                            meta_grid: &mut *ctx.meta_grid,
                            rand: &mut *ctx.rand,
                            has_lot_header: true,
                        };
                        match IsoChunk::load_from_disk_or_buffer(&data, *wx, *wy, &mut sctx) {
                            Ok(c) => Some(c),
                            Err(e) => {
                                log::warn!("stream chunk {wx},{wy} corrupt: {e}; generating from lotpack");
                                None
                            }
                        }
                    });
                    let mut chunk = if let Some(saved) = from_save {
                        restored += 1;
                        if let Some(runtime) = state.worldgen.as_mut() {
                            runtime.register_saved_chunk(&saved);
                        }
                        saved
                    } else {
                        generated.insert((*wx, *wy));
                        let mut errors = Vec::new();
                        let generated_chunk = match state.worldgen.as_mut() {
                            Some(runtime) => runtime.load_brand_new(
                                *wx, *wy, &mut ctx,
                                WorldGenCell { world_x: 0, world_y: 0 },
                                &mut stream.zones, &mut errors,
                            ).map_err(|e| anyhow!("stream WorldGen {wx},{wy}: {e}"))?,
                            None => return Err(anyhow!("streaming requires the native WorldGen runtime")),
                        };
                        for e in errors { log::warn!("stream chunk {wx},{wy}: {e}"); }
                        generated_chunk
                    };
                    chunk.set_loaded();
                    additions.push(chunk);
                }
                state.missing += ctx.missing_tiles.len() as u64;
                stream.lotpack_cache = ctx.lotpack_cache.take();
            }
            // A map boundary can have no available chunks. Keep the existing window and
            // its collision grid rather than leaving the player in an empty world.
            let player = (next.cx, next.cy);
            if !additions.iter().any(|c| (c.wx, c.wy) == player)
                && !state.chunks.iter().any(|c| (c.wx, c.wy) == player)
            {
                return Err(anyhow!("stream destination has no lotheader at chunk {},{}", player.0, player.1));
            }
            let load_ms = started.elapsed().as_secs_f64() * 1000.0;
            let count = additions.len();
            let newly_loaded: HashSet<_> = additions.iter()
                .map(|chunk| (chunk.wx, chunk.wy)).collect();
            state.chunks.append(&mut additions);
            // IsoChunkMap.LoadRight/Left/Up/Down releases the outgoing edge before
            // IsoChunk.update. Otherwise blending could observe a neighbour already
            // outside the active window and set a done flag too early.
            let mut resident = Vec::with_capacity(state.chunks.len());
            for ch in std::mem::take(&mut state.chunks) {
                if next.contains(ch.wx, ch.wy) { resident.push(ch); }
                else { detached.push(ch); }
            }
            state.chunks = resident;
            let removed = detached.len();
            if let Some(runtime) = state.worldgen.as_mut() {
                runtime.postprocess_near(&mut state.chunks, &mut state.world.sprites,
                    &state.world.alias, &newly_loaded)
                    .map_err(|e| anyhow!("stream postprocess: {e}"))?;
            }
            let postprocess_ms = started.elapsed().as_secs_f64() * 1000.0 - load_ms;
            // Blending and attachments may change existing adjacent chunks.
            // Only these chunks need new render/collision state or a delta copy.
            let mut affected = newly_loaded.clone();
            for &(wx, wy) in &newly_loaded {
                for dx in -1..=1 {
                    for dy in -1..=1 { affected.insert((wx + dx, wy + dy)); }
                }
            }
            for chunk in &detached { affected.insert((chunk.wx, chunk.wy)); }
            self.stream_touched.extend(affected.iter().copied());
            if stream.persist_worldgen {
                stream.pending_save.extend(generated);
                // Do not commit new files while a window is still being filled.
                // Additional blending/attachment handshakes can change earlier chunks.
                let present: HashSet<_> = state.chunks.iter()
                    .map(|ch| (ch.wx, ch.wy)).collect();
                let pending_neighbours = next.missing_resident(&present).into_iter()
                    .any(|(wx, wy)| state.world.meta_grid
                        .get_header(wx.div_euclid(32), wy.div_euclid(32)).is_some());
                if !pending_neighbours && !stream.pending_save.is_empty() {
                    let sv = stream.save.as_ref().ok_or_else(|| anyhow!("missing stream save"))?;
                    let mut bytes = Vec::new();
                    for chunk in &state.chunks {
                        if stream.pending_save.contains(&(chunk.wx, chunk.wy)) {
                            bytes.push((chunk.wx, chunk.wy, chunk.save_to_buffer(
                                &state.world.sprites, Some(&sv.dictionary),
                            ).map_err(|e| anyhow!("stream serialize {},{}: {e}", chunk.wx, chunk.wy))?));
                        }
                    }
                    for (wx, wy, data) in bytes {
                        IsoChunk::save_bytes_to_disk(&sv.dir, wx, wy, &data)
                            .map_err(|e| anyhow!("stream save {wx},{wy}: {e}"))?;
                        stream.pending_save.remove(&(wx, wy));
                    }
                }
            }
            // The Java TexturePack system looks up individual sprites. Avoid
            // re-reading all packs and repacking every previous sprite whenever
            // one more chunk introduces a new overlay.
            let atlas_started = std::time::Instant::now();
            let mut missing_sprites: HashSet<String> = HashSet::new();
            for ch in &state.chunks {
                if !affected.contains(&(ch.wx, ch.wy)) { continue; }
                for z in ch.get_min_level()..=ch.get_max_level() {
                    for i in 0..64 {
                        let Some(sq) = ch.get_grid_square(i % 8, i / 8, z) else { continue; };
                        for o in &sq.objects {
                            for spr in o.sprite.iter().chain(overlay_sprite(o).iter())
                                .chain(o.attached_anim_sprite.iter().flatten()) {
                                if let Some(name) = state.world.sprites.sprite(*spr).name.as_ref() {
                                    if !self.atlas.sprites.contains_key(name) {
                                        missing_sprites.insert(name.clone());
                                    }
                                }
                            }
                            if let ObjectKind::IsoDoor(d) = &o.kind {
                                for spr in d.open_sprite.iter().chain(d.closed_sprite.iter()) {
                                    if let Some(name) = state.world.sprites.sprite(*spr).name.as_ref() {
                                        if !self.atlas.sprites.contains_key(name) {
                                            missing_sprites.insert(name.clone());
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            let mut atlas_changed = false;
            if !missing_sprites.is_empty() {
                if stream.texture_catalog.is_none() {
                    stream.texture_catalog = Some(TextureCatalog::open(&stream.root)
                        .map_err(|e| anyhow!("texture catalog: {e}"))?);
                }
                let delta = stream.texture_catalog.as_mut().expect("texture catalog initialized")
                    .append_missing(&mut self.atlas, &missing_sprites)
                    .map_err(|e| anyhow!("append texture atlas: {e}"))?;
                atlas_changed = !delta.patches.is_empty();
                if atlas_changed {
                    self.pending_atlas_delta.patches.extend(delta.patches);
                    self.pending_atlas_delta.sprites.extend(delta.sprites);
                    self.pending_atlas_delta.layers = delta.layers;
                    self.pending_atlas_delta.cursor = delta.cursor;
                }
            }
            log::debug!("World Streamer phases: load {:.1}ms postprocess {:.1}ms atlas {:.1}ms total {:.1}ms new {}",
                load_ms, postprocess_ms, atlas_started.elapsed().as_secs_f64() * 1000.0,
                started.elapsed().as_secs_f64() * 1000.0, count);
            for chunk in detached.drain(..) {
                if let Some(runtime) = state.worldgen.as_mut() {
                    runtime.generator.chunk_states.remove(&(chunk.wx, chunk.wy));
                }
                stream.park(chunk);
            }
            state.chunk_index = state.chunks.iter().enumerate()
                .map(|(i, ch)| ((ch.wx, ch.wy), i)).collect();
            state.bounds = next.cells();
            state.levels = (
                state.chunks.iter().map(IsoChunk::get_min_level).min().unwrap_or(0),
                state.chunks.iter().map(IsoChunk::get_max_level).max().unwrap_or(0),
            );
            stream.window = next;
            state.loaded_from_save += restored;
            self.min_level = state.levels.0;
            Ok(Some(StreamReport { loaded: count, unloaded: removed,
                restored_from_disk: restored, teleported: diff.teleport, atlas_changed }))
        })();
        if result.is_err() {
            // An invalid save or unavailable texture pack must never erase chunks that
            // were resident before the shift. Release only this attempt's additions.
            let mut original = Vec::new();
            for ch in std::mem::take(&mut state.chunks) {
                if old_keys.contains(&(ch.wx, ch.wy)) { original.push(ch); }
                else {
                    if let Some(runtime) = state.worldgen.as_mut() {
                        runtime.generator.chunk_states.remove(&(ch.wx, ch.wy));
                    }
                    if let Some(stream) = state.stream.as_mut() { stream.park(ch); }
                }
            }
            original.append(&mut detached);
            state.chunks = original;
            state.chunk_index = state.chunks.iter().enumerate()
                .map(|(i, ch)| ((ch.wx, ch.wy), i)).collect();
            if let Some(window) = old_window {
                state.bounds = window.cells();
                if let Some(stream) = state.stream.as_mut() { stream.window = window; }
            }
            state.levels = (
                state.chunks.iter().map(IsoChunk::get_min_level).min().unwrap_or(0),
                state.chunks.iter().map(IsoChunk::get_max_level).max().unwrap_or(0),
            );
        }
        self.state = Some(state);
        if rebuild && result.as_ref().ok().is_some_and(Option::is_some) {
            self.rebuild();
            // The synchronous path already consumed these updates directly.
            // Only the worker needs to retain a pending delta snapshot.
            self.stream_touched.clear();
            self.pending_atlas_delta = AtlasDelta::default();
        }
        result
    }
}

impl SceneState {
    fn chunk(&self, x: i32, y: i32) -> Option<usize> {
        let (wx, wy) = (x.div_euclid(8), y.div_euclid(8));
        self.chunk_index.get(&(wx, wy)).copied()
    }
    fn square_mut(&mut self, x: i32, y: i32, z: i32) -> Option<&mut IsoGridSquare> {
        let i = self.chunk(x, y)?;
        self.chunks[i].get_grid_square_mut(x.rem_euclid(8), y.rem_euclid(8), z)
    }
}
impl GetSquare for SceneState {
    fn get_grid_square(&self, x: i32, y: i32, z: i32) -> Option<&IsoGridSquare> {
        let i = self.chunk(x, y)?;
        self.chunks[i].get_grid_square(x.rem_euclid(8), y.rem_euclid(8), z)
    }
}

/// Position of a character with a continuous height, as the original keeps it (`IsoMovingObject.x/y/z`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mover {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Scene {
    /// True when this world-space chunk has completed load/attachment and is
    /// available to collision. A queued chunk is NOT yet safe to walk into.
    pub fn stream_chunk_resident(&self, wx: i32, wy: i32) -> bool {
        self.state.as_ref().is_some_and(|st| st.chunk_index.contains_key(&(wx, wy)))
    }

    /// Detect an absent chunk along the requested move or the original Java
    /// IsoMovingObject.DoCollide half-square feeler. This is NOT a collision
    /// override: movement still waits for its real IsoGridSquare and matrices.
    /// Only valid lotheader-backed chunks inside the active streaming window
    /// are eligible for scheduling priority.
    pub fn stream_pending_move_chunk(&self, m: &Mover, dx: f32, dy: f32) -> Option<(i32, i32)> {
        if dx == 0.0 && dy == 0.0 { return None; }
        let state = self.state.as_ref()?;
        let window = state.stream.as_ref()?.window;
        // Do not mistake a player already outside the world for a queued chunk.
        self.current_square(m.x, m.y, m.z)?;
        let len = (dx * dx + dy * dy).sqrt();
        let feeler = (m.x + dx / len * 0.5, m.y + dy / len * 0.5);
        for (x, y) in [(m.x + dx, m.y + dy), feeler] {
            let (wx, wy) = ((x.floor() as i32).div_euclid(8),
                (y.floor() as i32).div_euclid(8));
            if window.contains(wx, wy)
                && !state.chunk_index.contains_key(&(wx, wy))
                && state.world.meta_grid.get_header(wx.div_euclid(32), wy.div_euclid(32)).is_some()
            {
                return Some((wx, wy));
            }
        }
        None
    }

    /// zombie.iso.IsoMovingObject.setCurrentSquareFromPosition: the square under a position; where there is none at
    /// that level, the first existing square below it (down to level 0).
    pub fn current_square(&self, x: f32, y: f32, z: f32) -> Option<(i32, i32, i32)> {
        let st = self.state.as_ref()?;
        let (sx, sy, sz) = (x.floor() as i32, y.floor() as i32, z.floor() as i32);
        if st.get_grid_square(sx, sy, sz).is_some() {
            return Some((sx, sy, sz));
        }
        (0..=sz)
            .rev()
            .find(|n| st.get_grid_square(sx, sy, *n).is_some())
            .map(|n| (sx, sy, n))
    }

    /// zombie.iso.IsoMovingObject.doStairs for a character that moved from square `last` to position `m`: sets the
    /// height from the staircase under the character, snaps to the whole level when a top stair was just left, and
    /// keeps the character off the open side of a stairwell. Compared with the original through MoverProbe (a character moved by hand over
    /// a north and a west staircase, up and down: positions, heights and squares identical). Climbing states are not modelled.
    pub fn do_stairs(&self, last: (i32, i32, i32), m: &mut Mover) {
        self.do_stairs_for(last, m, true)
    }

    /// `do_stairs` for a character (`is_character`: the height snaps to the whole level after a top stair) or for
    /// another moving object.
    pub fn do_stairs_for(&self, last: (i32, i32, i32), m: &mut Mover, is_character: bool) {
        let current = self.current_square(m.x, m.y, m.z);
        self.do_stairs_at(last, current, m, is_character)
    }

    /// `doStairs` with the object's current square given (the original uses the square set before the move).
    pub fn do_stairs_at(
        &self,
        last: (i32, i32, i32),
        current: Option<(i32, i32, i32)>,
        m: &mut Mover,
        is_character: bool,
    ) {
        let Some(st) = self.state.as_ref() else {
            return;
        };
        let sq = |x: i32, y: i32, z: i32| st.get_grid_square(x, y, z);
        let Some(last_sq) = sq(last.0, last.1, last.2) else {
            return;
        };
        let Some(c) = current else {
            return;
        };
        let Some(mut current) = sq(c.0, c.1, c.2) else {
            return;
        };
        use IsoObjectType::{stairsTN, stairsTW};
        let top = |s: &IsoGridSquare| s.has_type(stairsTN) || s.has_type(stairsTW);
        if top(current) && m.z - m.z.floor() < 0.1 {
            if let Some(below) = sq(current.x, current.y, current.z - 1).filter(|b| top(b)) {
                current = below;
            }
        }
        if is_character && top(last_sq) {
            // java.lang.Math.round(float)
            m.z = (m.z + 0.5).floor();
        }
        let mut z = m.z;
        if current.has_stairs() {
            z = current.get_apparent_z(
                &st.world.alias,
                m.x - current.x as f32,
                m.y - current.y as f32,
                FBO_RENDER_CHUNK,
            );
        }
        let frac = |v: f32| v - v.floor();
        let has = |s: Option<&IsoGridSquare>, t: IsoObjectType| s.is_some_and(|s| s.has_type(t));
        let up = sq(last_sq.x, last_sq.y, last_sq.z + 1);
        if last_sq.has_type(stairsTN) && up.is_some_and(|u| u.has(IsoFlagType::WallN)) {
            if frac(m.x) > 0.8 && !has(sq(last_sq.x + 1, last_sq.y, last_sq.z), stairsTN) {
                m.x = m.x.floor() + 0.8;
            }
            if !current.has_stairs() {
                m.y = (m.y + 0.5).floor();
            }
        }
        if last_sq.has_type(stairsTW) && up.is_some_and(|u| u.has(IsoFlagType::WallW)) {
            if frac(m.y) > 0.8 && !has(sq(last_sq.x, last_sq.y + 1, last_sq.z), stairsTW) {
                m.y = m.y.floor() + 0.8;
            }
            if !current.has_stairs() {
                m.x = (m.x + 0.5).floor();
            }
        }
        let down = sq(current.x, current.y, current.z - 1);
        if current.has(IsoFlagType::WallN) && has(down, stairsTN) {
            if frac(m.x) > 0.8 && !has(sq(current.x + 1, current.y, current.z - 1), stairsTN) {
                m.x = m.x.floor() + 0.8;
            }
            if frac(m.x) < 0.2 && !has(sq(current.x - 1, current.y, current.z - 1), stairsTN) {
                m.x = m.x.floor() + 0.2;
            }
        }
        if current.has(IsoFlagType::WallW) && has(down, stairsTW) {
            if frac(m.y) > 0.8 && !has(sq(current.x, current.y + 1, current.z - 1), stairsTW) {
                m.y = m.y.floor() + 0.8;
            }
            if frac(m.y) < 0.2 && !has(sq(current.x, current.y - 1, current.z - 1), stairsTW) {
                m.y = m.y.floor() + 0.2;
            }
        }
        if (z - m.z).abs() < 0.95 {
            m.z = z;
        }
    }

    /// One tick of a character wanting to move by (dx, dy) squares: the original's movement resolution for moving
    /// objects (`postupdate`: collision with walls, doors and windows, sliding along the free axis, 3/4 speed on
    /// stairs), with the character rule of `doStairs` (the height snaps to the level after a top stair). The
    /// original additionally pushes characters out of walls with its polygon collision (`PolygonalMap2`), which is
    /// not ported, so a character can stand closer to a wall here.
    pub fn move_character(&self, m: &mut Mover, dx: f32, dy: f32) -> bool {
        let mut o = MovingObject::new(m.x, m.y, m.z, self.current_square(m.x, m.y, m.z));
        o.nx = m.x + dx;
        o.ny = m.y + dy;
        self.postupdate_for(&mut o, true);
        let moved = o.x != m.x || o.y != m.y;
        *m = Mover {
            x: o.x,
            y: o.y,
            z: o.z,
        };
        moved
    }
}

/// zombie.iso.IsoMovingObject: the movement state of an object that is not a character (no polygon collision, no
/// states): position, the position it wants to reach this tick, and what it collided with.
#[derive(Clone, Debug, PartialEq)]
pub struct MovingObject {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    /// `nx`, `ny`: where the object tries to be at the end of the tick.
    pub nx: f32,
    pub ny: f32,
    /// `lx`, `ly`: position at the start of the tick.
    pub lx: f32,
    pub ly: f32,
    pub current: Option<(i32, i32, i32)>,
    pub last: Option<(i32, i32, i32)>,
    pub alt_collide: bool,
    pub collided_n: bool,
    pub collided_s: bool,
    pub collided_w: bool,
    pub collided_e: bool,
    pub collided_this_frame: bool,
}

impl MovingObject {
    pub fn new(x: f32, y: f32, z: f32, current: Option<(i32, i32, i32)>) -> Self {
        MovingObject {
            x,
            y,
            z,
            nx: x,
            ny: y,
            lx: x,
            ly: y,
            current,
            last: None,
            alt_collide: false,
            collided_n: false,
            collided_s: false,
            collided_w: false,
            collided_e: false,
            collided_this_frame: false,
        }
    }
}

impl Scene {
    /// zombie.iso.IsoGridSquare.testCollideAdjacent for an object that is not a player: doors, windows and window
    /// frames first, then the square's collide matrix.
    pub fn test_collide_adjacent(&self, from: (i32, i32, i32), x: i32, y: i32, z: i32) -> bool {
        let Some(st) = self.state.as_ref() else {
            return true;
        };
        let Some(matrix) = self.collision.collide_matrix(from.0, from.1, from.2) else {
            return true;
        };
        if matrix == -1
            || !(-1..=1).contains(&x)
            || !(-1..=1).contains(&y)
            || !(-1..=1).contains(&z)
        {
            return true;
        }
        if let (Some(a), Some(b)) = (
            st.get_grid_square(from.0, from.1, from.2),
            st.get_grid_square(from.0 + x, from.1 + y, from.2 + z),
        ) {
            if a.test_collide_special_objects(&st.world.alias, Some(b), st)
                .is_some()
            {
                return true;
            }
        }
        (matrix >> ((x + 1) + (y + 1) * 3 + (z + 1) * 9)) & 1 == 1
    }

    /// zombie.iso.IsoMovingObject.findCurrentGridSquare: the square at the position, searching towards level 0.
    fn find_current_grid_square(&self, x: f32, y: f32, z: f32) -> Option<(i32, i32, i32)> {
        let st = self.state.as_ref()?;
        let (xi, yi, mut zi) = (x.floor() as i32, y.floor() as i32, z.floor() as i32);
        let sign = zi.signum();
        loop {
            if st.get_grid_square(xi, yi, zi).is_some() {
                return Some((xi, yi, zi));
            }
            zi -= sign;
            if zi == -sign {
                return None;
            }
        }
    }

    /// zombie.iso.IsoMovingObject.DoCollide: does the step to (nx, ny) cross into a blocked square, or does the
    /// "feeler" half a square ahead touch one? Sets the collided-direction flags; `favour` 1 keeps the north/south
    /// flags of a diagonal hit, 2 keeps east/west.
    fn do_collide(&self, o: &mut MovingObject, favour: i32) -> bool {
        let Some(st) = self.state.as_ref() else {
            return false;
        };
        // setCurrentSquareFromPosition(nx, ny) with the object's height
        o.current = self.current_square(o.nx, o.ny, o.z);
        if o.current.is_none() {
            if o.nx != o.lx || o.ny != o.ly {
                // Walking off the loaded world: stay where the tick started.
                o.collided_w |= o.nx < o.lx;
                o.collided_e |= o.nx > o.lx;
                o.collided_n |= o.ny < o.ly;
                o.collided_s |= o.ny > o.ly;
                o.nx = o.lx;
                o.ny = o.ly;
                o.current = o.last;
                o.collided_this_frame = true;
                return true;
            }
            return false;
        }
        let favour_flags = |o: &mut MovingObject| {
            if (o.collided_s || o.collided_n) && (o.collided_e || o.collided_w) {
                if favour == 2 {
                    o.collided_s = false;
                    o.collided_n = false;
                } else if favour == 1 {
                    o.collided_w = false;
                    o.collided_e = false;
                }
            }
        };
        if let (Some(cur), Some(last)) = (o.current, o.last) {
            if cur != last {
                if self.test_collide_adjacent(last, cur.0 - last.0, cur.1 - last.1, cur.2 - last.2)
                {
                    o.collided_e |= last.0 < cur.0;
                    o.collided_w |= last.0 > cur.0;
                    o.collided_s |= last.1 < cur.1;
                    o.collided_n |= last.1 > cur.1;
                    o.current = Some(last);
                    favour_flags(o);
                    o.collided_this_frame = true;
                    return true;
                }
                return false;
            }
        }
        if o.nx != o.lx || o.ny != o.ly {
            // getFeelerTile(feelersize = 0.5): Vector2.setLength normalises unless the length is already 1.
            let (mut vx, mut vy) = (o.nx - o.lx, o.ny - o.ly);
            let len_sq = vx * vx + vy * vy;
            if (1.0 - len_sq).abs() >= 1.0e-5 {
                if len_sq == 0.0 {
                    vx = 0.0;
                    vy = 0.0;
                } else {
                    let len = (len_sq as f64).sqrt() as f32;
                    vx /= len;
                    vy /= len;
                }
            }
            let feeler = Some((
                (o.x + vx * 0.5).floor() as i32,
                (o.y + vy * 0.5).floor() as i32,
                o.z.floor() as i32,
            ))
            .filter(|f| st.get_grid_square(f.0, f.1, f.2).is_some());
            if let (Some(f), Some(cur)) = (feeler, o.current) {
                if f != cur
                    && self.test_collide_adjacent(cur, f.0 - cur.0, f.1 - cur.1, f.2 - cur.2)
                {
                    if o.last.is_some() {
                        o.collided_e |= cur.0 < f.0;
                        o.collided_w |= cur.0 > f.0;
                        o.collided_s |= cur.1 < f.1;
                        o.collided_n |= cur.1 > f.1;
                        favour_flags(o);
                    }
                    o.collided_this_frame = true;
                    return true;
                }
            }
        }
        false
    }

    /// zombie.iso.IsoMovingObject.postupdate for an object that is not a character: resolves the wanted step
    /// (`nx`, `ny`) against walls, doors and windows, sliding along the axis that is free, slows down on stairs,
    /// then follows stairs. The caller sets `lx`/`ly` (as `update` does) and `nx`/`ny` before each call.
    pub fn postupdate(&self, o: &mut MovingObject) {
        self.postupdate_for(o, false)
    }

    /// `postupdate` with the character variants of `doStairs` and of the height snap (which a character skips).
    pub fn postupdate_for(&self, o: &mut MovingObject, is_character: bool) {
        if o.current.is_none() {
            // ensureOnTile does nothing for a non-player without a square; the rest still runs.
        }
        o.collided_this_frame = false;
        o.collided_n = false;
        o.collided_s = false;
        o.collided_w = false;
        o.collided_e = false;
        o.last = o.current;
        let (tx, ty) = (o.nx - o.x, o.ny - o.y);
        let tl = ((tx * tx + ty * ty) as f64).sqrt() as f32;
        if tl > 1.0 {
            o.nx = o.x + tx / tl;
            o.ny = o.y + ty / tl;
        }
        let (onx, ony) = (o.nx, o.ny);
        let pass = |s: &Scene, o: &mut MovingObject, first: i32, second: i32| -> bool {
            let mut did = false;
            s.do_collide(o, first);
            if o.collided_n || o.collided_s {
                o.ny = o.ly;
            }
            if o.collided_w || o.collided_e {
                o.nx = o.lx;
            }
            s.do_collide(o, second);
            if o.collided_n || o.collided_s {
                o.ny = o.ly;
                did = true;
            }
            if o.collided_w || o.collided_e {
                o.nx = o.lx;
                did = true;
            }
            did
        };
        let (a, b) = if o.alt_collide { (2, 1) } else { (1, 2) };
        let did = pass(self, o, a, b);
        o.alt_collide = !o.alt_collide;
        let len = (o.nx - o.lx).abs() + (o.ny - o.ly).abs();
        let (lnx, lny) = (o.nx, o.ny);
        o.nx = onx;
        o.ny = ony;
        if did {
            let (a, b) = if o.alt_collide { (2, 1) } else { (1, 2) };
            pass(self, o, a, b);
            if (o.nx - o.lx).abs() + (o.ny - o.ly).abs() < len {
                o.nx = lnx;
                o.ny = lny;
            }
        }
        if o.collided_this_frame {
            o.current = o.last;
        }
        let (mut dx, mut dy) = (o.nx - o.x, o.ny - o.y);
        // getGlobalMovementMod: three quarters of the speed on stairs.
        let on_stairs = o
            .current
            .and_then(|c| {
                self.state
                    .as_ref()
                    .and_then(|st| st.get_grid_square(c.0, c.1, c.2))
            })
            .is_some_and(|s| s.has_stairs());
        let movement_mod = if dx.abs() > 0.0 || dy.abs() > 0.0 {
            if on_stairs {
                0.75
            } else {
                1.0
            }
        } else {
            0.0
        };
        if dx.abs() > 0.01 || dy.abs() > 0.01 {
            dx *= movement_mod;
            dy *= movement_mod;
        }
        o.x += dx;
        o.y += dy;
        o.nx = o.x;
        o.ny = o.y;
        // doStairs uses the square set during collision and the previous one; handleSlopedSurface is not ported.
        if let (Some(_), Some(last)) = (o.current, o.last) {
            let mut m = Mover {
                x: o.x,
                y: o.y,
                z: o.z,
            };
            self.do_stairs_at(last, o.current, &mut m, is_character);
            o.x = m.x;
            o.y = m.y;
            o.z = m.z;
            o.nx = o.x;
            o.ny = o.y;
        }
        o.current = self.find_current_grid_square(o.x, o.y, o.z);
        // snapZToCurrentSquare
        if let Some(c) = o.current.filter(|_| !is_character) {
            o.z = o.z.min(c.2 as f32 + 0.99999);
        }
        if o.current.is_none() {
            if let Some(last) = o.last {
                o.current = Some(last);
                o.x = last.0 as f32 + 0.5;
                o.y = last.1 as f32 + 0.5;
                o.nx = o.x;
                o.ny = o.y;
                o.z = o.z.min(last.2 as f32 + 0.99999);
            }
        }
    }
}

/// `PerformanceSettings.fboRenderChunk`: true in the original's default settings; it shifts stair heights by 0.1.
const FBO_RENDER_CHUNK: bool = true;

impl Scene {
    /// zombie.iso.LosUtil.lineClear between two squares of one level, as a client computes it; a closed window or
    /// door in the way counts as seen through only where the original says so. `false` without engine state.
    pub fn line_clear(&self, from: (i32, i32), to: (i32, i32), z: i32) -> bool {
        let Some(st) = &self.state else { return false };
        line_clear(
            &st.world.sprites,
            &st.world.alias,
            st,
            from.0,
            from.1,
            z,
            to.0,
            to.1,
            z,
            false,
            10000,
            false,
        ) != TestResults::Blocked
    }

    /// Recompute instances and collision from the engine squares (after an object changed).
    pub fn rebuild(&mut self) {
        self.presentations.clear();
        let Some(mut state) = self.state.take() else {
            return;
        };
        let pending;
        {
            let SceneState {
                world,
                chunks,
                bounds,
                levels,
                missing,
                ..
            } = &state;
            let (x0, y0, x1, y1) = *bounds;
            let (zmin, zmax) = *levels;
            let missing = *missing;
            let atlas = &self.atlas;
            let mut visions: Vec<(i32, i32, i32, i32)> = Vec::new();

            let mut collision = Collision::default();
            {
                let mut grid = RegionGrid(std::collections::HashMap::new());
                for ch in chunks.iter() {
                    for z in ch.get_min_level()..=ch.get_max_level() {
                        for i in 0..64 {
                            if let Some(sq) = ch.get_grid_square(i % 8, i / 8, z) {
                                grid.0.insert((sq.x, sq.y, sq.z), sq);
                            }
                        }
                    }
                }
                for sq in grid.0.values() {
                    let (c, _path, vision) = sq.calculate_matrices(&world.alias, &grid);
                    collision.set_collide_matrix(sq.x, sq.y, sq.z, c);
                    visions.push((sq.x, sq.y, sq.z, vision));
                    let mut mask = 0u16;
                    for dy in -1..=1 {
                        for dx in -1..=1 {
                            let next = grid.get_grid_square(sq.x + dx, sq.y + dy, sq.z);
                            if (dx != 0 || dy != 0)
                                && sq
                                    .test_collide_special_objects(&world.alias, next, &grid)
                                    .is_some()
                            {
                                mask |= 1 << ((dx + 1) + (dy + 1) * 3);
                            }
                        }
                    }
                    collision.set_special_blocks(sq.x, sq.y, sq.z, mask);
                }
            }
            let (aw, ah) = (atlas.width as f32, atlas.height as f32);
            let mut instances = Vec::new();
            let mut level_ranges = vec![];
            let mut unresolved = missing;
            let mut meta = Vec::new();
            // Room of every square inside a room rect (also squares without tiles), numbered by the meta grid's room handle.
            let mut rooms = std::collections::HashMap::new();
            for cy in y0..=y1 {
                for cx in x0..=x1 {
                    let Some(cell) = world.meta_grid.get_cell_data(cx, cy) else {
                        continue;
                    };
                    for r in &cell.room_list {
                        let def = world.meta_grid.room(*r);
                        for q in &def.rects {
                            for yy in q.y..q.y + q.h {
                                for xx in q.x..q.x + q.w {
                                    rooms.insert((xx, yy, def.level), r.0 as i32);
                                }
                            }
                        }
                    }
                }
            }
            // The previous full-cell scan tested every tile in a 256x256 cell even
            // when only an 81-chunk streaming window was resident. Collect exactly
            // the loaded squares while preserving the original (z, y, x) painter order.
            let mut ordered = std::collections::BTreeMap::<i32, Vec<&IsoGridSquare>>::new();
            for ch in chunks {
                for z in ch.get_min_level()..=ch.get_max_level() {
                    for i in 0..64 {
                        if let Some(sq) = ch.get_grid_square(i % 8, i / 8, z) {
                            ordered.entry(z).or_default().push(sq);
                        }
                    }
                }
            }
            for squares in ordered.values_mut() {
                squares.sort_by_key(|sq| (sq.y, sq.x));
            }
            for z in zmin..=zmax {
                level_ranges.push(instances.len() as u32);
                if let Some(squares) = ordered.get(&z) {
                    for &sq in squares {
                        let (gx, gy) = (sq.x, sq.y);
                        let p = iso(gx as f32, gy as f32, z);
                        let (px, py) = (p[0] as i32, p[1] as i32);
                        let room = rooms.get(&(gx, gy, z)).copied().unwrap_or(-1);
                        for o in &sq.objects {
                            // IsoObject.render: the sprite, then renderOverlaySprites, then renderAttachedSprites.
                            for (k, spr) in o
                                .sprite
                                .iter()
                                .chain(overlay_sprite(o).iter())
                                .chain(o.attached_anim_sprite.iter().flatten())
                                .enumerate()
                            {
                                if k == 0 {
                                    let fl = sprite_collision_flags(world, *spr);
                                    if fl != 0 {
                                        collision.add(gx, gy, z, fl);
                                    }
                                }
                                let Some(s) = world
                                    .sprites
                                    .sprite(*spr)
                                    .name
                                    .as_ref()
                                    .and_then(|n| atlas.sprites.get(n))
                                else {
                                    unresolved += 1;
                                    continue;
                                };
                                let x = (px - s.orig_w / 2 + s.off_x) as f32;
                                // renderYOffset (table stacking) is in unscaled pixels; the original multiplies by Core.tileScale (2).
                                let y = (py + TILE_H - s.orig_h + s.off_y) as f32
                                    - o.render_y_offset * 2.0;
                                meta.push((gx, gy, z, room));
                                instances.push(Instance {
                                    rect: [x, y, s.w as f32, s.h as f32],
                                    uv: [
                                        s.ax as f32 / aw,
                                        s.ay as f32 / ah,
                                        (s.ax as i32 + s.w) as f32 / aw,
                                        (s.ay as i32 + s.h) as f32 / ah,
                                    ],
                                    layer: s.layer as f32,
                                });
                            }
                        }
                    }
                }
            }
            level_ranges.push(instances.len() as u32);
            self.instances = instances;
            self.level_ranges = level_ranges;
            self.unresolved = unresolved;
            self.collision = collision;
            self.meta = meta;
            self.rooms = rooms;
            pending = visions;
        }
        // zombie.iso.IsoGridSquare.visionMatrix, read by the ported line of sight.
        for (x, y, z, v) in pending {
            if let Some(sq) = state.square_mut(x, y, z) {
                sq.vision_matrix = v;
            }
        }
        self.state = Some(state);
    }

    /// The player's "interact" on doors: flips the nearest ordinary door within reach of (x, y) on level z
    /// (zombie.iso.objects.IsoDoor.ToggleDoorSilent: no locks, keys, sounds or obstruction checks yet; double and
    /// garage doors are left alone) and rebuilds the scene. Returns whether a door was toggled.
    pub fn toggle_door_near(&mut self, x: f32, y: f32, z: i32) -> bool {
        self.toggle_door_near_internal(x, y, z, true)
    }

    pub fn toggle_door_near_unrebuilt(&mut self, x: f32, y: f32, z: i32) -> bool {
        self.toggle_door_near_internal(x, y, z, false)
    }

    fn toggle_door_near_internal(&mut self, x: f32, y: f32, z: i32, rebuild: bool) -> bool {
        let Some(state) = self.state.as_mut() else {
            return false;
        };
        let SceneState { world, chunks, .. } = state;
        let mut best: Option<(f32, usize, i32, i32, usize)> = None;
        for (ci, ch) in chunks.iter().enumerate() {
            if ((ch.wx * 8 + 4) as f32 - x).abs() > 8.0 || ((ch.wy * 8 + 4) as f32 - y).abs() > 8.0
            {
                continue;
            }
            for i in 0..64 {
                let Some(sq) = ch.get_grid_square(i % 8, i / 8, z) else {
                    continue;
                };
                for (oi, o) in sq.objects.iter().enumerate() {
                    let ObjectKind::IsoDoor(d) = &o.kind else {
                        continue;
                    };
                    let props = o.sprite.map(|s| &world.sprites.sprite(s).properties);
                    if props.is_some_and(|p| {
                        p.has_str(&world.alias, "DoubleDoor")
                            || p.has_str(&world.alias, "GarageDoor")
                    }) {
                        continue;
                    }
                    // The door stands on the north or west edge of its square.
                    let (dx, dy) = if d.north {
                        (sq.x as f32 + 0.5, sq.y as f32)
                    } else {
                        (sq.x as f32, sq.y as f32 + 0.5)
                    };
                    let dist = ((dx - x).powi(2) + (dy - y).powi(2)).sqrt();
                    if dist <= 1.5 && best.is_none_or(|b| dist < b.0) {
                        best = Some((dist, ci, i % 8, i / 8, oi));
                    }
                }
            }
        }
        let Some((_, ci, lx, ly, oi)) = best else {
            return false;
        };
        if let Some(stream) = state.stream.as_mut() {
            stream.dirty.insert((chunks[ci].wx, chunks[ci].wy));
        }
        if let Some(sq) = chunks[ci].get_grid_square_mut(lx, ly, z) {
            sq.objects[oi].toggle_door_silent();
            sq.recalc_properties(&world.sprites);
        }
        self.stream_touched.insert((chunks[ci].wx, chunks[ci].wy));
        if rebuild { self.rebuild(); }
        true
    }
}

impl Scene {
    /// Debug overlay: outline dots (5x5 px, atlas block `name`) along the edges of zones, sampled every `step` tiles,
    /// limited to zones that touch the tile box `bx = (x0, y0, x1, y1)`. Rectangles use x..x+w, y..y+h; polygons use their points.
    pub fn zone_dots(
        &self,
        name: &str,
        zones: &[zomboid_content::zones::Zone],
        kind: &str,
        bx: (i32, i32, i32, i32),
        z: i32,
        step: f32,
    ) -> Vec<Instance> {
        let s = self.atlas.sprites[name];
        let (aw, ah) = (self.atlas.width as f32, self.atlas.height as f32);
        let uv = [
            (s.ax as f32 + 1.0) / aw,
            (s.ay as f32 + 1.0) / ah,
            (s.ax as f32 + 4.0) / aw,
            (s.ay as f32 + 4.0) / ah,
        ];
        let mut out = vec![];
        let mut seg = |a: (f32, f32), b: (f32, f32)| {
            let len = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt();
            let n = (len / step).ceil().max(1.0) as i32;
            for i in 0..=n {
                let t = i as f32 / n as f32;
                let (x, y) = (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t);
                if x < bx.0 as f32 || x > bx.2 as f32 || y < bx.1 as f32 || y > bx.3 as f32 {
                    continue;
                }
                let p = iso(x, y, z);
                out.push(Instance {
                    rect: [p[0] - 2.5, p[1] - 2.5, 5.0, 5.0],
                    uv,
                    layer: s.layer as f32,
                });
            }
        };
        for q in zones.iter().filter(|q| q.kind == kind && q.z == z) {
            let pts: Vec<(f32, f32)> = if q.points.is_empty() {
                let (x, y, w, h) = (q.x as f32, q.y as f32, q.w as f32, q.h as f32);
                vec![(x, y), (x + w, y), (x + w, y + h), (x, y + h)]
            } else {
                q.points.iter().map(|p| (p.0 as f32, p.1 as f32)).collect()
            };
            for i in 0..pts.len() {
                seg(pts[i], pts[(i + 1) % pts.len()]);
            }
        }
        out
    }
}

impl Scene {
    /// Visibility mask measured against the real game (night, player indoors): squares outside the player's room are black.
    /// Keeps instances whose square is in the player's room, plus room-less squares touching it (walls/doors), at every level.
    /// Does nothing if the player's square has no room. Light spill from sources (door cone) is NOT modelled.
    /// zombie.iso.IsoCell.SetCutawayRoomsForPlayer (read from the class file, see docs/COMPATIBILITY.md): the real game shows the player's room
    /// plus the rooms at (x+-1.5, y+-1.5) that the player can see and that lie in front of the facing direction. `probes` adds the four
    /// probe squares WITHOUT the can-see and facing tests (the port has neither yet), so it can over-reveal.
    pub fn retain_rooms(
        &mut self,
        px: f32,
        py: f32,
        z: i32,
        probes: bool,
        los: Option<f32>,
    ) -> Option<i32> {
        let rid = *self.rooms.get(&(px.floor() as i32, py.floor() as i32, z))?;
        let mut set = vec![rid];
        if probes {
            for (dx, dy) in [(-1.5f32, -1.5f32), (1.5, -1.5), (-1.5, 1.5), (1.5, 1.5)] {
                if let Some(&r) =
                    self.rooms
                        .get(&((px + dx).floor() as i32, (py + dy).floor() as i32, z))
                {
                    if r >= 0 && !set.contains(&r) {
                        set.push(r);
                    }
                }
            }
        }
        // "Cutaway" rule measured on two real frames: a room-less square is kept only when the player's room lies on its +x or +y side,
        // i.e. walls on the north/west edges. Walls on the south/east edges (facing the camera) are not drawn by the real game.
        let near = |x: i32, y: i32, zz: i32| {
            [(0, 0), (1, 0), (0, 1)]
                .iter()
                .any(|&(dx, dy)| matches!(self.rooms.get(&(x + dx, y + dy, zz)), Some(r) if set.contains(r)))
        };
        // Squares within `radius` the player has line of sight to: the ported LosUtil.lineClear (the radius itself
        // and what the original does with the result when drawing are still approximations).
        let mut seen = std::collections::HashSet::new();
        if let Some(rad) = los {
            let r = rad.ceil() as i32;
            for x in (px.floor() as i32 - r)..=(px.floor() as i32 + r) {
                for y in (py.floor() as i32 - r)..=(py.floor() as i32 + r) {
                    let (cx, cy) = (x as f32 + 0.5 - px, y as f32 + 0.5 - py);
                    let seen_sq = if self.state.is_some() {
                        self.line_clear((px.floor() as i32, py.floor() as i32), (x, y), z)
                    } else {
                        self.collision.los((px, py), (x, y), z)
                    };
                    if cx * cx + cy * cy <= rad * rad && seen_sq {
                        seen.insert((x, y));
                    }
                }
            }
        }
        let keep: Vec<bool> = self
            .meta
            .iter()
            .map(|&(x, y, zz, r)| {
                set.contains(&r) || (r < 0 && near(x, y, zz)) || (zz == z && seen.contains(&(x, y)))
            })
            .collect();
        let (mut inst, mut meta) = (vec![], vec![]);
        for (i, k) in keep.iter().enumerate() {
            if *k {
                inst.push(self.instances[i]);
                meta.push(self.meta[i]);
            }
        }
        // rebuild level starts from the surviving instances' z (instances are already ordered by level)
        let (zmin, n) = (self.min_level, self.level_ranges.len() - 1);
        let starts: Vec<u32> = (0..=n)
            .map(|lv| {
                meta.iter()
                    .take_while(|m| ((m.2 - zmin) as usize) < lv)
                    .count() as u32
            })
            .collect();
        self.instances = inst;
        self.meta = meta;
        self.level_ranges = starts;
        self.clear = [0.0, 0.0, 0.0];
        Some(rid)
    }
}

#[cfg(test)]
mod collision_oracle_tests {
    use super::*;
    use zomboid_content::tiledefs::Props;

    #[test]
    #[ignore = "real-content acceptance: loads a complete cell and texture atlas"]
    fn spawn_house_steps_follow_original_collide_matrices() -> anyhow::Result<()> {
        // collideMatrix of four squares of the Rosewood spawn house, printed by the ORIGINAL through
        // tests/original_probe/CollideProbe.java (2026-11-13). Bit (dx+1) + (dy+1)*3 + 9 set = step blocked.
        let observed = [
            ((8303, 11689), 0x7fc2fff),
            ((8300, 11690), 0x7fe69ff),
            ((8301, 11692), 0x7feffff),
            ((8304, 11688), 0x7fcbfff),
        ];
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let scene = load_region(&root, 32, 45, 32, 45)?;
        for ((x, y), matrix) in observed {
            for dy in -1..=1 {
                for dx in -1..=1 {
                    if dx == 0 && dy == 0 {
                        continue;
                    }
                    let blocked = (matrix >> ((dx + 1) + (dy + 1) * 3 + 9)) & 1 == 1;
                    assert_eq!(
                        !scene.collision.can_step((x, y), (x + dx, y + dy), 0),
                        blocked,
                        "from {x},{y} by {dx},{dy}"
                    );
                }
            }
        }
        Ok(())
    }

    #[test]
    #[ignore = "real-content acceptance: loads a complete cell, the player's own save and the texture atlas"]
    fn the_players_save_supplies_its_chunks_and_their_overlay_sprites() -> anyhow::Result<()> {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let save = root.join("zomboid-rs/profile/Saves/Sandbox/2026-11-07_22-34-13");
        if !save.join("map_worldgen.bin").exists() {
            eprintln!("skipped: no save at {}", save.display());
            return Ok(());
        }
        // Chunk files of cell 32,45 in the save folder (chunks 1024..1056 x 1440..1472).
        let mut in_save = 0;
        for wx in 1024..1056 {
            for wy in 1440..1472 {
                in_save += usize::from(save.join(format!("map/{wx}/{wy}.bin")).exists());
            }
        }
        let scene = load_region_from_save(&root, Some(&save), 32, 45, 32, 45)?;
        let state = scene.state.as_ref().ok_or_else(|| anyhow!("no state"))?;
        assert!(in_save > 0);
        assert_eq!(state.loaded_from_save, in_save, "every chunk file loads");
        // Overlay sprites of two bookshelves in the spawn house, as the ORIGINAL's IsoChunk.LoadFromDisk leaves them
        // (tests/original_probe/ChunkSaveProbe.java on this save, 2026-11-14).
        for ((x, y), base, overlay) in [
            (
                (8304, 11688),
                "furniture_shelving_01_40",
                "books&misc_01_62",
            ),
            (
                (8305, 11693),
                "furniture_shelving_01_41",
                "books&misc_02_63",
            ),
        ] {
            let sq = state
                .get_grid_square(x, y, 0)
                .ok_or_else(|| anyhow!("no square {x},{y}"))?;
            let name = |s: SpriteRef| state.world.sprites.sprite(s).name.clone();
            let found = sq.objects.iter().find_map(|o| {
                (o.sprite.and_then(name).as_deref() == Some(base))
                    .then(|| overlay_sprite(o).and_then(name))
            });
            assert_eq!(found, Some(Some(overlay.to_string())), "{x},{y}");
            assert!(
                scene.atlas.sprites.contains_key(overlay),
                "{overlay} in atlas"
            );
        }
        Ok(())
    }

    #[test]
    #[ignore = "real-content acceptance: loads a complete cell and texture atlas"]
    fn opening_the_spawn_house_door_lets_the_player_through() -> anyhow::Result<()> {
        // (8288, 11702, 0) holds the closed west-edge door fixtures_doors_02_4 of the house's back room
        // (ChunkObjectsProbe output of the original, chunk 1036,1462, square 0,6).
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let mut scene = load_region(&root, 32, 45, 32, 45)?;
        let (inside, outside) = ((8288, 11702), (8287, 11702));
        assert!(
            !scene.collision.can_step(inside, outside, 0),
            "closed door blocks"
        );
        assert!(scene.toggle_door_near(8288.4, 11702.5, 0));
        assert!(
            scene.collision.can_step(inside, outside, 0),
            "open door lets through"
        );
        assert!(scene.collision.can_step(outside, inside, 0));
        assert!(scene.toggle_door_near(8288.4, 11702.5, 0));
        assert!(
            !scene.collision.can_step(inside, outside, 0),
            "closed again"
        );
        Ok(())
    }

    #[test]
    #[ignore = "real-content acceptance: loads a complete cell and texture atlas"]
    fn walking_the_staircase_changes_the_floor() -> anyhow::Result<()> {
        // A north-facing staircase of the original map: bottom (8118, 11614, 0), middle 11613, top 11612
        // (StairsProbe output of the original). Walk up from the south, then back down.
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let scene = load_region(&root, 31, 45, 31, 45)?;
        let mut m = Mover {
            x: 8118.5,
            y: 11615.5,
            z: 0.0,
        };
        let mut heights = vec![m.z];
        for _ in 0..60 {
            assert!(scene.move_character(&mut m, 0.0, -0.1), "blocked at {m:?}");
            heights.push(m.z);
        }
        assert!(
            heights.windows(2).all(|w| w[1] >= w[0]),
            "height never drops on the way up: {heights:?}"
        );
        assert_eq!(m.z, 1.0, "arrived on the upper floor at {m:?}");
        assert_eq!(scene.current_square(m.x, m.y, m.z), Some((8118, 11610, 1)));
        for _ in 0..60 {
            assert!(
                scene.move_character(&mut m, 0.0, 0.1),
                "blocked on the way down at {m:?}"
            );
        }
        assert_eq!(scene.current_square(m.x, m.y, m.z), Some((8118, 11615, 0)));
        // doStairs leaves the height of the last point on the bottom stair; the original does the same (MoverProbe).
        assert!(m.z < 0.1, "{m:?}");
        Ok(())
    }

    #[test]
    #[ignore = "real-content acceptance: loads a complete cell and texture atlas"]
    fn real_water_square_is_wired_into_loaded_scene() -> anyhow::Result<()> {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
        // (1, 4803, 0) holds the water tile blends_natural_02_0: found by scanning the lotpack of cell 0_18 (2026-11-11).
        let (x, y) = (1, 4803);
        let scene = load_region(&root, 0, 18, 0, 18)?;
        for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            assert!(
                !scene.collision.can_step((x, y), (x + dx, y + dy), 0),
                "water source at {x},{y}, direction {dx},{dy}"
            );
        }
        println!("real water square: {x},{y},0");
        Ok(())
    }

    #[test]
    fn real_water_tile_uses_original_measured_collision_flags() -> anyhow::Result<()> {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let defs = TileDefs::load(&root)?;
        // The installed newtiledefinitions.tiles.txt declares these three
        // properties together. WaterProbe records blocked entry and exit.
        let tile = "blends_natural_02_0";
        for prop in ["water", "solidtrans", "solidfloor"] {
            assert!(defs.has(tile, prop), "{tile} missing {prop}");
        }
        let mut c = Collision::default();
        c.add(1, 0, 0, tile_collision_flags(&defs, tile));
        assert!(!c.can_step((0, 0), (1, 0), 0));
        assert!(!c.can_step((1, 0), (0, 0), 0));
        Ok(())
    }

    #[test]
    fn water_solidtrans_window_destination_blocks_entry() {
        // WaterWindowProbe: eastward cases 48 and 56 differ only by water.
        let line = include_str!("../../../tests/golden/collision_water_window.txt")
            .lines()
            .next()
            .expect("east oracle");
        let (direction, bits) = line.split_once(':').expect("oracle line");
        assert_eq!(direction, "1,0");
        let mut defs = TileDefs::default();
        for name in ["water", "solidtrans", "windowN"] {
            defs.tiles
                .insert(name.into(), Props::from([(name.into(), String::new())]));
        }
        for mask in [48usize, 56] {
            let mut c = Collision::default();
            for (bit, name) in ["water", "solidtrans", "windowN"].iter().enumerate() {
                if mask & (1 << (bit + 3)) != 0 {
                    c.add(1, 0, 0, tile_collision_flags(&defs, name));
                }
            }
            assert_eq!(
                !c.can_step((0, 0), (1, 0), 0),
                bits.as_bytes()[mask] == b'1'
            );
        }
    }

    #[test]
    fn water_and_window_on_both_squares_match_original() {
        let names = ["water", "solidtrans", "windowN"];
        let mut defs = TileDefs::default();
        for name in names {
            defs.tiles
                .insert(name.into(), Props::from([(name.into(), String::new())]));
        }
        let mut count = 0;
        for line in include_str!("../../../tests/golden/collision_water_window.txt").lines() {
            let (direction, bits) = line.split_once(':').expect("oracle line");
            let (dx, dy) = direction.split_once(',').expect("direction");
            let to = (
                dx.parse::<i32>().expect("dx"),
                dy.parse::<i32>().expect("dy"),
            );
            assert_eq!(bits.len(), 64);
            for (mask, expected) in bits.bytes().enumerate() {
                let mut c = Collision::default();
                for (square, shift) in [((0, 0), 0), (to, 3)] {
                    for (bit, name) in names.iter().enumerate() {
                        if mask & (1 << (shift + bit)) != 0 {
                            c.add(square.0, square.1, 0, tile_collision_flags(&defs, name));
                        }
                    }
                }
                assert_eq!(
                    !c.can_step((0, 0), to, 0),
                    expected == b'1',
                    "{direction} mask {mask}"
                );
                count += 1;
            }
        }
        assert_eq!(count, 256);
    }

    #[test]
    fn adjacent_window_and_hoppable_flags_match_original() {
        let names = [
            "solidtrans",
            "windowN",
            "windowW",
            "WindowN",
            "WindowW",
            "HoppableN",
            "HoppableW",
        ];
        let mut defs = TileDefs::default();
        for name in names {
            defs.tiles
                .insert(name.into(), Props::from([(name.into(), String::new())]));
        }
        let mut count = 0;
        for line in include_str!("../../../tests/golden/collision_adjacent_window.txt").lines() {
            let (input, expected) = line.split_once(':').expect("oracle line");
            let values: Vec<i32> = input
                .split(',')
                .map(|s| s.parse().expect("oracle integer"))
                .collect();
            assert_eq!(values.len(), 3);
            let (x, y, mask) = (values[0], values[1], values[2]);
            let mut c = Collision::default();
            c.add(0, 0, 0, tile_collision_flags(&defs, "solidtrans"));
            for (bit, name) in names.iter().enumerate() {
                if mask & (1 << bit) != 0 {
                    c.add(x, y, 0, tile_collision_flags(&defs, name));
                }
            }
            assert_eq!(!c.can_step((-1, 0), (0, 0), 0), expected == "1", "{input}");
            count += 1;
        }
        assert_eq!(count, 1152);
    }

    #[test]
    fn adjacent_window_flags_match_original_all_directions() {
        let names = [
            "solidtrans",
            "windowN",
            "windowW",
            "WindowN",
            "WindowW",
            "HoppableN",
            "HoppableW",
        ];
        let mut defs = TileDefs::default();
        for name in names {
            defs.tiles
                .insert(name.into(), Props::from([(name.into(), String::new())]));
        }
        let mut count = 0;
        for line in
            include_str!("../../../tests/golden/collision_adjacent_window_directions.txt").lines()
        {
            let (input, expected) = line.split_once(':').expect("oracle line");
            let values: Vec<i32> = input
                .split(',')
                .map(|s| s.parse().expect("oracle integer"))
                .collect();
            assert_eq!(values.len(), 5);
            let (sx, sy, x, y, mask) = (values[0], values[1], values[2], values[3], values[4]);
            let mut c = Collision::default();
            c.add(0, 0, 0, tile_collision_flags(&defs, "solidtrans"));
            for (bit, name) in names.iter().enumerate() {
                if mask & (1 << bit) != 0 {
                    c.add(x, y, 0, tile_collision_flags(&defs, name));
                }
            }
            assert_eq!(!c.can_step((sx, sy), (0, 0), 0), expected == "1", "{input}");
            count += 1;
        }
        assert_eq!(count, 9216);
    }

    #[test]
    fn window_and_solidtrans_match_original_square_cases() {
        let names = [
            "solidtrans",
            "windowN",
            "windowW",
            "WindowN",
            "WindowW",
            "collideN",
            "collideW",
            "transparentN",
            "transparentW",
        ];
        let mut defs = TileDefs::default();
        for name in names {
            defs.tiles
                .insert(name.into(), Props::from([(name.into(), String::new())]));
        }
        let mut checked = 0;
        for line in include_str!("../../../tests/golden/collision_window_solid.txt").lines() {
            let (direction, bits) = line.split_once(':').expect("oracle line");
            let (dx, dy) = direction.split_once(',').expect("oracle direction");
            let to = (
                dx.parse::<i32>().expect("dx"),
                dy.parse::<i32>().expect("dy"),
            );
            assert_eq!(bits.len(), 512);
            for (mask, expected) in bits.bytes().enumerate() {
                let mut collision = Collision::default();
                for (bit, name) in names.iter().enumerate() {
                    if mask & (1 << bit) != 0 {
                        collision.add(to.0, to.1, 0, tile_collision_flags(&defs, name));
                    }
                }
                assert_eq!(
                    !collision.can_step((0, 0), to, 0),
                    expected == b'1',
                    "direction={direction} mask={mask}"
                );
                checked += 1;
            }
        }
        assert_eq!(checked, 4096);
    }

    #[test]
    fn mixed_solid_and_window_flags_match_original_square_cases() {
        let names = [
            "solid",
            "solidtrans",
            "windowN",
            "windowW",
            "WindowN",
            "WindowW",
            "collideN",
            "collideW",
            "transparentN",
            "transparentW",
        ];
        let mut defs = TileDefs::default();
        for name in names {
            defs.tiles
                .insert(name.into(), Props::from([(name.into(), String::new())]));
        }
        let mut checked = 0;
        for line in include_str!("../../../tests/golden/collision_mixed_solid_window.txt").lines() {
            let (direction, bits) = line.split_once(':').expect("oracle line");
            let (dx, dy) = direction.split_once(',').expect("oracle direction");
            let to = (
                dx.parse::<i32>().expect("dx"),
                dy.parse::<i32>().expect("dy"),
            );
            assert_eq!(bits.len(), 1024);
            for (mask, expected) in bits.bytes().enumerate() {
                let mut collision = Collision::default();
                for (bit, name) in names.iter().enumerate() {
                    if mask & (1 << bit) != 0 {
                        collision.add(to.0, to.1, 0, tile_collision_flags(&defs, name));
                    }
                }
                assert_eq!(
                    !collision.can_step((0, 0), to, 0),
                    expected == b'1',
                    "direction={direction} mask={mask}"
                );
                checked += 1;
            }
        }
        assert_eq!(checked, 8192);
    }

    #[test]
    fn water_and_solidtrans_match_original_cardinal_samples() {
        // The original-side fixture uses water, solidtrans and solidfloor on
        // the source and destination; model each property as a separate tile.
        let mut defs = TileDefs::default();
        for name in ["water", "solidtrans", "solidfloor"] {
            defs.tiles
                .insert(name.into(), Props::from([(name.into(), String::new())]));
        }
        let mut checked = 0;
        for line in include_str!("../../../tests/golden/collision_water.txt").lines() {
            let (direction, bits) = line.split_once(':').expect("oracle line");
            let (dx, dy) = direction.split_once(',').expect("oracle direction");
            let to = (
                dx.parse::<i32>().expect("dx"),
                dy.parse::<i32>().expect("dy"),
            );
            assert_eq!(bits.len(), 64);
            for (mask, expected) in bits.bytes().enumerate() {
                let mut collision = Collision::default();
                for (square, shift) in [((0, 0), 0), (to, 3)] {
                    for (bit, name) in ["water", "solidtrans", "solidfloor"]
                        .into_iter()
                        .enumerate()
                    {
                        if mask & (1 << (shift + bit)) != 0 {
                            collision.add(square.0, square.1, 0, tile_collision_flags(&defs, name));
                        }
                    }
                }
                assert_eq!(
                    !collision.can_step((0, 0), to, 0),
                    expected == b'1',
                    "direction={direction} mask={mask}"
                );
                checked += 1;
            }
        }
        assert_eq!(checked, 256);
    }

    #[test]
    fn static_square_flags_match_original_adjacent_samples() {
        // Expected blocked states came from tests/original_probe/CollisionProbe.java.
        // Each pair is forward/reverse, with the flags on the eastern or southern square.
        type Case = (&'static [&'static str], (bool, bool), (bool, bool));
        let cases: &[Case] = &[
            (&[], (false, false), (false, false)),
            (&["solid"], (true, false), (true, false)),
            (&["solidtrans"], (true, false), (true, false)),
            (&["collideW"], (true, true), (false, false)),
            (&["collideN"], (false, false), (true, true)),
            (&["WallW"], (false, false), (false, false)),
            (&["WallN"], (false, false), (false, false)),
            (&["doorW"], (false, false), (false, false)),
            (&["windowW"], (false, false), (false, false)),
            (&["collideW", "doorW"], (true, true), (false, false)),
            (&["collideW", "windowW"], (true, true), (false, false)),
            (&["WallW", "collideW"], (true, true), (false, false)),
            (&["WallN", "collideN"], (false, false), (true, true)),
        ];
        for (properties, east, south) in cases {
            let mut defs = TileDefs::default();
            let props: Props = properties
                .iter()
                .map(|p| (p.to_string(), String::new()))
                .collect();
            defs.tiles.insert("probe".into(), props);
            for (to, expected) in [((1, 0), east), ((0, 1), south)] {
                let mut collision = Collision::default();
                collision.add(to.0, to.1, 0, tile_collision_flags(&defs, "probe"));
                let result = (
                    !collision.can_step((0, 0), to, 0),
                    !collision.can_step(to, (0, 0), 0),
                );
                assert_eq!(result, *expected, "properties={properties:?}, to={to:?}");
            }
        }
    }
}

#[cfg(test)]
mod world_stream_unit_tests {
    use super::*;

    #[test]
    fn clean_chunk_cache_is_bounded_but_changed_chunks_are_never_dropped() {
        let mut stream = WorldStreamState {
            root: std::path::PathBuf::new(), save: None,
            rand: RandAbstract::new(java::Random::new(1)), locked_houses: 6,
            window: ChunkWindow { cx: 0, cy: 0, radius: 4 },
            persist_worldgen: false, zones: Vec::new(),
            lotpack_cache: None, texture_catalog: None,
            parked: HashMap::new(), parking_order: VecDeque::new(), dirty: HashSet::new(),
            pending_save: HashSet::new(),
        };
        stream.dirty.insert((0, 0));
        for x in 0..300 {
            stream.park(IsoChunk::new(x, 0));
        }
        assert!(stream.parked.contains_key(&(0, 0)), "a changed chunk must survive eviction");
        assert!(stream.parked.len() <= 257, "the clean cache must stay bounded");
        assert!(stream.parked.contains_key(&(299, 0)), "the newest chunk is resident");
    }
}

#[cfg(test)]
mod async_stream_parity_tests {
    use super::*;
    use zomboid_engine::zombie::iso::sprite::iso_sprite_manager::IsoSpriteManager;

    /// Convert sprite references to common handles keyed by the actual sprite
    /// names. Only the test uses this: production translation is done in
    /// `accept_stream_snapshot` and must remain unchanged.
    fn canonicalize_chunk_sprites(
        chunk: &mut IsoChunk,
        sprites: &IsoSpriteManager,
        canonical: &mut HashMap<String, SpriteRef>,
    ) {
        let mut remap = |value: &mut Option<SpriteRef>| {
            let Some(reference) = *value else { return; };
            let sprite = sprites.sprite(reference);
            let key = sprite.name.as_ref().or(sprite.texture_name.as_ref())
                .cloned().unwrap_or_else(|| format!("unnamed-id:{}", sprite.id));
            let id = if let Some(id) = canonical.get(&key) { *id } else {
                let id = SpriteRef(canonical.len() as u32);
                canonical.insert(key, id);
                id
            };
            *value = Some(id);
        };
        for z in chunk.get_min_level()..=chunk.get_max_level() {
            for i in 0..64 {
                let Some(square) = chunk.get_grid_square_mut(i % 8, i / 8, z) else { continue; };
                for object in &mut square.objects {
                    remap(&mut object.sprite);
                    if let Some(attached) = &mut object.attached_anim_sprite {
                        for sprite in attached {
                            let mut value = Some(*sprite);
                            remap(&mut value);
                            if let Some(mapped) = value { *sprite = mapped; }
                        }
                    }
                    match &mut object.kind {
                        ObjectKind::IsoDoor(door) => {
                            remap(&mut door.open_sprite);
                            remap(&mut door.closed_sprite);
                        }
                        ObjectKind::IsoWindow(window) => {
                            remap(&mut window.open_sprite);
                            remap(&mut window.closed_sprite);
                            remap(&mut window.smashed_sprite);
                            remap(&mut window.glass_removed_sprite);
                        }
                        ObjectKind::IsoCurtain(curtain) => {
                            remap(&mut curtain.open_sprite);
                            remap(&mut curtain.closed_sprite);
                        }
                        _ => {}
                    }
                    if let Some(save) = &mut object.save {
                        remap(&mut save.overlay_sprite);
                        for splat in &mut save.wall_blood_splats {
                            remap(&mut splat.sprite);
                        }
                        if let zomboid_engine::zombie::iso::chunk_save::ClassSave::Barbecue {
                            normal, no_tank, ..
                        } = &mut save.class {
                            remap(normal);
                            remap(no_tank);
                        }
                    }
                }
            }
        }
    }

    /// Real-game regression check for the batching adaptation. The synchronous
    /// path was previously checked against the Java WorldGen oracles; here both
    /// update schedules must end with identical resident chunk objects/states.
    /// Run explicitly with DEADOXIDE_GAME_ROOT set to the installed game folder.
    #[test]
    #[ignore = "requires the player's installed Project Zomboid data"]
    fn streamed_two_chunk_batches_match_single_batch() -> Result<()> {
        let root = std::path::PathBuf::from(std::env::var("DEADOXIDE_GAME_ROOT")
            .map_err(|_| anyhow!("set DEADOXIDE_GAME_ROOT to the installed game directory"))?);
        let (x, y) = (41.0 * 256.0 + 116.0, 39.0 * 256.0 + 116.0);
        let mut synchronous = load_streaming(&root, None, x, y, 2, false)?;
        let mut batched = load_streaming(&root, None, x, y, 2, false)?;
        let dest = (x + 8.5, y);
        synchronous.stream_around(dest.0, dest.1)?;
        for _ in 0..32 {
            if batched.stream_around_budget(dest.0, dest.1, 2)?.is_none() {
                break;
            }
        }
        let sync_chunks = &synchronous.state.as_ref()
            .ok_or_else(|| anyhow!("missing synchronous state"))?.chunks;
        let batch_chunks = &batched.state.as_ref()
            .ok_or_else(|| anyhow!("missing batched state"))?.chunks;
        let a: HashMap<_, _> = sync_chunks.iter().map(|c| ((c.wx, c.wy), c)).collect();
        let b: HashMap<_, _> = batch_chunks.iter().map(|c| ((c.wx, c.wy), c)).collect();
        assert_eq!(a, b, "incremental postprocessing changed a chunk");
        assert_eq!(synchronous.instances.len(), batched.instances.len());
        Ok(())
    }

    /// This is the regression that protects the expensive render/collision
    /// refactor: the final chunk-based scene must be byte-for-byte identical to
    /// the old full rebuild, including its global (z, y, x) painter order.
    #[test]
    #[ignore = "requires the player's installed Project Zomboid data"]
    fn streamed_incremental_presentation_matches_full_rebuild() -> Result<()> {
        let root = std::path::PathBuf::from(std::env::var("DEADOXIDE_GAME_ROOT")
            .map_err(|_| anyhow!("set DEADOXIDE_GAME_ROOT to the installed game directory"))?);
        let (x, y) = (41.0 * 256.0 + 116.0, 39.0 * 256.0 + 116.0);
        let mut worker = load_streaming(&root, None, x, y, 2, false)?;
        let mut main = load_streaming(&root, None, x, y, 2, false)?;
        let mut full = load_streaming(&root, None, x, y, 2, false)?;
        let mut cache = StreamSnapshotCache::default();

        let assert_equal = |a: &Scene, b: &Scene| {
            assert_eq!(a.level_ranges, b.level_ranges, "z-range painter order");
            assert_eq!(a.min_level, b.min_level);
            assert_eq!(a.unresolved, b.unresolved);
            assert_eq!(a.meta, b.meta, "instance metadata order");
            assert_eq!(a.rooms, b.rooms, "room cache changed");
            assert!(bytemuck::cast_slice::<Instance, u8>(&a.instances)
                == bytemuck::cast_slice::<Instance, u8>(&b.instances),
                "instance bytes differ from the original full rebuild");
            for chunk in &b.state.as_ref().expect("full state").chunks {
                for z in chunk.get_min_level()..=chunk.get_max_level() {
                    for i in 0..64 {
                        let (tx, ty) = (chunk.wx * 8 + i % 8, chunk.wy * 8 + i / 8);
                        assert_eq!(a.collision.collide_matrix(tx, ty, z),
                            b.collision.collide_matrix(tx, ty, z),
                            "collide matrix at {tx},{ty},{z}");
                        for (dx, dy) in [(1, 0), (0, 1), (-1, 0), (0, -1)] {
                            assert_eq!(a.collision.can_step((tx, ty), (tx + dx, ty + dy), z),
                                b.collision.can_step((tx, ty), (tx + dx, ty + dy), z),
                                "movement collision at {tx},{ty},{z}");
                        }
                    }
                }
            }
        };
        main.accept_stream_snapshot(worker.stream_snapshot(&mut cache, StreamReport {
            atlas_changed: true, ..Default::default()
        }, true)?)?;
        // Different initial packing ties are possible in two independently
        // created scenes; compare with the same atlas that the worker sent.
        full.atlas = main.atlas.clone();
        full.rebuild();
        assert_equal(&main, &full);
        let unchanged = worker.stream_snapshot(&mut cache, StreamReport::default(), false)?;
        assert!(unchanged.chunks.is_empty() && unchanged.presentations.is_empty(),
            "unchanged chunks must not be republished");

        let destination = (x + 8.5, y);
        full.stream_around(destination.0, destination.1)?;
        let mut loaded = 0;
        for _ in 0..32 {
            let Some(report) = worker.stream_around_budget_unrebuilt(
                destination.0, destination.1, 1)? else { break; };
            loaded += report.loaded;
            let atlas_changed = report.atlas_changed;
            let snapshot = worker.stream_snapshot(&mut cache, report, atlas_changed)?;
            assert!(snapshot.presentations.len() <= 25,
                "one new chunk should affect only resident neighbour presentations");
            main.accept_stream_snapshot(snapshot)?;
        }
        assert!(loaded > 0, "streaming test never loaded a chunk");
        full.atlas = main.atlas.clone();
        full.rebuild();
        assert_equal(&main, &full);
        let foreground = main.state.as_ref().expect("main state");
        let reference = full.state.as_ref().expect("full state");
        assert_eq!(foreground.chunks.len(), reference.chunks.len(),
            "number of resident native chunks");
        let mut canonical = HashMap::new();
        for left in &foreground.chunks {
            let key = (left.wx, left.wy);
            let right = reference.chunk_index.get(&key)
                .map(|&index| &reference.chunks[index])
                .unwrap_or_else(|| panic!("missing native reference chunk at {key:?}"));
            // SpriteRef is an index in each IsoSpriteManager. The foreground,
            // worker and reference own independent sprite managers: equal sprite
            // names can have different indices. Canonicalise *only* these handles
            // before comparing the complete native IsoChunk fields.
            let mut left = left.clone();
            let mut right = right.clone();
            canonicalize_chunk_sprites(&mut left, &foreground.world.sprites, &mut canonical);
            canonicalize_chunk_sprites(&mut right, &reference.world.sprites, &mut canonical);
            if left != right {
                if left.save != right.save {
                    panic!("native chunk {key:?}: save/WorldGen state differs: {:?} vs {:?}",
                        left.save, right.save);
                }
                if left.erosion != right.erosion || left.loaded != right.loaded {
                    panic!("native chunk {key:?}: erosion/loaded state differs");
                }
                for z in left.get_min_level().min(right.get_min_level())
                    ..=left.get_max_level().max(right.get_max_level()) {
                    for i in 0..64 {
                        let (lx, ly) = (i % 8, i / 8);
                        let (a, b) = (left.get_grid_square(lx, ly, z),
                            right.get_grid_square(lx, ly, z));
                        if a != b {
                            let (x, y) = (key.0 * 8 + lx, key.1 * 8 + ly);
                            assert_eq!(a.map(|sq| sq.vision_matrix),
                                b.map(|sq| sq.vision_matrix),
                                "vision matrix at ({x},{y},{z})");
                            assert_eq!(a.map(|sq| sq.collide_matrix),
                                b.map(|sq| sq.collide_matrix),
                                "collision matrix at ({x},{y},{z})");
                            assert_eq!(a.map(|sq| sq.objects.len()),
                                b.map(|sq| sq.objects.len()),
                                "object count at ({x},{y},{z})");
                            if let (Some(a), Some(b)) = (a, b) {
                                for (object_index, (a_obj, b_obj)) in a.objects.iter()
                                    .zip(&b.objects).enumerate() {
                                    assert_eq!(a_obj, b_obj,
                                        "native object {object_index} at ({x},{y},{z})");
                                }
                            }
                            panic!("native grid square metadata differs at ({x},{y},{z}); inspect properties, path, roof and dirty flags");
                        }
                    }
                }
                panic!("native chunk {key:?}: min/max level or non-square metadata differs");
            }
        }
        Ok(())
    }
}
