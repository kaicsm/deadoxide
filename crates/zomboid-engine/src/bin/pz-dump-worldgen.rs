//! Original world-generation probes: WgTilesProbe, WgChunkProbe and WgGenerateProbe.
//! The generated-chunk case compares a real 2x2 tile/object stream against the original
//! game server. It does not yet cover mapped biome raster replacement or attachments.

use std::fmt::Write as _;
use std::path::Path;

use zomboid_engine::java;
use zomboid_engine::zombie::iso::sprite::iso_sprite_manager::IsoSpriteManager;
use zomboid_engine::zombie::iso::worldgen::biomes::biome::{Biome, BiomeFeatures, Placements, SubBiomes};
use zomboid_engine::zombie::iso::worldgen::biomes::feature::Feature;
use zomboid_engine::zombie::iso::worldgen::biomes::feature_type::FeatureType;
use zomboid_engine::zombie::iso::worldgen::biomes::probability::Probability;
use zomboid_engine::zombie::iso::worldgen::biomes::tile_group::TileGroup;
use zomboid_engine::zombie::iso::worldgen::chunks_cache::ChunksCache;
use zomboid_engine::zombie::iso::worldgen::world_gen_tile::{PendingTiles, WorldGenTile};

fn group(sx: i32, sy: i32, names: &[&str]) -> TileGroup {
    TileGroup { sx, sy, tiles: names.iter().map(|v| (*v).to_string()).collect() }
}

fn feature(p: f64, min: i32, groups: Vec<TileGroup>) -> Feature {
    Feature { tile_groups: groups, min_size: min, max_size: 8, probability: Probability::proba_double(p) }
}

fn biome(features: Option<BiomeFeatures>, placements: Option<Placements>) -> Biome {
    Biome::new(
        "ORACLE".into(), None, true, features, None,
        None, None, None, None, None, None, 0.0,
        placements, SubBiomes::default(), Vec::new(), None,
    )
}

fn lookup_biome(name: &str) -> Biome {
    let mut features = if name == "no-features" { None } else { Some(BiomeFeatures::default()) };
    if let Some(ref mut features) = features {
        if name != "no-specific" {
            let mut list = Vec::new();
            if name != "empty-list" {
                let min = if name == "filtered" { 9 } else { 1 };
                if name == "no-groups" {
                    list.push(feature(1.0, min, Vec::new()));
                } else {
                    let tile = match name {
                        "too-big" | "cant-place" | "already-placed" | "two-by-two" => group(2, 2, &["a", "b", "c", "d"]),
                        "reserved" => group(1, 1, &["$no_tree"]),
                        "any" => group(1, 1, &["$any"]),
                        _ => group(1, 1, &["single_tile"]),
                    };
                    list.push(feature(if name == "zero-proba" { 0.0 } else { 1.0 }, min, vec![tile]));
                }
            }
            features.put(FeatureType::Ground, list);
        }
    }
    let placements = if name == "cant-place" {
        let mut p = Placements::default();
        p.by_type[FeatureType::Ground.index()] = vec!["*".into()];
        Some(p)
    } else {
        None
    };
    biome(features, placements)
}

fn group_text(group: Option<&TileGroup>) -> String {
    group.map_or_else(|| "nil".to_owned(), |g| format!("{}x{}:{}", g.sx, g.sy, g.tiles.join(",")))
}

fn pending_text(pending: &PendingTiles) -> String {
    let mut values = Vec::new();
    for y in 0..16 {
        for x in 0..16 {
            if let Some(value) = pending.get(FeatureType::Ground, x, y) {
                values.push(format!("{x}:{y}={value}"));
            }
        }
    }
    format!("[{}]", values.join(","))
}

/// WorldGenTile.findFeature/getGround/getPlant/private getBiomeTile over deterministic fabricated
/// biomes. Every RNG state is checked after the method, not only the returned result.
pub fn tiles() -> Result<String, String> {
    let wg = WorldGenTile;
    let mut out = String::new();
    let features = vec![
        feature(0.2, 1, vec![group(1, 1, &["a", "b"])]),
        feature(0.3, 1, vec![group(1, 1, &["c"])]),
        feature(0.5, 1, vec![group(1, 1, &["d"])]),
    ];
    for mode in 0..4 {
        for i in 0..24 {
            let mut rnd = java::Random::new(1000 + 100 * mode + i);
            let candidates: &[Feature] = if mode == 1 { &features[0..2] } else if mode == 2 { &[] } else { &features };
            let pre = if mode == 1 { 1.0 } else if mode == 3 { 0.0 } else { 1.0 };
            let post = if mode == 1 { 0.5 } else if mode == 3 { 0.0 } else { 1.0 };
            let found = wg.find_feature(candidates, pre, post, &mut rnd);
            let index = found.and_then(|f| candidates.iter().position(|v| std::ptr::eq(v, f))).map_or(-1, |v| v as i32);
            let _ = writeln!(out, "FIND {mode} {i} {index} NEXT {:x}", rnd.next_long() as u64);
        }
    }
    let mut features = BiomeFeatures::default();
    features.put(FeatureType::Ground, vec![
        feature(0.75, 1, vec![group(1, 1, &["g0"]), group(1, 1, &["g1"])]),
        feature(0.25, 1, vec![group(1, 1, &["g2"]), group(1, 1, &["g3"])]),
    ]);
    features.put(FeatureType::Plant, vec![
        feature(0.25, 1, vec![group(1, 1, &["p0"]), group(1, 1, &["p1"])]),
        feature(0.8, 1, vec![group(1, 1, &["p2"]), group(1, 1, &["p3"])]),
    ]);
    let selection_biome = biome(Some(features), None);
    for i in 0..40 {
        let mut ground_rnd = java::Random::new(2200 + i);
        let ground = group_text(wg.get_ground(&selection_biome, &mut ground_rnd));
        let _ = writeln!(out, "GROUND {i} {ground} NEXT {:x}", ground_rnd.next_long() as u64);
        let mut plant_rnd = java::Random::new(3200 + i);
        let plant = group_text(wg.get_plant(&selection_biome, &mut plant_rnd));
        let _ = writeln!(out, "PLANT {i} {plant} NEXT {:x}", plant_rnd.next_long() as u64);
    }
    let cases = [
        "no-features", "no-specific", "empty-list", "filtered", "no-groups",
        "too-big", "cant-place", "already-placed", "single", "reserved",
        "any", "existing", "zero-proba", "two-by-two",
    ];
    let chunks = ChunksCache::new();
    let sprites = IsoSpriteManager::new();
    for (i, name) in cases.iter().enumerate() {
        let x = if *name == "too-big" { 15 } else { 1 };
        let y = 1;
        let mut pending = PendingTiles::new();
        if *name == "already-placed" {
            pending.set(FeatureType::Ground, x + 1, y + 1, Some("occupied".into()));
        }
        if *name == "existing" {
            pending.set(FeatureType::Ground, x, y, Some("existing_tile".into()));
        }
        let mut rnd = java::Random::new(9000 + i as i64);
        let result = wg.get_biome_tile(
            &lookup_biome(name), FeatureType::Ground, &chunks, 0, 0,
            x, y, 0, x, y, &mut pending, &mut rnd, 8, &sprites,
        );
        let tile = result.tile.as_deref().unwrap_or("nil");
        let _ = writeln!(out, "LOOKUP {name} {} {tile} {} NEXT {:x}",
            result.tile_lookup.name(), pending_text(&pending), rnd.next_long() as u64);
    }
    Ok(out)
}

/// WorldGenChunk.getBiome/getMapBiome/priority/isProtected on the original WorldGen table and
/// the same server map bounds as wgroads. No generated chunks are used by this probe.
pub fn chunk(root: &Path) -> Result<String, String> {
    use std::rc::Rc;
    use zomboid_engine::se::krka::kahlua::j2se::kahlua_table_impl::KahluaTableImpl;
    use zomboid_engine::se::krka::kahlua::vm::object::Object;
    use zomboid_engine::zombie::core::random::rand_abstract::RandAbstract;
    use zomboid_engine::zombie::game_window::ClientStartup;
    use zomboid_engine::zombie::iso::iso_world::IsoWorld;
    use zomboid_engine::zombie::iso::iso_meta_grid::AlarmSettings;
    use zomboid_engine::zombie::iso::worldgen::world_gen_chunk::WorldGenChunk;
    use zomboid_engine::zombie::iso::worldgen::world_gen_params::WorldGenParams;
    use zomboid_engine::zombie::iso::worldgen::world_gen_reader::WorldGenReader;
    use zomboid_engine::zombie::lua::exposed_game;
    use zomboid_engine::zombie::lua::exposed_methods::sandbox_options_instance;
    use zomboid_engine::zombie::lua::lua_manager::LuaManager;

    let mut rand = RandAbstract::new(java::Random::new(12345));
    let alarm = AlarmSettings { last_stand: false, power_grid_exists: true, alarm: 1, alarm_decay: 1 };
    let world = IsoWorld::init(root, "Muldraugh, KY", &|_| false, &alarm, None, &mut rand)
        .map_err(|e| format!("IsoWorld.init: {e}"))?;
    let meta_bounds = (world.meta_grid.min_x, world.meta_grid.max_x, world.meta_grid.min_y, world.meta_grid.max_y);
    drop(world);
    let startup = ClientStartup::init(root).map_err(|e| format!("startup: {e}"))?;
    let mut lm = LuaManager::init_after(root, Some(Rc::new(startup)))
        .map_err(|e| format!("LuaManager.init: {e:?}"))?;
    exposed_game::init_shared(&lm.statics).map_err(|e| format!("initShared: {e}"))?;
    for dir in ["shared", "server"] {
        lm.load_dir_base(dir).map_err(|e| format!("LoadDirBase {dir}: {e:?}"))?;
    }
    let worldgen = match KahluaTableImpl::rawget(&lm.env, &Object::str("worldgen"))
        .map_err(|e| format!("worldgen table: {e:?}"))? {
        Object::Table(table) => table,
        _ => return Err("worldgen table missing".into()),
    };
    let sandbox = sandbox_options_instance(&lm.statics, &mut lm.thread)
        .map_err(|e| format!("SandboxOptions.instance: {e:?}"))?;
    let sb = sandbox.0.borrow();
    let reader = WorldGenReader { sandbox: &sb };
    let mut params = WorldGenParams::default();
    params.set_seed_string("DeadOxideOracle");
    let mut chunk = WorldGenChunk::from_worldgen_table(params.get_seed(), params, &worldgen, &sb, meta_bounds)?;
    let mut out = String::new();
    let _ = writeln!(out, "SEED {}", chunk.seed);
    let coords = [-63, -1, 0, 1, 7, 8, 30, 1574];
    for x in coords {
        for y in coords {
            let b = chunk.get_biome(x, y)?;
            let _ = writeln!(out, "BIOME {x} {y} {}", b.name);
        }
    }
    let map = reader.load_biomes(&worldgen, "biomes_map")
        .map_err(|e| format!("loadBiomes: {e:?}"))?;
    let mut filters: Vec<String> = map.iter().map(|(key, _)| key.to_owned()).collect();
    filters.sort();
    for filter in filters {
        for x in [-1, 0, 8, 1574] {
            for y in [-1, 0, 8, 1574] {
                let b = chunk.get_map_biome(x, y, Some(&filter))?;
                let _ = writeln!(out, "MAP {filter} {x} {y} {}", b.map_or("nil".to_string(), |b| b.name));
            }
        }
    }
    let mut keys: Vec<String> = chunk.priorities.iter().map(|(key, _)| key.to_owned()).collect();
    keys.sort();
    keys.truncate(8);
    keys.push("__missing__".into());
    for a in &keys {
        for b in &keys {
            let _ = writeln!(out, "PRIORITY {a} {b} {}", i32::from(chunk.priority(a, b)));
        }
    }
    let patterns: [&[&str]; 6] = [
        &[], &["vegetation_*"], &["fencing_01_?"], &["natural.02_3"], &["*"], &["?"],
    ];
    for (i, group) in patterns.iter().enumerate() {
        let pattern = group.iter().map(|v| (*v).to_string()).collect::<Vec<_>>();
        for tile in ["vegetation_01_0", "fencing_01_1", "natural.02_3", "other", "X"] {
            let _ = writeln!(out, "PROTECT {i} {tile} {}", i32::from(chunk.is_protected(&pattern, tile)));
        }
    }
    Ok(out)
}

/// WorldGenChunk.generateChunks: run the real 2x2 random-chunk path with the same Lua tables,
/// seed, tile definitions, original server map bounds and fixed cell origin as WgGenerateProbe.
/// Output includes every level-zero square, ordered objects and generation metadata.
pub fn generate(root: &Path) -> Result<String, String> {
    generate_prefab_fixture(root, false)
}

/// A static-module prefab inside the real 2x2 chunk-generation path.
pub fn prefabs(root: &Path) -> Result<String, String> {
    generate_prefab_fixture(root, true)
}

fn generate_prefab_fixture(root: &Path, use_prefab: bool) -> Result<String, String> {
    use std::rc::Rc;

    use zomboid_engine::se::krka::kahlua::j2se::kahlua_table_impl::KahluaTableImpl;
    use zomboid_engine::se::krka::kahlua::vm::object::Object;
    use zomboid_engine::zombie::core::random::rand_abstract::RandAbstract;
    use zomboid_engine::zombie::game_window::ClientStartup;
    use zomboid_engine::zombie::iso::cell_loader::LoadContext;
    use zomboid_engine::zombie::iso::iso_chunk::IsoChunk;
    use zomboid_engine::zombie::iso::iso_grid_square::IsoGridSquare;
    use zomboid_engine::zombie::iso::iso_meta_grid::AlarmSettings;
    use zomboid_engine::zombie::iso::iso_object::{IsoObject, ObjectKind};
    use zomboid_engine::zombie::iso::iso_world::IsoWorld;
    use zomboid_engine::zombie::iso::worldgen::chunks_cache::ChunksCache;
    use zomboid_engine::zombie::iso::worldgen::utils::chunk_coord::ChunkCoord;
    use zomboid_engine::zombie::iso::worldgen::world_gen_chunk::{WorldGenCell, WorldGenChunk};
    use zomboid_engine::zombie::iso::worldgen::world_gen_params::WorldGenParams;
    use zomboid_engine::zombie::lua::exposed_game;
    use zomboid_engine::zombie::lua::exposed_methods::sandbox_options_instance;
    use zomboid_engine::zombie::lua::lua_manager::LuaManager;

    const BASE_X: i32 = 1034;
    const BASE_Y: i32 = 1458;
    const FIXTURE_X: i32 = 2;
    const FIXTURE_Y: i32 = 3;
    const FIXTURE_SPRITE: &str = "blends_natural_01_0";

    let mut rand = RandAbstract::new(java::Random::new(12345));
    let alarm = AlarmSettings { last_stand: false, power_grid_exists: true, alarm: 1, alarm_decay: 1 };
    let mut world = IsoWorld::init(root, "Muldraugh, KY", &|_| false, &alarm, None, &mut rand)
        .map_err(|e| format!("IsoWorld.init: {e}"))?;
    let bounds = (world.meta_grid.min_x, world.meta_grid.max_x,
        world.meta_grid.min_y, world.meta_grid.max_y);
    let startup = ClientStartup::init(root).map_err(|e| format!("startup: {e}"))?;
    let mut lm = LuaManager::init_after(root, Some(Rc::new(startup)))
        .map_err(|e| format!("LuaManager.init: {e:?}"))?;
    exposed_game::init_shared(&lm.statics).map_err(|e| format!("initShared: {e}"))?;
    for dir in ["shared", "server"] {
        lm.load_dir_base(dir).map_err(|e| format!("LoadDirBase {dir}: {e:?}"))?;
    }
    let table = match KahluaTableImpl::rawget(&lm.env, &Object::str("worldgen"))
        .map_err(|e| format!("worldgen: {e:?}"))? {
        Object::Table(t) => t,
        _ => return Err("worldgen table missing".into()),
    };
    let sandbox = sandbox_options_instance(&lm.statics, &mut lm.thread)
        .map_err(|e| format!("SandboxOptions.instance: {e:?}"))?;
    let sb = sandbox.0.borrow();
    let mut params = WorldGenParams::default();
    params.set_seed_string("DeadOxideOracle");
    let seed = params.get_seed();
    let mut generator = WorldGenChunk::from_worldgen_table(seed, params, &table, &sb, bounds)?;
    if use_prefab {
        use zomboid_engine::java::JavaHashMap;
        use zomboid_engine::zombie::iso::worldgen::prefab_structure::PrefabStructure;
        use zomboid_engine::zombie::iso::worldgen::static_module::StaticModule;
        let mut schematic = JavaHashMap::default();
        schematic.put("Floor".to_owned(), vec![vec![1, 3], vec![3, 1]]);
        schematic.put("FloorFurniture".to_owned(), vec![vec![0, 0], vec![0, 2]]);
        schematic.put("FloorOverlay".to_owned(), vec![vec![2, 0], vec![0, 2]]);
        schematic.put("Furniture".to_owned(), vec![vec![0, 2], vec![0, 0]]);
        let prefab = PrefabStructure::new(
            [2, 2],
            vec!["blends_natural_01_0".to_owned(), "vegetation_groundcover_01_0".to_owned(), "blends_natural_01_64".to_owned()],
            schematic, 0.0,
        );
        generator.static_modules.insert(0, StaticModule::new(
            None, Some(prefab), BASE_X * 8, BASE_X * 8 + 15, BASE_Y * 8, BASE_Y * 8 + 15,
        ));
    }

    let mut cache = ChunksCache::new();
    for x in BASE_X..BASE_X + 2 {
        for y in BASE_Y..BASE_Y + 2 {
            cache.put(ChunkCoord::new(x, y), IsoChunk::new(x, y));
        }
    }
    let fixture_ref = world.sprites.get_sprite(FIXTURE_SPRITE);
    let mut square = IsoGridSquare::new(BASE_X * 8 + FIXTURE_X, BASE_Y * 8 + FIXTURE_Y, 0);
    square.objects.push(IsoObject::new(ObjectKind::IsoObject, Some(fixture_ref)));
    cache.get_mut(ChunkCoord::new(BASE_X, BASE_Y))
        .ok_or("missing fixture chunk")?
        .set_square(FIXTURE_X, FIXTURE_Y, 0, Some(square));

    let IsoWorld { sprites, alias, tile_definitions, meta_grid, .. } = &mut world;
    let mut context = LoadContext {
        sprites,
        alias,
        glass_removed_window_sprite_map: &tile_definitions.glass_removed_window_sprite_map,
        smashed_window_sprite_map: &tile_definitions.smashed_window_sprite_map,
        meta_grid,
        rand: &mut rand,
        locked_houses: 6,
        missing_tiles: Vec::new(),
        lotpack_cache: None,
    };
    // Every chunk has missing level-zero squares, so this exercises genRandomChunk, not the
    // map/raster branch; the empty zone slice is not consulted on that path.
    generator.generate_chunks(
        &mut cache, &mut context, WorldGenCell { world_x: 0, world_y: 0 },
        None, &mut [],
    )?;

    let mut out = String::new();
    let _ = writeln!(out, "{} {BASE_X} {BASE_Y} SEED {seed}", if use_prefab { "PREFABS" } else { "GENERATE" });
    let bit = |v: bool| i32::from(v);
    for wx in BASE_X..BASE_X + 2 {
        for wy in BASE_Y..BASE_Y + 2 {
            let key = ChunkCoord::new(wx, wy);
            let chunk = cache.get(key).ok_or("missing generated chunk")?;
            let state = generator.chunk_states.get(&(wx, wy))
                .ok_or("missing WorldGenChunk state")?;
            let depth = state.modification_depth;
            let partial = if state.attachments_partial.is_empty() {
                "null".to_owned()
            } else {
                state.attachments_partial.len().to_string()
            };
            let _ = writeln!(out,
                "CHUNK {wx} {wy} LEVELS {} {} WORLDGEN {} BLEND {} {} DEPTH {} {} {} {} ATTACH {} {} {} {} {} {} PARTIAL {} ZOMBIES {}",
                chunk.get_min_level(), chunk.get_max_level(), bit(state.generated_worldgen),
                bit(state.blending_done_full), bit(state.blending_done_partial),
                depth[0], depth[1], depth[2], depth[3],
                bit(state.attachments_done_full), bit(state.attachments_state[0]),
                bit(state.attachments_state[1]), bit(state.attachments_state[2]),
                bit(state.attachments_state[3]), bit(state.attachments_state[4]),
                partial, state.procedural_zombie_squares.len());
            let mut nonempty = 0;
            for x in 0..8 {
                for y in 0..8 {
                    let _ = write!(out, "S {wx} {wy} {x} {y}");
                    if let Some(square) = chunk.get_grid_square(x, y, 0) {
                        let _ = write!(out, " ROOM {} OBJECTS {}", square.room_id, square.objects.len());
                        for object in &square.objects {
                            let name = object.sprite.and_then(|s| context.sprites.sprite(s).name.as_deref())
                                .unwrap_or("null");
                            let _ = write!(out, " | {} {name}", object.kind.class_name());
                            nonempty += 1;
                        }
                    } else {
                        out.push_str(" null");
                    }
                    out.push('\n');
                }
            }
            for (x, y, z) in &state.procedural_zombie_squares {
                let _ = writeln!(out, "Z {wx} {wy} {x} {y} {z}");
            }
            if nonempty == 0 {
                return Err(format!("WorldGenChunk left an entire chunk empty: {wx},{wy}"));
            }
        }
    }
    Ok(out)
}

fn sample_summary(values: Option<Vec<i32>>) -> String {
    let Some(values) = values else { return "nil".to_owned(); };
    if values.is_empty() {
        return "0 0 0 0 0 0".to_owned();
    }
    let mut sum = 0i64;
    let mut hash = 0i32;
    for &value in &values {
        sum += i64::from(value);
        hash = hash.wrapping_mul(31).wrapping_add(value);
    }
    format!(
        "{} {} {} {} {} {}",
        values.len(),
        values[0],
        values[values.len() / 2],
        values[values.len() - 1],
        sum,
        hash,
    )
}

struct BiomeMapFixture {
    root: std::path::PathBuf,
}

impl BiomeMapFixture {
    fn new() -> Result<Self, String> {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let root = std::env::temp_dir().join(format!(
            "deadoxide-wgbiomemap-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&root).map_err(|e| format!("{}: {e}", root.display()))?;
        Ok(Self { root })
    }

    fn path(&self, map: &str, cell_x: i32, cell_y: i32) -> std::path::PathBuf {
        self.root.join(map).join(format!("biomemap_{cell_x}_{cell_y}.png"))
    }

    fn register(
        &self,
        fs: &mut zomboid_engine::zombie::zomboid_file_system::ZomboidFileSystem,
        map: &str,
        cell_x: i32,
        cell_y: i32,
        path: &std::path::Path,
    ) {
        let relative = format!("media/maps/{map}/maps/biomemap_{cell_x}_{cell_y}.png").to_lowercase();
        fs.active_file_map.insert(relative, path.to_string_lossy().replace('\\', "/"));
    }

    fn write_raster(
        &self,
        path: &std::path::Path,
        base_red: u8,
        base_green: u8,
        block_red: u8,
        block_green: u8,
    ) -> Result<(), String> {
        use std::io::BufWriter;

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        let file = std::fs::File::create(path)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        let mut encoder = png::Encoder::new(BufWriter::new(file), 256, 256);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder
            .write_header()
            .map_err(|e| format!("{}: {e}", path.display()))?;
        let mut pixels = vec![0u8; 256 * 256 * 4];
        for pixel in pixels.chunks_exact_mut(4) {
            pixel[0] = base_red;
            pixel[1] = base_green;
            pixel[3] = 255;
        }
        for y in 0usize..8 {
            for x in 8usize..16 {
                let index = (y * 256 + x) * 4;
                pixels[index] = block_red;
                pixels[index + 1] = block_green;
                pixels[index + 3] = 255;
            }
        }
        writer
            .write_image_data(&pixels)
            .map_err(|e| format!("{}: {e}", path.display()))
    }

    fn write_corrupt(&self, path: &std::path::Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        std::fs::write(path, b"not-a-png")
            .map_err(|e| format!("{}: {e}", path.display()))
    }
}

impl Drop for BiomeMapFixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// BiomeMap runtime parity: automatic PNG lookup through the active file map, world-map search
/// order, the two image bands, missing-entry cache retention/eviction, and the refusal to fall
/// through to a later world map when the first existing PNG is corrupt.
pub fn biomemap(root: &Path) -> Result<String, String> {
    use std::rc::Rc;

    use zomboid_engine::zombie::game_window::ClientStartup;
    use zomboid_engine::zombie::iso::worldgen::maps::biome_map::{BiomeMap, BiomeMapType};
    use zomboid_engine::zombie::lua::exposed_game;
    use zomboid_engine::zombie::lua::exposed_methods::sandbox_options_instance;
    use zomboid_engine::zombie::lua::lua_manager::LuaManager;
    use zomboid_engine::zombie::zomboid_file_system::ZomboidFileSystem;

    let startup = ClientStartup::init(root).map_err(|e| format!("startup: {e}"))?;
    let mut lm = LuaManager::init_after(root, Some(Rc::new(startup)))
        .map_err(|e| format!("LuaManager.init: {e:?}"))?;
    exposed_game::init_shared(&lm.statics).map_err(|e| format!("initShared: {e}"))?;
    for dir in ["shared", "server"] {
        lm.load_dir_base(dir)
            .map_err(|e| format!("LoadDirBase {dir}: {e:?}"))?;
    }
    let sandbox = sandbox_options_instance(&lm.statics, &mut lm.thread)
        .map_err(|e| format!("SandboxOptions.instance: {e:?}"))?;
    let sb = sandbox.0.borrow();
    let mut fs = ZomboidFileSystem::init(root).map_err(|e| format!("ZomboidFileSystem.init: {e}"))?;
    let fixture = BiomeMapFixture::new()?;
    let mut out = String::new();

    let first = fixture.path("oracle_a", 1, 2);
    fixture.write_raster(&first, 17, 201, 18, 202)?;
    fixture.register(&mut fs, "oracle_a", 1, 2, &first);
    let second = fixture.path("oracle_b", 1, 2);
    fixture.write_raster(&second, 91, 45, 92, 46)?;
    fixture.register(&mut fs, "oracle_b", 1, 2, &second);
    let first_map = BiomeMap::from_lua_environment("oracle_missing;oracle_a;oracle_b", &fs, &lm.env, &sb)?;
    let _ = writeln!(out, "CASE FIRST");
    let _ = writeln!(out, "CELL BIOME {}", sample_summary(first_map.get_cell(1, 2, BiomeMapType::Biome)?));
    let _ = writeln!(out, "CELL ZONE {}", sample_summary(first_map.get_cell(1, 2, BiomeMapType::Zone)?));
    let _ = writeln!(out, "ZONES BIOME {}", sample_summary(first_map.get_zones(33, 64, BiomeMapType::Biome)?));
    let _ = writeln!(out, "ZONES ZONE {}", sample_summary(first_map.get_zones(33, 64, BiomeMapType::Zone)?));

    let active = fixture.path("oracle_override", 0, 0);
    fixture.write_raster(&active, 55, 66, 77, 88)?;
    fixture.register(&mut fs, "oracle_override", 0, 0, &active);
    let active_map = BiomeMap::from_lua_environment("oracle_override", &fs, &lm.env, &sb)?;
    let _ = writeln!(out, "CASE ACTIVE");
    let _ = writeln!(out, "CELL BIOME {}", sample_summary(active_map.get_cell(0, 0, BiomeMapType::Biome)?));
    let _ = writeln!(out, "CELL ZONE {}", sample_summary(active_map.get_cell(0, 0, BiomeMapType::Zone)?));

    let cache_file = fixture.path("oracle_cache", 9, 9);
    let _ = std::fs::remove_file(&cache_file);
    fixture.register(&mut fs, "oracle_cache", 9, 9, &cache_file);
    let cache_map = BiomeMap::from_lua_environment("oracle_cache", &fs, &lm.env, &sb)?;
    let _ = writeln!(out, "CASE CACHE");
    let _ = writeln!(out, "MISS0 {}", sample_summary(cache_map.get_cell(9, 9, BiomeMapType::Biome)?));
    fixture.write_raster(&cache_file, 70, 80, 90, 100)?;
    let _ = writeln!(out, "MISS1 {}", sample_summary(cache_map.get_cell(9, 9, BiomeMapType::Biome)?));
    for i in 0..17 {
        let _ = cache_map.get_cell(100 + i, i, BiomeMapType::Biome)?;
    }
    let _ = writeln!(out, "MISS2 {}", sample_summary(cache_map.get_cell(9, 9, BiomeMapType::Biome)?));

    let corrupt = fixture.path("oracle_corrupt", 0, 0);
    fixture.write_corrupt(&corrupt)?;
    fixture.register(&mut fs, "oracle_corrupt", 0, 0, &corrupt);
    let fallback = fixture.path("oracle_fallback", 0, 0);
    fixture.write_raster(&fallback, 13, 14, 15, 16)?;
    fixture.register(&mut fs, "oracle_fallback", 0, 0, &fallback);
    let corrupt_map = BiomeMap::from_lua_environment("oracle_corrupt;oracle_fallback", &fs, &lm.env, &sb)?;
    let _ = writeln!(out, "CASE CORRUPT");
    for (name, kind) in [("BIOME", BiomeMapType::Biome), ("ZONE", BiomeMapType::Zone)] {
        let result = match corrupt_map.get_cell(0, 0, kind) {
            Ok(samples) => sample_summary(samples),
            Err(error) => format!("THROW {error}"),
        };
        let _ = writeln!(out, "CELL {name} {result}");
    }

    let _ = writeln!(out, "CASE NEGATIVE");
    let _ = writeln!(out, "WEST {}", sample_summary(first_map.get_zones(-1, 64, BiomeMapType::Biome)?));
    let _ = writeln!(out, "NORTH {}", sample_summary(first_map.get_zones(33, -1, BiomeMapType::Zone)?));
    Ok(out)
}

struct MapChunkFixture {
    root: std::path::PathBuf,
}

impl MapChunkFixture {
    fn new() -> Result<Self, String> {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let root = std::env::temp_dir().join(format!(
            "deadoxide-wgmapchunk-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed),
        ));
        std::fs::create_dir_all(&root).map_err(|e| format!("{}: {e}", root.display()))?;
        Ok(Self { root })
    }

    fn path(&self, cell_x: i32, cell_y: i32) -> std::path::PathBuf {
        self.root.join(format!("biomemap_{cell_x}_{cell_y}.png"))
    }

    fn red(x: usize, y: usize, pixels: &[i32; 3]) -> i32 {
        let lx = x & 15;
        let ly = y & 15;
        if lx < 8 && ly < 8 {
            pixels[0]
        } else if lx >= 8 && ly < 8 {
            pixels[1]
        } else if lx < 8 {
            pixels[2]
        } else if (lx + ly) & 1 == 0 {
            pixels[0]
        } else {
            pixels[1]
        }
    }

    fn write_raster(&self, path: &Path, pixels: &[i32; 3]) -> Result<(), String> {
        use std::io::BufWriter;
        let file = std::fs::File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut encoder = png::Encoder::new(BufWriter::new(file), 256, 256);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().map_err(|e| format!("{}: {e}", path.display()))?;
        let mut data = vec![0u8; 256 * 256 * 4];
        for y in 0..256usize {
            for x in 0..256usize {
                let index = (y * 256 + x) * 4;
                data[index] = Self::red(x, y, pixels) as u8;
                data[index + 1] = (13 + ((x & 15) + 2 * (y & 15)) % 200) as u8;
                data[index + 3] = 255;
            }
        }
        writer.write_image_data(&data).map_err(|e| format!("{}: {e}", path.display()))
    }
}

impl Drop for MapChunkFixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// WorldGenChunk.genMapChunk: generate four fully populated chunks using the original Lua biome
/// mapping and a generated PNG in the active-file index. The output matches WgMapChunkProbe's
/// ordered level-zero objects, chunk metadata, and biome selection.
pub fn mapchunk(root: &Path) -> Result<String, String> {
    use std::rc::Rc;

    use zomboid_engine::se::krka::kahlua::j2se::kahlua_table_impl::KahluaTableImpl;
    use zomboid_engine::se::krka::kahlua::vm::object::Object;
    use zomboid_engine::zombie::core::random::rand_abstract::RandAbstract;
    use zomboid_engine::zombie::game_window::ClientStartup;
    use zomboid_engine::zombie::iso::cell_loader::LoadContext;
    use zomboid_engine::zombie::iso::iso_chunk::IsoChunk;
    use zomboid_engine::zombie::iso::iso_grid_square::IsoGridSquare;
    use zomboid_engine::zombie::iso::iso_meta_grid::AlarmSettings;
    use zomboid_engine::zombie::iso::iso_object::{IsoObject, ObjectKind};
    use zomboid_engine::zombie::iso::iso_world::IsoWorld;
    use zomboid_engine::zombie::iso::worldgen::chunks_cache::ChunksCache;
    use zomboid_engine::zombie::iso::worldgen::maps::biome_map::{BiomeMap, BiomeMapType};
    use zomboid_engine::zombie::iso::worldgen::utils::chunk_coord::ChunkCoord;
    use zomboid_engine::zombie::iso::worldgen::world_gen_chunk::{WorldGenCell, WorldGenChunk, WorldGenChunkState};
    use zomboid_engine::zombie::iso::worldgen::world_gen_params::WorldGenParams;
    use zomboid_engine::zombie::lua::exposed_game;
    use zomboid_engine::zombie::lua::exposed_methods::sandbox_options_instance;
    use zomboid_engine::zombie::lua::lua_manager::LuaManager;
    use zomboid_engine::zombie::zomboid_file_system::ZomboidFileSystem;

    const BASE_X: i32 = 3072;
    const BASE_Y: i32 = 4096;
    const MAP_NAME: &str = "oracle_mapchunk";
    const FLOOR_SPRITE: &str = "blends_natural_01_0";
    const PLANT_SPRITE: &str = "vegetation_groundcover_01_0";

    let mut rand = RandAbstract::new(java::Random::new(12345));
    let alarm = AlarmSettings { last_stand: false, power_grid_exists: true, alarm: 1, alarm_decay: 1 };
    let mut world = IsoWorld::init(root, "Muldraugh, KY", &|_| false, &alarm, None, &mut rand)
        .map_err(|e| format!("IsoWorld.init: {e}"))?;
    let bounds = (world.meta_grid.min_x, world.meta_grid.max_x,
        world.meta_grid.min_y, world.meta_grid.max_y);
    let startup = ClientStartup::init(root).map_err(|e| format!("startup: {e}"))?;
    let mut lm = LuaManager::init_after(root, Some(Rc::new(startup)))
        .map_err(|e| format!("LuaManager.init: {e:?}"))?;
    exposed_game::init_shared(&lm.statics).map_err(|e| format!("initShared: {e}"))?;
    for dir in ["shared", "server"] {
        lm.load_dir_base(dir).map_err(|e| format!("LoadDirBase {dir}: {e:?}"))?;
    }
    let worldgen = match KahluaTableImpl::rawget(&lm.env, &Object::str("worldgen"))
        .map_err(|e| format!("worldgen table: {e:?}"))? {
        Object::Table(t) => t,
        _ => return Err("worldgen table missing".to_owned()),
    };
    let sandbox = sandbox_options_instance(&lm.statics, &mut lm.thread)
        .map_err(|e| format!("SandboxOptions.instance: {e:?}"))?;
    let sb = sandbox.0.borrow();
    let mut params = WorldGenParams::default();
    params.set_seed_string("DeadOxideOracle");
    let seed = params.get_seed();
    let mut generator = WorldGenChunk::from_worldgen_table(seed, params, &worldgen, &sb, bounds)?;
    let mut fs = ZomboidFileSystem::init(root)
        .map_err(|e| format!("ZomboidFileSystem.init: {e}"))?;
    let lookup = BiomeMap::from_lua_environment(MAP_NAME, &fs, &lm.env, &sb)?;
    let mut candidates = lookup.zone_map.iter()
        .filter_map(|(pixel, entry)| {
            let name = entry.biome.as_deref()?;
            (pixel >= 0 && pixel <= 255 && name != "$random")
                .then(|| (pixel, name.to_owned()))
        }).collect::<Vec<_>>();
    candidates.sort_by_key(|(pixel, _)| *pixel);
    if candidates.is_empty() {
        return Err("biome_map_config contains no mapped biome pixels".to_owned());
    }
    let mut chosen: Vec<(i32, String)> = Vec::new();
    for (pixel, name) in candidates {
        if generator.get_map_biome(BASE_X * 8, BASE_Y * 8, Some(&name))?.is_some() {
            chosen.push((pixel, name));
            if chosen.len() == 2 { break; }
        }
    }
    if chosen.is_empty() {
        return Err("no configured pixel resolves to a mapped biome".to_owned());
    }
    if chosen.len() == 1 { chosen.push(chosen[0].clone()); }
    let unknown = (0..=255).rev()
        .find(|pixel| lookup.get_entry(*pixel).is_none())
        .ok_or("no free PNG palette index for missing biome")?;
    let pixels = [chosen[0].0, chosen[1].0, unknown];

    let fixture = MapChunkFixture::new()?;
    let filename = fixture.path(BASE_X / 32, BASE_Y / 32);
    fixture.write_raster(&filename, &pixels)?;
    fs.active_file_map.insert(
        format!("media/maps/{MAP_NAME}/maps/biomemap_{}_{}.png", BASE_X / 32, BASE_Y / 32),
        filename.to_string_lossy().replace('\\', "/"),
    );
    let map = BiomeMap::from_lua_environment(MAP_NAME, &fs, &lm.env, &sb)?;

    let mut cache = ChunksCache::new();
    let floor = world.sprites.get_sprite(FLOOR_SPRITE);
    let plant = world.sprites.get_sprite(PLANT_SPRITE);
    for wx in BASE_X..BASE_X + 2 {
        for wy in BASE_Y..BASE_Y + 2 {
            let mut chunk = IsoChunk::new(wx, wy);
            chunk.set_min_max_level(0, 0);
            for x in 0..8 {
                for y in 0..8 {
                    let mut square = IsoGridSquare::new(wx * 8 + x, wy * 8 + y, 0);
                    square.objects.push(IsoObject::new(ObjectKind::IsoObject, Some(floor)));
                    if x == 1 && y == 1 {
                        square.objects.push(IsoObject::new(ObjectKind::IsoObject, Some(plant)));
                    }
                    if square.get_floor(&world.sprites).is_none() {
                        return Err(format!("fixture floor lacks solidfloor: {FLOOR_SPRITE}"));
                    }
                    chunk.set_square(x, y, 0, Some(square));
                }
            }
            cache.put(ChunkCoord::new(wx, wy), chunk);
        }
    }
    let samples = map.get_zones(BASE_X, BASE_Y, BiomeMapType::Biome)?
        .ok_or("the Rust BiomeMap did not load the fixture PNG")?;
    if samples.len() != 64 || samples[0] != pixels[0] {
        return Err("the Rust BiomeMap returned incorrect fixture PNG samples".to_owned());
    }

    let IsoWorld { sprites, alias, tile_definitions, meta_grid, .. } = &mut world;
    let mut ctx = LoadContext {
        sprites,
        alias,
        glass_removed_window_sprite_map: &tile_definitions.glass_removed_window_sprite_map,
        smashed_window_sprite_map: &tile_definitions.smashed_window_sprite_map,
        meta_grid,
        rand: &mut rand,
        locked_houses: 6,
        missing_tiles: Vec::new(),
        lotpack_cache: None,
    };
    generator.generate_chunks(
        &mut cache,
        &mut ctx,
        WorldGenCell { world_x: 0, world_y: 0 },
        Some(&map),
        &mut [],
    )?;

    let bit = |v: bool| i32::from(v);
    let mut out = String::new();
    let _ = writeln!(out, "MAPGEN {BASE_X} {BASE_Y} SEED {seed}");
    let _ = writeln!(out, "PIXELS {} {} {} BIOMES {} {}",
        pixels[0], pixels[1], pixels[2], chosen[0].1, chosen[1].1);
    for wx in BASE_X..BASE_X + 2 {
        for wy in BASE_Y..BASE_Y + 2 {
            let chunk = cache.get(ChunkCoord::new(wx, wy)).ok_or("missing map chunk")?;
            let state = generator.chunk_states.get(&(wx, wy));
            let defaults = WorldGenChunkState::default();
            let state = state.unwrap_or(&defaults);
            let depth = state.modification_depth;
            let partial = if state.attachments_partial.is_empty() {
                "null".to_owned()
            } else {
                state.attachments_partial.len().to_string()
            };
            let _ = writeln!(out,
                "CHUNK {wx} {wy} LEVELS {} {} WORLDGEN {} BLEND {} {} DEPTH {} {} {} {} ATTACH {} {} {} {} {} {} PARTIAL {} ZOMBIES {}",
                chunk.get_min_level(), chunk.get_max_level(), bit(state.generated_worldgen),
                bit(state.blending_done_full), bit(state.blending_done_partial),
                depth[0], depth[1], depth[2], depth[3],
                bit(state.attachments_done_full), bit(state.attachments_state[0]),
                bit(state.attachments_state[1]), bit(state.attachments_state[2]),
                bit(state.attachments_state[3]), bit(state.attachments_state[4]),
                partial, state.procedural_zombie_squares.len(),
            );
            for x in 0..8 {
                for y in 0..8 {
                    let _ = write!(out, "S {wx} {wy} {x} {y}");
                    if let Some(square) = chunk.get_grid_square(x, y, 0) {
                        let _ = write!(out, " ROOM {} OBJECTS {}", square.room_id, square.objects.len());
                        for object in &square.objects {
                            let name = object.sprite.and_then(|sprite| ctx.sprites.sprite(sprite).name.as_deref())
                                .unwrap_or("null");
                            let _ = write!(out, " | {} {name}", object.kind.class_name());
                        }
                    } else {
                        out.push_str(" null");
                    }
                    out.push('\n');
                }
            }
            for &(x, y, z) in &state.procedural_zombie_squares {
                let _ = writeln!(out, "Z {wx} {wy} {x} {y} {z}");
            }
        }
    }
    Ok(out)
}

fn advanced_worldgen<F>(root: &Path, run: F) -> Result<String, String>
where
    F: FnOnce(
        &mut zomboid_engine::zombie::iso::worldgen::world_gen_chunk::WorldGenChunk,
        &mut zomboid_engine::zombie::iso::iso_world::IsoWorld,
        &mut zomboid_engine::zombie::core::random::rand_abstract::RandAbstract<java::Random>,
    ) -> Result<String, String>,
{
    use std::rc::Rc;
    use zomboid_engine::se::krka::kahlua::j2se::kahlua_table_impl::KahluaTableImpl;
    use zomboid_engine::se::krka::kahlua::vm::object::Object;
    use zomboid_engine::zombie::core::random::rand_abstract::RandAbstract;
    use zomboid_engine::zombie::game_window::ClientStartup;
    use zomboid_engine::zombie::iso::iso_meta_grid::AlarmSettings;
    use zomboid_engine::zombie::iso::iso_world::IsoWorld;
    use zomboid_engine::zombie::iso::worldgen::world_gen_chunk::WorldGenChunk;
    use zomboid_engine::zombie::iso::worldgen::world_gen_params::WorldGenParams;
    use zomboid_engine::zombie::lua::exposed_game;
    use zomboid_engine::zombie::lua::exposed_methods::sandbox_options_instance;
    use zomboid_engine::zombie::lua::lua_manager::LuaManager;

    let mut rand = RandAbstract::new(java::Random::new(12345));
    let alarm = AlarmSettings { last_stand: false, power_grid_exists: true, alarm: 1, alarm_decay: 1 };
    let mut world = IsoWorld::init(root, "Muldraugh, KY", &|_| false, &alarm, None, &mut rand)
        .map_err(|e| format!("IsoWorld.init: {e}"))?;
    let bounds = (world.meta_grid.min_x, world.meta_grid.max_x,
        world.meta_grid.min_y, world.meta_grid.max_y);
    let startup = ClientStartup::init(root).map_err(|e| format!("startup: {e}"))?;
    let mut lm = LuaManager::init_after(root, Some(Rc::new(startup)))
        .map_err(|e| format!("LuaManager.init: {e:?}"))?;
    exposed_game::init_shared(&lm.statics).map_err(|e| format!("initShared: {e}"))?;
    for dir in ["shared", "server"] {
        lm.load_dir_base(dir).map_err(|e| format!("LoadDirBase {dir}: {e:?}"))?;
    }
    let table = match KahluaTableImpl::rawget(&lm.env, &Object::str("worldgen"))
        .map_err(|e| format!("worldgen: {e:?}"))? {
        Object::Table(t) => t,
        _ => return Err("worldgen table missing".to_owned()),
    };
    let sandbox = sandbox_options_instance(&lm.statics, &mut lm.thread)
        .map_err(|e| format!("SandboxOptions.instance: {e:?}"))?;
    let sb = sandbox.0.borrow();
    let mut params = WorldGenParams::default();
    params.set_seed_string("DeadOxideOracle");
    let mut generator = WorldGenChunk::from_worldgen_table(params.get_seed(), params, &table, &sb, bounds)?;
    run(&mut generator, &mut world, &mut rand)
}

struct AdvancedChunks {
    cache: ChunksCache,
}

impl AdvancedChunks {
    const X: i32 = 1034;
    const Y: i32 = 1458;

    fn new(floor: zomboid_engine::zombie::iso::sprite::iso_sprite_manager::SpriteRef) -> Self {
        use zomboid_engine::zombie::iso::iso_chunk::IsoChunk;
        use zomboid_engine::zombie::iso::iso_grid_square::IsoGridSquare;
        use zomboid_engine::zombie::iso::iso_object::{IsoObject, ObjectKind};
        use zomboid_engine::zombie::iso::worldgen::utils::chunk_coord::ChunkCoord;

        let mut cache = ChunksCache::new();
        for dx in -1..=1 {
            for dy in -1..=1 {
                let wx = Self::X + dx;
                let wy = Self::Y + dy;
                let mut chunk = IsoChunk::new(wx, wy);
                chunk.set_min_max_level(0, 0);
                for x in 0..8 {
                    for y in 0..8 {
                        let mut sq = IsoGridSquare::new(wx * 8 + x, wy * 8 + y, 0);
                        sq.objects.push(IsoObject::new(ObjectKind::IsoObject, Some(floor)));
                        chunk.set_square(x, y, 0, Some(sq));
                    }
                }
                chunk.set_loaded();
                cache.put(ChunkCoord::new(wx, wy), chunk);
            }
        }
        Self { cache }
    }

    fn key(dx: i32, dy: i32) -> zomboid_engine::zombie::iso::worldgen::utils::chunk_coord::ChunkCoord {
        use zomboid_engine::zombie::iso::worldgen::utils::chunk_coord::ChunkCoord;
        ChunkCoord::new(Self::X + dx, Self::Y + dy)
    }
}

fn advanced_objects(sq: &zomboid_engine::zombie::iso::iso_grid_square::IsoGridSquare,
    sprites: &IsoSpriteManager) -> String {
    let mut result = String::new();
    for obj in &sq.objects {
        let name = obj.sprite.and_then(|s| sprites.sprite(s).name.as_deref()).unwrap_or("null");
        let _ = write!(result, " | {} {name}", obj.kind.class_name());
        if let Some(attached) = &obj.attached_anim_sprite {
            for spr in attached {
                let name = sprites.sprite(*spr).name.as_deref().unwrap_or("null");
                let _ = write!(result, " + {name}");
            }
        }
    }
    result
}

fn blending_dump(out: &mut String, label: &str, fixture: &AdvancedChunks,
    generator: &zomboid_engine::zombie::iso::worldgen::world_gen_chunk::WorldGenChunk,
    sprites: &IsoSpriteManager) -> Result<(), String> {
    use zomboid_engine::zombie::iso::worldgen::blending::BlendDirection;
    let key = AdvancedChunks::key(0, 0);
    let state = generator.chunk_states.get(&(key.x, key.y)).ok_or("missing blending state")?;
    let bit = |v: bool| i32::from(v);
    let _ = writeln!(out, "{label} FULL {} PARTIAL {} DIR {} {} {} {}",
        bit(state.blending_done_full), bit(state.blending_done_partial),
        bit(state.blending_modified[0]), bit(state.blending_modified[1]),
        bit(state.blending_modified[2]), bit(state.blending_modified[3]));
    for dir in BlendDirection::ALL {
        let (dx, dy) = dir.offset();
        let opposite = dir.opposite().index();
        let (name, state) = match dir {
            BlendDirection::North => ("NORTH", AdvancedChunks::key(0, -1)),
            BlendDirection::South => ("SOUTH", AdvancedChunks::key(0, 1)),
            BlendDirection::West => ("WEST", AdvancedChunks::key(-1, 0)),
            BlendDirection::East => ("EAST", AdvancedChunks::key(1, 0)),
        };
        debug_assert_eq!(state, AdvancedChunks::key(dx, dy));
        let done = generator.chunk_states.get(&(state.x, state.y))
            .is_some_and(|s| s.blending_modified[opposite]);
        let _ = writeln!(out, "NEIGHBOR {name} {}", bit(done));
    }
    let chunk = fixture.cache.get(key).ok_or("missing center chunk")?;
    for x in 0..8 {
        for y in 0..8 {
            let sq = chunk.get_grid_square(x, y, 0).ok_or("missing blending square")?;
            let _ = writeln!(out, "S {x} {y}{}", advanced_objects(sq, sprites));
        }
    }
    Ok(())
}

/// The original Java `Blending.applyBlending` over a seeded nine-chunk neighbourhood.
pub fn blending(root: &Path) -> Result<String, String> {
    use zomboid_engine::zombie::iso::iso_object::{IsoObject, ObjectKind, IsoTree};
    use zomboid_engine::zombie::iso::worldgen::blending::Blending;

    advanced_worldgen(root, |generator, world, _| {
        let floor = world.sprites.get_sprite("blends_natural_01_0");
        let tree = world.sprites.get_sprite("vegetation_trees_01_0");
        let mut fixture = AdvancedChunks::new(floor);
        let center = AdvancedChunks::key(0, 0);
        generator.chunk_states.entry((center.x, center.y)).or_default();
        generator.chunk_states.entry((AdvancedChunks::X - 1, AdvancedChunks::Y))
            .or_default().blending_done_full = true;
        for y in [0, 4] {
            fixture.cache.get_mut(center).unwrap().get_grid_square_mut(0, y, 0)
                .unwrap().objects.push(IsoObject::new(ObjectKind::IsoTree(IsoTree::default()), Some(tree)));
        }
        let mut out = String::new();
        blending_dump(&mut out, "BEFORE", &fixture, generator, &world.sprites)?;
        Blending.apply_blending(generator, &mut fixture.cache, center, &mut world.sprites, &world.alias)?;
        blending_dump(&mut out, "AFTER", &fixture, generator, &world.sprites)?;
        Blending.apply_blending(generator, &mut fixture.cache, center, &mut world.sprites, &world.alias)?;
        blending_dump(&mut out, "REPEAT", &fixture, generator, &world.sprites)?;
        Ok(out)
    })
}

fn attachments_dump(out: &mut String, label: &str, fixture: &AdvancedChunks,
    generator: &zomboid_engine::zombie::iso::worldgen::world_gen_chunk::WorldGenChunk,
    sprites: &IsoSpriteManager) -> Result<(), String> {
    let key = AdvancedChunks::key(0, 0);
    let state = generator.chunk_states.get(&(key.x, key.y)).ok_or("missing attachment state")?;
    let partial = if state.attachments_partial_initialized { state.attachments_partial.len().to_string() } else { "null".to_owned() };
    let bit = |v: bool| i32::from(v);
    let _ = writeln!(out, "{label} STATE {} {} {} {} {} {} PARTIAL {partial}",
        bit(state.attachments_done_full), bit(state.attachments_state[0]), bit(state.attachments_state[1]),
        bit(state.attachments_state[2]), bit(state.attachments_state[3]), bit(state.attachments_state[4]));
    let center = fixture.cache.get(key).ok_or("missing center")?;
    let east = fixture.cache.get(AdvancedChunks::key(1, 0)).ok_or("missing east")?;
    for x in 0..8 {
        for y in 0..8 {
            let sq = center.get_grid_square(x, y, 0).ok_or("missing attachment square")?;
            let _ = writeln!(out, "C {x} {y}{}", advanced_objects(sq, sprites));
        }
    }
    for y in 0..8 {
        let sq = east.get_grid_square(0, y, 0).ok_or("missing east square")?;
        let _ = writeln!(out, "E 0 {y}{}", advanced_objects(sq, sprites));
    }
    Ok(())
}

fn attachment_floor_candidates(sprites: &IsoSpriteManager,
    alias: &zomboid_engine::zombie::core::tile_property_alias_map::TilePropertyAliasMap,
) -> Result<[(String, String); 2], String> {
    use std::collections::BTreeMap;
    use zomboid_engine::zombie::iso::sprite_details::iso_flag_type::IsoFlagType;
    let mut floors: BTreeMap<String, String> = BTreeMap::new();
    let mut names: Vec<&String> = sprites.named_map.keys().collect();
    names.sort();
    for name in names {
        let sprite = sprites.sprite(*sprites.named_map.get(name).ok_or("named sprite missing")?);
        if !sprite.properties.has(IsoFlagType::solidfloor) { continue; }
        let Some(material) = sprite.properties.get(alias, "FloorMaterial") else { continue; };
        if material != "None" && material != "null" {
            floors.entry(material.to_owned()).or_insert_with(|| name.to_owned());
        }
    }
    let mut iter = floors.into_iter();
    Ok([iter.next().ok_or("missing first floor material")?,
        iter.next().ok_or("missing second floor material")?])
}

/// The original Java `AttachmentsHandler`: catalog, full pass, incomplete-neighbour gate,
/// and partial pass with removal/replacement of an existing floor overlay.
pub fn attachments(root: &Path) -> Result<String, String> {
    use zomboid_engine::zombie::iso::sprite_details::iso_flag_type::IsoFlagType;
    use zomboid_engine::zombie::iso::worldgen::attachments::AttachmentsHandler;
    use zomboid_engine::zombie::iso::worldgen::utils::direction::Direction;

    advanced_worldgen(root, |generator, world, _| {
        let floors = attachment_floor_candidates(&world.sprites, &world.alias)?;
        let ((material_a, name_a), (material_b, name_b)) = (&floors[0], &floors[1]);
        let mut out = String::new();
        let _ = writeln!(out, "MATERIALS {material_a} {name_a} {material_b} {name_b}");
        let floor_a = world.sprites.get_sprite(name_a);
        let floor_b = world.sprites.get_sprite(name_b);
        let mut fixture = AdvancedChunks::new(floor_a);
        let east_key = AdvancedChunks::key(1, 0);
        for x in 0..8 {
            for y in 0..8 {
                let sq = fixture.cache.get_mut(east_key).unwrap().get_grid_square_mut(x, y, 0).unwrap();
                let floor = sq.get_floor(&world.sprites).ok_or("east fixture has no floor")?;
                sq.objects[floor].sprite = Some(floor_b);
            }
        }
        let mut handler = AttachmentsHandler::new();
        handler.load_attachments(&world.sprites, &world.alias)?;
        let (materials, total) = handler.catalogue_counts();
        let _ = writeln!(out, "CATALOG {materials} {total}");
        let overlay = world.sprites.get_sprite("oracle_floor_attachment_w");
        {
            let props = &mut world.sprites.sprite_mut(overlay).properties;
            props.set(IsoFlagType::IsFloorAttached);
            props.set(IsoFlagType::FloorAttachmentW);
        }
        // The Java fixture's forced named sprite is independent of the installed media.
        world.sprites.sprite_mut(overlay).name = Some("oracle_floor_attachment_w".to_owned());
        handler.set_probe_choice(material_a, Direction::West, overlay);

        let center = AdvancedChunks::key(0, 0);
        handler.reset_attachments(generator, center);
        let missing = AdvancedChunks::key(-1, -1);
        let displaced = fixture.cache.remove(missing).ok_or("missing northwest fixture")?;
        handler.apply_attachments(generator, &mut fixture.cache, center, &world.sprites, &world.alias)?;
        attachments_dump(&mut out, "MISSING", &fixture, generator, &world.sprites)?;
        fixture.cache.put(missing, displaced);

        handler.apply_attachments(generator, &mut fixture.cache, center, &world.sprites, &world.alias)?;
        attachments_dump(&mut out, "FULL", &fixture, generator, &world.sprites)?;

        handler.reset_attachments(generator, center);
        let state = generator.chunk_states.entry((center.x, center.y)).or_default();
        state.attachments_partial_initialized = true;
        state.attachments_partial.push((center.x * 8 + 7, center.y * 8 + 3, 0));
        let east_sq = fixture.cache.get_mut(east_key).unwrap().get_grid_square_mut(0, 3, 0).unwrap();
        let floor = east_sq.get_floor(&world.sprites).ok_or("no east floor")?;
        east_sq.objects[floor].attach(overlay);
        handler.apply_attachments(generator, &mut fixture.cache, center, &world.sprites, &world.alias)?;
        attachments_dump(&mut out, "PARTIAL", &fixture, generator, &world.sprites)?;
        Ok(out)
    })
}

/// IsoChunk.LoadBrandNew -> WorldGenChunk.generateChunks -> IsoChunk.update: a real
/// 3x3 neighbourhood of requested chunks, each independently generated from its
/// original 2x2 lotpack group, followed by blending and attachments.
pub fn fullworld(root: &Path) -> Result<String, String> {
    use zomboid_engine::java::JavaHashMap;
    use zomboid_engine::zombie::core::random::rand_abstract::RandAbstract;
    use zomboid_engine::zombie::iso::cell_loader::LoadContext;
    use zomboid_engine::zombie::iso::iso_chunk::IsoChunk;
    use zomboid_engine::zombie::iso::iso_meta_grid::AlarmSettings;
    use zomboid_engine::zombie::iso::iso_world::IsoWorld;
    use zomboid_engine::zombie::iso::worldgen::prefab_structure::PrefabStructure;
    use zomboid_engine::zombie::iso::worldgen::runtime::WorldGenRuntime;
    use zomboid_engine::zombie::iso::worldgen::static_module::StaticModule;
    use zomboid_engine::zombie::iso::worldgen::world_gen_chunk::WorldGenCell;
    use zomboid_engine::zombie::iso::worldgen::world_gen_params::WorldGenParams;
    use zomboid_engine::zombie::sandbox_options::SandboxOptions;

    const BASE_X: i32 = 1034;
    const BASE_Y: i32 = 1458;
    let mut rand = RandAbstract::new(java::Random::new(12345));
    let alarm = AlarmSettings { last_stand: false, power_grid_exists: true, alarm: 1, alarm_decay: 1 };
    let mut world = IsoWorld::init(root, "Muldraugh, KY", &|_| false, &alarm, None, &mut rand)
        .map_err(|e| format!("IsoWorld.init: {e}"))?;
    let bounds = (
        world.meta_grid.min_x, world.meta_grid.max_x,
        world.meta_grid.min_y, world.meta_grid.max_y,
    );
    let mut sandbox = SandboxOptions::new();
    if let Ok(bytes) = zomboid_engine::vfs::read(&root.join("media/lua/shared/Sandbox/Apocalypse.lua")) {
        sandbox.load_game_file(&String::from_utf8_lossy(&bytes));
    }
    let mut params = WorldGenParams::default();
    params.set_seed_string("DeadOxideOracle");
    let mut runtime = WorldGenRuntime::from_game(
        root, "Muldraugh, KY", params, &sandbox, bounds,
        &world.sprites, &world.alias,
    )?;
    // WgPrefabsProbe.injectPrefab: identical tile refs and static-module bounds.
    let mut schematic = JavaHashMap::default();
    schematic.put("Floor".to_owned(), vec![vec![1, 3], vec![3, 1]]);
    schematic.put("FloorFurniture".to_owned(), vec![vec![0, 0], vec![0, 2]]);
    schematic.put("FloorOverlay".to_owned(), vec![vec![2, 0], vec![0, 2]]);
    schematic.put("Furniture".to_owned(), vec![vec![0, 2], vec![0, 0]]);
    let prefab = PrefabStructure::new(
        [2, 2],
        vec!["blends_natural_01_0".to_owned(), "vegetation_groundcover_01_0".to_owned(), "blends_natural_01_64".to_owned()],
        schematic, 0.0,
    );
    runtime.generator.static_modules.insert(0, StaticModule::new(
        None, Some(prefab), BASE_X * 8, BASE_X * 8 + 15, BASE_Y * 8, BASE_Y * 8 + 15,
    ));
    let IsoWorld { sprites, alias, tile_definitions, meta_grid, .. } = &mut world;
    let mut ctx = LoadContext {
        sprites,
        alias,
        glass_removed_window_sprite_map: &tile_definitions.glass_removed_window_sprite_map,
        smashed_window_sprite_map: &tile_definitions.smashed_window_sprite_map,
        meta_grid,
        rand: &mut rand,
        locked_houses: 6,
        missing_tiles: Vec::new(),
        lotpack_cache: None,
    };
    let mut chunks: Vec<IsoChunk> = Vec::new();
    for dx in -1..=1 {
        for dy in -1..=1 {
            let (wx, wy) = (BASE_X + dx, BASE_Y + dy);
            let mut errors = Vec::new();
            let mut ch = runtime.load_brand_new(
                wx, wy, &mut ctx, WorldGenCell { world_x: 0, world_y: 0 }, &mut [], &mut errors,
            )?;
            // The Java fixture finishes the loading phase before registering its 3x3 grid.
            ch.set_loaded();
            chunks.push(ch);
        }
    }
    drop(ctx);
    let mut out = String::new();
    fullworld_dump(&mut out, "GENERATED", &chunks, &runtime, &world.sprites)?;
    runtime.postprocess_loaded(&mut chunks, &mut world.sprites, &world.alias)?;
    fullworld_dump(&mut out, "POSTPROCESSED", &chunks, &runtime, &world.sprites)?;
    runtime.postprocess_loaded(&mut chunks, &mut world.sprites, &world.alias)?;
    fullworld_dump(&mut out, "REPEATED", &chunks, &runtime, &world.sprites)?;
    Ok(out)
}

fn fullworld_dump(
    out: &mut String,
    stage: &str,
    chunks: &[zomboid_engine::zombie::iso::iso_chunk::IsoChunk],
    runtime: &zomboid_engine::zombie::iso::worldgen::runtime::WorldGenRuntime,
    sprites: &IsoSpriteManager,
) -> Result<(), String> {
    let _ = writeln!(out, "STAGE {stage} SEED {}", runtime.generator.seed);
    for ch in chunks {
        let state = runtime.generator.chunk_states
            .get(&(ch.wx, ch.wy))
            .ok_or_else(|| format!("WorldGen runtime state missing: {},{}", ch.wx, ch.wy))?;
        let bit = |v: bool| i32::from(v);
        let _ = writeln!(out,
            "CHUNK {} {} LEVELS {} {} BLEND {} {} FLAGS {} {} {} {} ATTACH {} {} {} {} {} {}",
            ch.wx, ch.wy, ch.get_min_level(), ch.get_max_level(),
            bit(state.blending_done_full), bit(state.blending_done_partial),
            bit(state.blending_modified[0]), bit(state.blending_modified[1]),
            bit(state.blending_modified[2]), bit(state.blending_modified[3]),
            bit(state.attachments_done_full),
            bit(state.attachments_state[0]), bit(state.attachments_state[1]),
            bit(state.attachments_state[2]), bit(state.attachments_state[3]),
            bit(state.attachments_state[4]),
        );
        for x in 0..8 {
            for y in 0..8 {
                if let Some(sq) = ch.get_grid_square(x, y, 0) {
                    let _ = writeln!(out, "S {} {} {} {} ROOM {}{}",
                        ch.wx, ch.wy, x, y, sq.room_id, advanced_objects(sq, sprites));
                } else {
                    let _ = writeln!(out, "S {} {} {} {} null", ch.wx, ch.wy, x, y);
                }
            }
        }
    }
    Ok(())
}

/// IsoChunk.Save(v249) -> IsoChunk.LoadFromDiskOrBuffer on an 8x8 chunk.
/// The full native byte stream is compared against the original Java server.
pub fn savereload(root: &Path) -> Result<String, String> {
    use std::collections::{BTreeMap, HashMap};
    use zomboid_engine::java;
    use zomboid_engine::zombie::core::random::rand_abstract::RandAbstract;
    use zomboid_engine::zombie::iso::chunk_save::{ChunkSave, SaveLoadContext};
    use zomboid_engine::zombie::iso::iso_chunk::IsoChunk;
    use zomboid_engine::zombie::iso::iso_grid_square::IsoGridSquare;
    use zomboid_engine::zombie::iso::iso_meta_grid::AlarmSettings;
    use zomboid_engine::zombie::iso::iso_object::{IsoObject, ObjectKind};
    use zomboid_engine::zombie::iso::iso_world::IsoWorld;
    use zomboid_engine::zombie::inventory::inventory_item::{ItemFactory, ItemTemplate};
    use zomboid_engine::zombie::world::dictionary_data::DictionaryData;

    struct NoItems;
    impl ItemFactory for NoItems {
        fn create_item(&self, _: i16) -> Option<ItemTemplate> { None }
        fn is_obsolete(&self, _: &str) -> bool { false }
        fn media_data_from_index(&self, _: i16) -> Option<String> { None }
    }
    const X: i32 = 1034;
    const Y: i32 = 1458;
    let mut rng = RandAbstract::new(java::Random::new(12345));
    let alarm = AlarmSettings { last_stand: false, power_grid_exists: true, alarm: 1, alarm_decay: 1 };
    let mut world = IsoWorld::init(root, "Muldraugh, KY", &|_| false, &alarm, None, &mut rng)
        .map_err(|e| format!("IsoWorld.init: {e}"))?;
    let ground = world.sprites.get_sprite("blends_natural_01_0");
    let plant = world.sprites.get_sprite("vegetation_groundcover_01_0");
    let mut ch = IsoChunk::new(X, Y);
    ch.set_min_max_level(0, 0);
    for x in 0..8 {
        for y in 0..8 {
            let mut sq = IsoGridSquare::new(X * 8 + x, Y * 8 + y, 0);
            let mut floor = IsoObject::new(ObjectKind::IsoObject, Some(ground));
            if x == 0 && y == 0 { floor.name = Some("Grass".into()); }
            sq.objects.push(floor);
            if x == 0 && y == 0 { sq.objects.push(IsoObject::new(ObjectKind::IsoObject, Some(plant))); }
            if x == 1 && y == 0 {
                sq.objects.push(IsoObject::new(ObjectKind::IsoObject, Some(plant)));
                sq.objects.push(IsoObject::new(ObjectKind::IsoObject, Some(plant)));
            }
            ch.set_square(x, y, 0, Some(sq));
        }
    }
    ch.set_loaded();
    ch.save = Some(Box::new(ChunkSave {
        blending_done_partial: true,
        blending_modified: [true, false, true, false],
        blending_depth: [4, 0, 3, 0],
        attachments_done_full: false,
        attachments_state: [true, true, false, true, true],
        attachments_partial: Some(vec![(X * 8 + 1, Y * 8 + 2, 0)]),
        ..ChunkSave::default()
    }));
    let mut out = String::new();
    savereload_dump(&mut out, "BEFORE", &ch, &world.sprites)?;
    let original = ch.save_to_buffer(&world.sprites, None)?;
    let _ = writeln!(out, "BYTES {} CRC {}", original.len(), java::crc32(&original[17..]));
    out.push_str("HEX ");
    for b in &original { let _ = write!(out, "{b:02x}"); }
    out.push('\n');
    let dictionary = DictionaryData {
        version: 0, next_info_id: 0, next_object_name_id: 0, next_sprite_name_id: 0,
        mod_ids: Vec::new(), modules: Vec::new(), items: BTreeMap::new(),
        item_types: HashMap::new(), entities: BTreeMap::new(), entity_types: HashMap::new(),
        object_names: BTreeMap::new(), object_ids: HashMap::new(),
        sprite_names: BTreeMap::new(), sprite_ids: HashMap::new(),
        string_registers: Vec::new(), script_registers: Vec::new(),
    };
    // Persist the same native bytes to the real map/<wx>/<wy>.bin layout.
    // The temporary directory is independent of any existing Project Zomboid save.
    let temp = std::env::temp_dir().join(format!("deadoxide-wgsavereload-{}", std::process::id()));
    std::fs::create_dir(&temp).map_err(|e| format!("{}: {e}", temp.display()))?;
    let result = (|| -> Result<(), String> {
        IsoChunk::save_bytes_to_disk(&temp, X, Y, &original)?;
        let path = temp.join("map").join(X.to_string()).join(format!("{Y}.bin"));
        let on_disk = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let _ = writeln!(out, "DISK_EQUALS {}", original == on_disk);
        let mut ctx = SaveLoadContext {
            sprites: &mut world.sprites, alias: &world.alias,
            dictionary: &dictionary, items: &NoItems, meta_grid: &mut world.meta_grid,
            rand: &mut rng, has_lot_header: true,
        };
        let reloaded = IsoChunk::load_from_disk_or_buffer(&on_disk, X, Y, &mut ctx)?;
        drop(ctx);
        savereload_dump(&mut out, "RELOADED", &reloaded, &world.sprites)?;
        let again = reloaded.save_to_buffer(&world.sprites, None)?;
        let _ = writeln!(out, "RESAVE_EQUALS {}", original == again);
        Ok(())
    })();
    let _ = std::fs::remove_dir_all(&temp);
    result?;
    Ok(out)
}

fn savereload_dump(
    out: &mut String, stage: &str,
    ch: &zomboid_engine::zombie::iso::iso_chunk::IsoChunk,
    sprites: &IsoSpriteManager,
) -> Result<(), String> {
    let s = ch.save.as_deref().ok_or("missing save header")?;
    let bit = |b: bool| i32::from(b);
    let partial = s.attachments_partial.as_ref().map_or("null".to_owned(), |v| v.len().to_string());
    let _ = writeln!(out,
        "{stage} {} {} LEVELS {} {} BLEND {} {} {} {} {} {} ATTACH {} {} {} {} {} {} PARTIAL {}",
        ch.wx, ch.wy, ch.get_min_level(), ch.get_max_level(),
        bit(s.blending_done_full), bit(s.blending_done_partial),
        bit(s.blending_modified[0]), bit(s.blending_modified[1]),
        bit(s.blending_modified[2]), bit(s.blending_modified[3]),
        bit(s.attachments_done_full), bit(s.attachments_state[0]), bit(s.attachments_state[1]),
        bit(s.attachments_state[2]), bit(s.attachments_state[3]), bit(s.attachments_state[4]),
        partial,
    );
    for x in 0..8 {
        for y in 0..8 {
            if let Some(sq) = ch.get_grid_square(x, y, 0) {
                let _ = write!(out, "S {x} {y}");
                for obj in &sq.objects {
                    let name = obj.sprite.and_then(|v| sprites.sprite(v).name.as_deref()).unwrap_or("null");
                    let _ = write!(out, " | {} {name}", obj.kind.class_name());
                    if let Some(attachments) = &obj.attached_anim_sprite {
                        for sprite in attachments {
                            let name = sprites.sprite(*sprite).name.as_deref().unwrap_or("null");
                            let _ = write!(out, " + {name}");
                        }
                    }
                }
                out.push('\n');
            } else {
                let _ = writeln!(out, "S {x} {y} null");
            }
        }
    }
    Ok(())
}

/// IsoChunkMap.update / window-scroll scheduling compared with WgStreamWindowProbe.java.
/// The Java probe also verifies the real IsoChunkMap.setInitialPos/getWorldXMin origin.
pub fn streamwindow() -> Result<String, String> {
    use zomboid_engine::zombie::iso::streaming_window::ChunkWindow;
    let positions = [
        (255.2f32, 255.8f32), (256.1,255.8), (256.2,256.2), (256.2,256.2),
        (-0.01,-8.01), (-0.01,-8.01), (255.2,255.8),
    ];
    let mut out = String::new();
    let mut previous: Option<ChunkWindow> = None;
    for (i, (x, y)) in positions.into_iter().enumerate() {
        let win = ChunkWindow::for_player(x, y, 4)?;
        let (x0,y0,x1,y1) = win.cells();
        writeln!(out, "WINDOW {i} {} {} CELLS {x0} {y0} {x1} {y1}", win.cx, win.cy)
            .map_err(|e| e.to_string())?;
        if let Some(old) = previous {
            let d = old.diff(win);
            writeln!(out, "DIFF {} {} {}", d.load.len(), d.unload.len(), i32::from(d.teleport))
                .map_err(|e| e.to_string())?;
            for (cx,cy) in d.load { writeln!(out, "+ {cx} {cy}").map_err(|e| e.to_string())?; }
            let mut sorted = d.unload;
            sorted.sort_unstable();
            for (cx,cy) in sorted { writeln!(out, "- {cx} {cy}").map_err(|e| e.to_string())?; }
        }
        previous = Some(win);
    }
    Ok(out)
}

/// WorldGenZone(Rocks), Zone.contains across boundaries, negative coordinates and polygons.
pub fn specialzones() -> Result<String, String> {
    use zomboid_engine::zombie::iso::worldgen::zones::world_gen_zone::WorldGenZone;
    use zomboid_engine::zombie::iso::zones::zone::Zone;
    use zomboid_engine::zombie::iso::zones::zone_geometry_type::ZoneGeometryType;
    let mut zones = vec![
        WorldGenZone::new(Zone::new("default", "WorldGen", -16, -16, 0, 8, 8), None),
        WorldGenZone::new(Zone::new("off", "WorldGen", -4, 6, 0, 8, 8), Some(false)),
        WorldGenZone::new(Zone::new("on", "WorldGen", -4, 6, 1, 8, 8), Some(true)),
        WorldGenZone::new(Zone::new("polygon", "WorldGen", 10, 10, 0, 8, 8), Some(false)),
    ];
    zones[3].zone.geometry_type = ZoneGeometryType::Polygon;
    zones[3].zone.points = vec![10,10,17,10,17,17,10,17];
    let mut out = String::new();
    for (i,z) in zones.iter_mut().enumerate() {
        writeln!(out, "ZONE {i} rocks={}", i32::from(z.get_rocks())).map_err(|e| e.to_string())?;
        for x in [-17,-16,-15,-9,-8,-5,-4,-1,0,3,4,9,10,11,15,17,18] {
            for y in [-17,-16,-9,-8,5,6,7,13,14,17,18] {
                for level in [0,1] {
                    if z.contains(x,y,level) {
                        writeln!(out, "HIT {i} {x} {y} {level}").map_err(|e| e.to_string())?;
                    }
                }
            }
        }
    }
    Ok(out)
}
