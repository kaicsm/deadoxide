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
    let _ = writeln!(out, "GENERATE {BASE_X} {BASE_Y} SEED {seed}");
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
