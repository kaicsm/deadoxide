//! zombie.iso.worldgen.WorldGenChunk: tile-level procedural generation for an owned 2x2 chunk cache.
//!
//! The engine passes singleton state (Lua tables, sprite manager, meta grid, world cell coords and
//! the biome map) explicitly. This preserves the Java algorithm without depending on IsoWorld.instance.

use std::collections::HashMap;

use crate::java::{self, JavaHashMap};
use crate::se::krka::kahlua::j2se::kahlua_table_impl::KahluaTableImpl;
use crate::se::krka::kahlua::vm::object::{Object, TableRef};
use crate::zombie::iso::cell_loader::LoadContext;
use crate::zombie::iso::iso_grid_square::IsoGridSquare;
use crate::zombie::iso::iso_object::{IsoObject, ObjectKind};
use crate::zombie::iso::sprite::iso_sprite_manager::IsoSpriteManager;
use crate::zombie::sandbox_options::SandboxOptions;

use super::biomes::biome::Biome;
use super::biomes::biome_registry::{BiomeRegistry, BiomeSelection, Selection};
use super::biomes::biome_type::{Bush, Hygrometry, Landscape, OreLevel, Plant, Temperature};
use super::biomes::feature_type::FeatureType;
use super::chunks_cache::ChunksCache;
use super::enums::tile_replacement::TileReplacement;
use super::enums::tile_replacement_ret_value::TileReplacementRetValue;
use super::maps::biome_map::{BiomeMap, BiomeMapType};
use super::prefab_structure::PrefabStructure;
use super::roads::road::Road;
use super::roads::road_generator::RoadGenerator;
use super::static_module::StaticModule;
use super::utils::chunk_coord::ChunkCoord;
use super::veins::veins::Veins;
use super::world_gen_params::WorldGenParams;
use super::world_gen_reader::WorldGenReader;
use super::world_gen_simplex_generator::WorldGenSimplexGenerator;
use super::world_gen_tile::{is_bush, is_grass, is_grass_like, is_ore, PendingTiles, WorldGenTile, ANY, NO_BUSH, NO_GRASS, NO_TREE, SUBBIOME};
use super::world_gen_utils::{can_place, matches_tile_pattern};
use super::zones::world_gen_zone::WorldGenZone;

/// Context supplied by IsoWorld when a WorldGenChunk is called. Java reads the current cell's
/// world coordinates to visit the neighbouring 21x21 vein cells; the caller gives them here.
#[derive(Clone, Copy, Debug)]
pub struct WorldGenCell {
    pub world_x: i32,
    pub world_y: i32,
}

/// Java's ChunkGenerationStatus.WORLDGEN and attachment/blending state, kept separately until
/// those fields are ported to the existing IsoChunk structure. The generated square objects are
/// written directly into IsoChunk regardless of this metadata.
#[derive(Clone, Debug)]
pub struct WorldGenChunkState {
    pub blending_done_full: bool,
    pub blending_done_partial: bool,
    pub attachments_done_full: bool,
    pub attachments_state: [bool; 5],
    pub generated_worldgen: bool,
    pub procedural_zombie_squares: Vec<(i32, i32, i32)>,
    pub attachments_partial: Vec<(i32, i32, i32)>,
    /// NORTH, SOUTH, WEST, EAST as in BlendDirection enum.
    pub modification_depth: [i32; 4],
}

impl Default for WorldGenChunkState {
    fn default() -> Self {
        Self {
            blending_done_full: false,
            blending_done_partial: false,
            attachments_done_full: false,
            attachments_state: [false; 5],
            generated_worldgen: false,
            procedural_zombie_squares: Vec::new(),
            attachments_partial: Vec::new(),
            // BlendDirection.NORTH/SOUTH/WEST/EAST.defaultDepth.
            modification_depth: [7, 0, 7, 0],
        }
    }
}

/// WorldGenChunk's constructor state, loaded by WorldGenReader from the original Lua tables.
pub struct WorldGenChunk {
    pub seed: i64,
    pub simplex: WorldGenSimplexGenerator,
    pub tile: WorldGenTile,
    pub registry: BiomeRegistry,
    pub biomes: JavaHashMap<Biome>,
    pub biomes_map: JavaHashMap<Biome>,
    pub landscape: Selection<Landscape>,
    pub plant: Selection<Plant>,
    pub bush: Selection<Bush>,
    pub temperature: Selection<Temperature>,
    pub hygrometry: Selection<Hygrometry>,
    pub ore_level: Selection<OreLevel>,
    pub static_modules: Vec<StaticModule>,
    pub veins: Veins,
    pub priorities: JavaHashMap<f64>,
    pub road_generators: Vec<RoadGenerator>,
    pub params: WorldGenParams,
    pub chunk_states: HashMap<(i32, i32), WorldGenChunkState>,
}

impl WorldGenChunk {
    /// WorldGenChunk.<init>(long): runLuaOverride is deliberately the caller's responsibility (the
    /// Lua VM and map paths live outside the engine); the already-overridden `worldgen` Kahlua
    /// table is passed here. All loader calls are made in the original constructor order.
    /// The road generators use the current meta grid's inclusive min/max cell bounds.
    pub fn from_worldgen_table(
        seed: i64,
        params: WorldGenParams,
        worldgen_table: &TableRef,
        sandbox: &SandboxOptions,
        meta_bounds: (i32, i32, i32, i32),
    ) -> Result<Self, String> {
        let reader = WorldGenReader { sandbox };
        // The Java constructor uses the *presence* of each override entry to choose the
        // literal table key, not the entry's value. The override entry is often a table.
        let key = |name: &str, fallback: &str| -> Result<String, String> {
            let value = KahluaTableImpl::rawget(worldgen_table, &Object::str(name))
                .map_err(|e| format!("{e:?}"))?;
            Ok(if matches!(value, Object::Nil) { fallback } else { name }.to_owned())
        };
        let biomes_key = key("biomes_override", "biomes")?;
        let biomes = filter_generated(reader.load_biomes(worldgen_table, &biomes_key).map_err(|e| format!("{e:?}"))?);
        let biomes_map = filter_generated(reader.load_biomes(worldgen_table, "biomes_map").map_err(|e| format!("{e:?}"))?);
        let selection = key("selection_override", "selection")?;
        let landscape = reader.load_selection(worldgen_table, &selection).map_err(|e| format!("{e:?}"))?;
        let plant = reader.load_selection(worldgen_table, &selection).map_err(|e| format!("{e:?}"))?;
        let bush = reader.load_selection(worldgen_table, &selection).map_err(|e| format!("{e:?}"))?;
        let temperature = reader.load_selection(worldgen_table, &selection).map_err(|e| format!("{e:?}"))?;
        let hygrometry = reader.load_selection(worldgen_table, &selection).map_err(|e| format!("{e:?}"))?;
        let ore_level = reader.load_selection(worldgen_table, &selection).map_err(|e| format!("{e:?}"))?;
        let static_modules = reader.load_static_modules(worldgen_table, "static_modules").map_err(|e| format!("{e:?}"))?;
        let veins = Veins::new(reader.load_veins_config(worldgen_table, "veins").map_err(|e| format!("{e:?}"))?);
        let priorities = reader.load_priorities(worldgen_table, "priorities").map_err(|e| format!("{e:?}"))?;
        let roads_config = reader.load_road_config(worldgen_table, "roads").map_err(|e| format!("{e:?}"))?;
        let mut road_generators = Vec::new();
        let mut offset = 1000i64;
        for config in roads_config.values() {
            let (min_x, max_x, min_y, max_y) = meta_bounds;
            road_generators.push(RoadGenerator::new(seed, config.clone(), offset, min_x, max_x, min_y, max_y, &params)?);
            offset += 1000;
        }
        Ok(Self {
            seed,
            simplex: WorldGenSimplexGenerator::new(seed),
            tile: WorldGenTile,
            registry: BiomeRegistry::default(),
            biomes,
            biomes_map,
            landscape,
            plant,
            bush,
            temperature,
            hygrometry,
            ore_level,
            static_modules,
            veins,
            priorities,
            road_generators,
            params,
            chunk_states: HashMap::new(),
        })
    }

    /// WorldGenChunk.getRoadGenerators.
    pub fn get_road_generators(&self) -> &[RoadGenerator] {
        &self.road_generators
    }

    /// WorldGenChunk.getBiome: the original six-field noise and selector through BiomeRegistry.
    pub fn get_biome(&mut self, x: i32, y: i32) -> Result<Biome, String> {
        let selection = self.registry.get(&self.biomes, self.simplex.noise(x as f64, y as f64), self.simplex.selector(x as f64, y as f64), &self.landscape, &self.plant, &self.bush, &self.temperature, &self.hygrometry, &self.ore_level)?;
        Ok(self.resolve_biome(selection, false))
    }

    /// WorldGenChunk.getMapBiome: null filter remains null rather than selecting a default biome.
    pub fn get_map_biome(&mut self, x: i32, y: i32, filter: Option<&str>) -> Result<Option<Biome>, String> {
        let selection = self.registry.get_filtered(&self.biomes_map, filter, self.simplex.noise(x as f64, y as f64), self.simplex.selector(x as f64, y as f64), &self.bush, &self.ore_level)?;
        Ok(selection.map(|selection| self.resolve_biome(selection, true)))
    }

    fn resolve_biome(&self, selection: BiomeSelection, mapped: bool) -> Biome {
        match selection {
            BiomeSelection::Default => Biome::default_biome(),
            BiomeSelection::Named(name) => {
                let map = if mapped { &self.biomes_map } else { &self.biomes };
                map.get(&name).cloned().unwrap_or_else(Biome::default_biome)
            }
        }
    }

    /// WorldGenChunk.generateChunks: shared pending arrays and x-major/y-minor order over four
    /// chunks. An empty level-0 square selects genRandomChunk, otherwise genMapChunk and cleanup.
    /// The `biome_map` parameter corresponds to IsoWorld.getBiomeMap (None when no raster is loaded).
    pub fn generate_chunks<R: java::JavaRandom>(
        &mut self,
        chunks: &mut ChunksCache,
        ctx: &mut LoadContext<R>,
        cell: WorldGenCell,
        biome_map: Option<&BiomeMap>,
        zones: &mut [WorldGenZone],
    ) -> Result<(), String> {
        let mut to_be_done = PendingTiles::new();
        let min_x = chunks.get_min_chunk_x();
        let min_y = chunks.get_min_chunk_y();
        for wx in min_x..min_x + 2 {
            for wy in min_y..min_y + 2 {
                let key = ChunkCoord::new(wx, wy);
                let Some(chunk) = chunks.get(key) else {
                    return Err(format!("NullPointerException: ChunksCache lacks chunk {wx},{wy}"));
                };
                let random = has_empty_squares_on_level_zero(chunk);
                if random {
                    self.gen_random_chunk(chunks, key, ctx, cell, &mut to_be_done)?;
                } else if let Some(map) = biome_map {
                    self.gen_map_chunk(chunks, key, ctx, cell, map, zones, &mut to_be_done)?;
                    self.clean_chunk(chunks, key, "Sand", "vegetation_groundcover_01", ctx.sprites, ctx.alias);
                    self.clean_chunk(chunks, key, "Road_*", "vegetation_groundcover_01", ctx.sprites, ctx.alias);
                }
            }
        }
        Ok(())
    }

    /// WorldGenChunk.genRandomChunk: marks the chunk WorldGen, computes road candidates (the Java
    /// loop intentionally replaces the set for every generator), and visits 64 squares in x/y order.
    fn gen_random_chunk<R: java::JavaRandom>(
        &mut self, chunks: &mut ChunksCache, key: ChunkCoord, ctx: &mut LoadContext<R>,
        cell: WorldGenCell, done: &mut PendingTiles,
    ) -> Result<(), String> {
        let Some(chunk) = chunks.get_mut(key) else { return Err("missing chunk".into()); };
        chunk.set_min_max_level(0, 0);
        let state = self.chunk_states.entry((key.x, key.y)).or_default();
        state.blending_done_full = true;
        state.blending_done_partial = false;
        state.attachments_done_full = false;
        state.attachments_state.fill(false);
        state.generated_worldgen = true;
        let mut roads: Vec<Road> = Vec::new();
        for generator in &self.road_generators {
            roads = generator.get_roads(key.x, key.y).into_iter().cloned().collect();
        }
        let min_x = key.x * 8;
        let min_y = key.y * 8;
        let mut generation_error = None;
        'squares: for x in min_x..min_x + 8 {
            for y in min_y..min_y + 8 {
                if let Err(error) = self.gen_random_square(chunks, key, x, y, &roads, done, ctx, cell) {
                    generation_error = Some(error);
                    break 'squares;
                }
            }
        }
        if let Some(error) = generation_error {
            // Java catches all square-generation exceptions, clears the entire chunk and moves on.
            eprintln!("WorldGenChunk.genRandomChunk: failed to load chunk {}/{key:?}: {error}", self.seed);
            for cx in 0..8 {
                for cy in 0..8 {
                    if let Some(chunk) = chunks.get_mut(key) {
                        chunk.set_square(cx, cy, 0, None);
                    }
                }
            }
        }
        Ok(())
    }

    /// WorldGenChunk.genRandomSquare: existing objects force partial blending. Empty squares are
    /// created, then a static module / road / random biome gets the tile, in that precedence.
    #[allow(clippy::too_many_arguments)]
    fn gen_random_square<R: java::JavaRandom>(
        &mut self, chunks: &mut ChunksCache, key: ChunkCoord, x: i32, y: i32,
        roads: &[Road], done: &mut PendingTiles, ctx: &mut LoadContext<R>, cell: WorldGenCell,
    ) -> Result<(), String> {
        let tile_x = x - key.x * 8;
        let tile_y = y - key.y * 8;
        let cc_x = x - chunks.get_min_tile_x();
        let cc_y = y - chunks.get_min_tile_y();
        if let Some(square) = chunks.get(key).and_then(|c| c.get_grid_square(tile_x, tile_y, 0)) {
            if !square.objects.is_empty() {
                let state = self.chunk_states.entry((key.x, key.y)).or_default();
                state.blending_done_full = false;
                state.blending_done_partial = true;
                state.modification_depth[0] = state.modification_depth[0].min(tile_y);
                state.modification_depth[1] = state.modification_depth[1].max(tile_y);
                state.modification_depth[2] = state.modification_depth[2].min(tile_x);
                state.modification_depth[3] = state.modification_depth[3].max(tile_x);
                return Ok(());
            }
        } else {
            chunks.get_mut(key).ok_or("missing chunk")?.set_square(tile_x, tile_y, 0, Some(IsoGridSquare::new(x, y, 0)));
        }
        if let Some(square) = chunks.get_mut(key).and_then(|c| c.get_grid_square_mut(tile_x, tile_y, 0)) {
            square.room_id = -1;
            square.room = None;
        }
        let mut rnd = self.params.get_random(key.x * 8 + x, key.y * 8 + y);
        let static_module = self.static_modules.iter().find(|m| x >= m.xmin && x <= m.xmax && y >= m.ymin && y <= m.ymax).cloned();
        match static_module {
            None => {
                let mut placed_road = false;
                let mut rnd_road = self.params.get_random(key.x * 8 + x, key.y * 8 + y);
                for road in roads {
                    if x >= road.a.x.min(road.b.x) && x <= road.a.x.max(road.b.x)
                        && y >= road.a.y.min(road.b.y) && y <= road.a.y.max(road.b.y)
                        && rnd_road.next_double() < road.probability {
                        let square = chunks.get_mut(key).and_then(|c| c.get_grid_square_mut(tile_x, tile_y, 0)).ok_or("missing square")?;
                        self.tile.set_road_tile(road, square, ctx, &mut rnd_road)?;
                        placed_road = true;
                    }
                }
                if !placed_road {
                    let biome = self.get_biome(x, y)?;
                    self.gen_random_tiles(chunks, key, x, y, &biome, done, cc_x, cc_y, ctx, cell, &mut rnd)?;
                }
            }
            Some(module) => {
                if let Some(biome) = &module.biome {
                    self.gen_random_tiles(chunks, key, x, y, biome, done, cc_x, cc_y, ctx, cell, &mut rnd)?;
                } else if let Some(prefab) = &module.prefab {
                    let biome = self.get_biome(x, y)?;
                    self.apply_prefab(prefab, &biome, chunks, key, x, y, cc_x, cc_y, done, module.xmin, module.ymin, ctx, &mut rnd)?;
                } else {
                    return Err("Need at least one of 'biome' or 'prefab' in WorldGenOverride.lua/worldgen.static_modules".into());
                }
            }
        }
        Ok(())
    }

    /// WorldGenChunk.genRandomTiles: resolve pending tiles, place each feature in enum order, and
    /// generate ore veins after the biome, using the same square RNG.
    #[allow(clippy::too_many_arguments)]
    fn gen_random_tiles<R: java::JavaRandom>(
        &mut self, chunks: &mut ChunksCache, key: ChunkCoord, x: i32, y: i32,
        initial: &Biome, done: &mut PendingTiles, cc_x: i32, cc_y: i32,
        ctx: &mut LoadContext<R>, cell: WorldGenCell, rnd: &mut java::Random,
    ) -> Result<(), String> {
        let mut sub_biomes: HashMap<usize, Biome> = HashMap::new();
        self.do_pending(chunks, key, x, y, cc_x, cc_y, done, Some(initial), Some(initial), &mut sub_biomes, ctx, rnd)?;
        let mut biome = initial.clone();
        for feature_type in FeatureType::ALL {
            if let Some(sub_biome) = sub_biomes.get(&feature_type.index()) {
                biome = sub_biome.clone();
            }
            let result = self.apply_biome(chunks, key, x, y, cc_x, cc_y, &biome, *feature_type, done, rnd, ctx)?;
            if result.retval == TileReplacement::Pending {
                self.do_pending(chunks, key, x, y, cc_x, cc_y, done, Some(&biome), Some(&biome), &mut sub_biomes, ctx, rnd)?;
                if let Some(sub_biome) = sub_biomes.get(&feature_type.index()) {
                    biome = sub_biome.clone();
                }
                self.apply_biome(chunks, key, x, y, cc_x, cc_y, &biome, *feature_type, done, rnd, ctx)?;
            }
        }
        self.apply_ore_veins(chunks, key, x, y, cell, ctx, rnd)?;
        Ok(())
    }

    /// WorldGenChunk.applyBiome: tile placement, FixStackableObjects, then a zombie-density RNG draw
    /// only when the tile actually succeeded.
    #[allow(clippy::too_many_arguments)]
    fn apply_biome<R: java::JavaRandom>(
        &mut self, chunks: &mut ChunksCache, key: ChunkCoord, x: i32, y: i32,
        cc_x: i32, cc_y: i32, biome: &Biome, feature_type: FeatureType,
        done: &mut PendingTiles, rnd: &mut java::Random, ctx: &mut LoadContext<R>,
    ) -> Result<TileReplacementRetValue, String> {
        let result = self.tile.set_tiles(biome, feature_type, chunks, key, x, y, 0, cc_x, cc_y, done, rnd, ctx)?;
        if result.retval == TileReplacement::Success {
            let square = chunks.get_mut(key).and_then(|c| c.get_grid_square_mut(x - key.x * 8, y - key.y * 8, 0)).ok_or("missing square")?;
            square.fix_stackable_objects(ctx.sprites, ctx.alias);
            self.generate_zombies(biome.zombies, x, y, 0, key, rnd);
        }
        Ok(result)
    }

    /// WorldGenChunk.generateZombies: only a successful draw adds a procedural square to the
    /// chunk's queue. Instantiating the zombie happens later in addZombieToSquare.
    fn generate_zombies(&mut self, chance: f32, x: i32, y: i32, z: i32, key: ChunkCoord, rnd: &mut java::Random) {
        if rnd.next_float() < chance {
            self.chunk_states.entry((key.x, key.y)).or_default().procedural_zombie_squares.push((x, y, z));
        }
    }

    /// WorldGenChunk.doPending: resolve the sentinels already written by an earlier tile group.
    /// Sub-biomes use a separate, position-seeded Random and do not consume the main stream.
    #[allow(clippy::too_many_arguments)]
    fn do_pending<R: java::JavaRandom>(
        &mut self, chunks: &mut ChunksCache, key: ChunkCoord, x: i32, y: i32,
        cc_x: i32, cc_y: i32, done: &mut PendingTiles, lookup: Option<&Biome>, ore: Option<&Biome>,
        sub_biomes: &mut HashMap<usize, Biome>, ctx: &mut LoadContext<R>, _rnd: &mut java::Random,
    ) -> Result<(), String> {
        for feature_type in FeatureType::ALL {
            let Some(tile) = done.get(*feature_type, cc_x, cc_y).map(str::to_owned) else {
                done.set(*feature_type, cc_x, cc_y, None);
                continue;
            };
            if tile == ANY {
                done.set(*feature_type, cc_x, cc_y, None);
                continue;
            }
            let square = chunks.get_mut(key).and_then(|c| c.get_grid_square_mut(x - key.x * 8, y - key.y * 8, 0)).ok_or("missing square")?;
            match tile.as_str() {
                SUBBIOME => {
                    // WorldGenChunk.doPending: loop is over FeatureType.values() and the first
                    // outer subType with a non-null mapping wins. The original indexes the INNER
                    // list using the OUTER map's size rather than the list's size.
                    let source = if *feature_type == FeatureType::Ore { ore } else { lookup };
                    if let Some(source) = source {
                        if !source.sub_biomes.is_empty() {
                            for sub_type in FeatureType::ALL {
                                if let Some(inner) = source.sub_biomes.get(*sub_type) {
                                    let mut rnd2 = self.params.get_random(key.x * 8 + x, key.y * 8 + y);
                                    let outer_size = FeatureType::ALL.iter().filter(|t| source.sub_biomes.get(**t).is_some()).count();
                                    let Some(candidates) = inner.get(*feature_type) else { break; };
                                    let choice = rnd2.next_int_bound(outer_size as i32) as usize;
                                    if let Some(biome) = candidates.get(choice) {
                                        sub_biomes.insert(sub_type.index(), biome.clone());
                                    }
                                    break;
                                }
                            }
                        }
                    }
                }
                NO_TREE => delete_first(square, |o| matches!(&o.kind, ObjectKind::IsoTree(_))),
                NO_BUSH => delete_first(square, |o| is_bush(o, ctx.sprites)),
                NO_GRASS => delete_first(square, |o| is_grass(o, ctx.sprites)),
                _ => {
                    delete_first(square, |o| matches!(&o.kind, ObjectKind::IsoTree(_)));
                    delete_first(square, |o| is_bush(o, ctx.sprites));
                    delete_first(square, |o| is_grass(o, ctx.sprites));
                    self.tile.apply_tile(&tile, square, ctx)?;
                }
            }
        }
        Ok(())
    }

    /// WorldGenChunk.applyOreVeins: the original visits a 21x21 area of cells in x-then-y order.
    fn apply_ore_veins<R: java::JavaRandom>(
        &mut self, chunks: &mut ChunksCache, key: ChunkCoord, x: i32, y: i32,
        cell: WorldGenCell, ctx: &mut LoadContext<R>, rnd: &mut java::Random,
    ) -> Result<(), String> {
        for px in -10..=10 {
            for py in -10..=10 {
                let veins = self.veins.get(cell.world_x + px, cell.world_y + py, &self.params);
                for vein in veins {
                    if vein.is_valid(x, y, rnd) {
                        let sq = chunks.get_mut(key).and_then(|c| c.get_grid_square_mut(x - key.x * 8, y - key.y * 8, 0)).ok_or("missing square")?;
                        self.tile.set_ore_tile(vein, sq, ctx, rnd)?;
                    }
                }
            }
        }
        Ok(())
    }

    /// WorldGenChunk.applyPrefab: repeated prefab tiles, then FixStackableObjects and a zombie draw.
    #[allow(clippy::too_many_arguments)]
    fn apply_prefab<R: java::JavaRandom>(
        &mut self, prefab: &PrefabStructure, biome: &Biome, chunks: &mut ChunksCache,
        key: ChunkCoord, x: i32, y: i32, cc_x: i32, cc_y: i32,
        done: &mut PendingTiles, xmin: i32, ymin: i32, ctx: &mut LoadContext<R>, rnd: &mut java::Random,
    ) -> Result<(), String> {
        if prefab.get_x() == 0 || prefab.get_y() == 0 {
            return Err("ArithmeticException: / by zero".into());
        }
        let prefab_x = (x - xmin).abs() % prefab.get_x();
        let prefab_y = (y - ymin).abs() % prefab.get_y();
        for &category in prefab.get_categories() {
            if prefab.has_category(category) {
                let tile_ref = prefab.get_tile_ref(category, prefab_x, prefab_y).ok_or("invalid prefab tile reference")?;
                if tile_ref == 0 {
                    if category == "Floor" {
                        self.tile.set_tiles(biome, FeatureType::Ground, chunks, key, x, y, 0, cc_x, cc_y, done, rnd, ctx)?;
                    }
                } else {
                    let name = prefab.get_tile(tile_ref - 1).ok_or("invalid prefab tile name")?;
                    let sq = chunks.get_mut(key).and_then(|c| c.get_grid_square_mut(x - key.x * 8, y - key.y * 8, 0)).ok_or("missing square")?;
                    self.tile.apply_tile(name, sq, ctx)?;
                }
            }
        }
        let sq = chunks.get_mut(key).and_then(|c| c.get_grid_square_mut(x - key.x * 8, y - key.y * 8, 0)).ok_or("missing square")?;
        sq.fix_stackable_objects(ctx.sprites, ctx.alias);
        self.generate_zombies(prefab.get_zombies(), x, y, 0, key, rnd);
        Ok(())
    }

    /// WorldGenChunk.genMapChunk: a map raster supplies an 8x8 zone, then replacements, mapped
    /// biome features and ore features are applied to the existing squares.
    fn gen_map_chunk<R: java::JavaRandom>(
        &mut self, chunks: &mut ChunksCache, key: ChunkCoord, ctx: &mut LoadContext<R>,
        cell: WorldGenCell, map: &BiomeMap, zones: &mut [WorldGenZone], done: &mut PendingTiles,
    ) -> Result<(), String> {
        let Some(biomes) = map.get_zones(key.x, key.y, BiomeMapType::Biome)? else { return Ok(()); };
        for x in key.x * 8..key.x * 8 + 8 {
            for y in key.y * 8..key.y * 8 + 8 {
                self.gen_map_square(chunks, key, x, y, cell, map, &biomes, zones, done, ctx)?;
            }
        }
        Ok(())
    }

    /// WorldGenChunk.genMapSquare: replacement happens before special `$random` tiles and feature
    /// regeneration. Existing objects are filtered by protection and allowed-floor rules.
    #[allow(clippy::too_many_arguments)]
    fn gen_map_square<R: java::JavaRandom>(
        &mut self, chunks: &mut ChunksCache, key: ChunkCoord, x: i32, y: i32,
        cell: WorldGenCell, map: &BiomeMap, biomes: &[i32], zones: &mut [WorldGenZone], done: &mut PendingTiles,
        ctx: &mut LoadContext<R>,
    ) -> Result<(), String> {
        let lx = x - key.x * 8;
        let ly = y - key.y * 8;
        let cx = x - chunks.get_min_tile_x();
        let cy = y - chunks.get_min_tile_y();
        let pixel = biomes[(ly * 8 + lx) as usize];
        self.replace_square(chunks, key, x, y, map, pixel, ctx)?;
        let rocks = zones
            .iter_mut()
            .find_map(|zone| {
                if zone.contains(x, y, 0) {
                    Some(zone.get_rocks())
                } else {
                    None
                }
            })
            .unwrap_or(true);
        let Some(square) = chunks.get(key).and_then(|c| c.get_grid_square(lx, ly, 0)) else { return Ok(()); };
        let Some(floor_index) = square.get_floor(ctx.sprites) else { return Ok(()); };
        let Some(lookup) = map.get_entry(pixel) else { return Ok(()); };
        let floor_name = square.objects[floor_index].sprite.and_then(|s| ctx.sprites.sprite(s).name.clone()).unwrap_or_default();
        let biome_filter = lookup.biome.as_deref();
        let ore_filter = lookup.ore.as_deref();
        if biome_filter == Some("$random") {
            // The original discards the square and starts genRandomSquare with an empty road set.
            chunks.get_mut(key).ok_or("missing chunk")?.set_square(lx, ly, 0, Some(IsoGridSquare::new(x, y, 0)));
            self.gen_random_square(chunks, key, x, y, &[], done, ctx, cell)?;
            return Ok(());
        }
        let mut rnd = self.params.get_random(key.x * 8 + x, key.y * 8 + y);
        let lookup_biome = self.get_map_biome(x, y, biome_filter)?;
        let lookup_ore = self.get_map_biome(x, y, ore_filter)?;
        let mut sub_biomes = HashMap::new();
        self.do_pending(chunks, key, x, y, cx, cy, done, lookup_biome.as_ref(), lookup_ore.as_ref(), &mut sub_biomes, ctx, &mut rnd)?;
        let initial_tiles = {
            let sq = chunks.get(key).and_then(|c| c.get_grid_square(lx, ly, 0)).ok_or("missing square")?;
            [find_first(sq, |o| matches!(&o.kind, ObjectKind::IsoTree(_))),
             find_first(sq, |o| is_bush(o, ctx.sprites)),
             find_first(sq, |o| is_grass(o, ctx.sprites))]
        };
        for feature_type in FeatureType::ALL {
            let index = match feature_type {
                FeatureType::Tree => Some(0), FeatureType::Bush => Some(1), FeatureType::Plant => Some(2), _ => None,
            };
            if let Some(Some(original_index)) = index.map(|i| initial_tiles[i]) {
                let Some(biome) = sub_biomes.get(&feature_type.index()).cloned().or_else(|| lookup_biome.clone()) else { return Ok(()); };
                if let Some(sq) = chunks.get(key).and_then(|c| c.get_grid_square(lx, ly, 0)) {
                    if let Some(current) = sq.objects.get(original_index) {
                        if let Some(sprite) = current.sprite {
                            let name = ctx.sprites.sprite(sprite).name.as_deref().unwrap_or("");
                            if self.is_protected(&biome.protected_list, name) {
                                continue;
                            }
                        }
                    }
                }
                if !biome.placements.as_ref().is_some_and(|p| can_place(p.get(*feature_type), &floor_name)) {
                    let sq = chunks.get_mut(key).and_then(|c| c.get_grid_square_mut(lx, ly, 0)).ok_or("missing square")?;
                    if original_index < sq.objects.len() { sq.objects.remove(original_index); }
                    continue;
                }
                let mut result = self.apply_biome(chunks, key, x, y, cx, cy, &biome, *feature_type, done, &mut rnd, ctx)?;
                if result.retval == TileReplacement::Pending {
                    self.do_pending(chunks, key, x, y, cx, cy, done, lookup_biome.as_ref(), lookup_ore.as_ref(), &mut sub_biomes, ctx, &mut rnd)?;
                    let Some(biome) = sub_biomes.get(&feature_type.index()).or(lookup_biome.as_ref()) else { return Ok(()); };
                    result = self.apply_biome(chunks, key, x, y, cx, cy, biome, *feature_type, done, &mut rnd, ctx)?;
                }
                let sq = chunks.get_mut(key).and_then(|c| c.get_grid_square_mut(lx, ly, 0)).ok_or("missing square")?;
                if result.retval == TileReplacement::Success {
                    // The Java loop increments its index even after deletion, so the next object
                    // can be skipped. Preserve this behavior by advancing by one after removal.
                    let mut i = 1;
                    while i < sq.objects.len() {
                        let object = &sq.objects[i];
                        if object.sprite != result.placed && (matches!(&object.kind, ObjectKind::IsoTree(_)) || is_bush(object, ctx.sprites) || is_grass_like(object, ctx.sprites)) {
                            sq.objects.remove(i);
                        }
                        i += 1;
                    }
                } else if result.retval == TileReplacement::Delete {
                    let mut i = 1;
                    while i < sq.objects.len() {
                        let obj = &sq.objects[i];
                        if matches!(&obj.kind, ObjectKind::IsoTree(_)) || is_bush(obj, ctx.sprites) || is_ore(obj, ctx.sprites) || is_grass_like(obj, ctx.sprites) {
                            sq.objects.remove(i);
                        }
                        i += 1;
                    }
                }
            }
        }
        let grasses = {
            let sq = chunks.get(key).and_then(|c| c.get_grid_square(lx, ly, 0)).ok_or("missing square")?;
            sq.objects.iter().enumerate().filter(|(_, o)| is_grass_like(o, ctx.sprites)).map(|(i, _)| i).collect::<Vec<_>>()
        };
        let eligible_for_ore = chunks.get(key).and_then(|c| c.get_grid_square(lx, ly, 0)).is_some_and(|s| s.objects.len().saturating_sub(grasses.len()) == 1);
        if eligible_for_ore {
            let Some(ore_biome) = sub_biomes.get(&FeatureType::Ore.index()).cloned().or(lookup_ore) else { return Ok(()); };
            if ore_biome.placements.as_ref().is_some_and(|p| can_place(p.get(FeatureType::Ore), &floor_name)) && rocks {
                let mut result = self.apply_biome(chunks, key, x, y, cx, cy, &ore_biome, FeatureType::Ore, done, &mut rnd, ctx)?;
                if result.retval == TileReplacement::Pending {
                    self.do_pending(chunks, key, x, y, cx, cy, done, lookup_biome.as_ref(), Some(&ore_biome), &mut sub_biomes, ctx, &mut rnd)?;
                    let Some(biome) = sub_biomes.get(&FeatureType::Ore.index()).cloned().or_else(|| lookup_biome.clone()) else { return Ok(()); };
                    result = self.apply_biome(chunks, key, x, y, cx, cy, &biome, FeatureType::Ore, done, &mut rnd, ctx)?;
                }
                if result.retval == TileReplacement::Success {
                    if let Some(sq) = chunks.get_mut(key).and_then(|c| c.get_grid_square_mut(lx, ly, 0)) {
                        for i in grasses.into_iter().rev() {
                            if i < sq.objects.len() { sq.objects.remove(i); }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// WorldGenChunk.replaceTiles: process every existing tile of a mapped 8x8 chunk, before
    /// feature generation. The new sprite is created through WorldGenTile.applyTile.
    pub fn replace_tiles<R: java::JavaRandom>(
        &mut self, chunks: &mut ChunksCache, key: ChunkCoord, map: &BiomeMap,
        ctx: &mut LoadContext<R>,
    ) -> Result<(), String> {
        let Some(pixels) = map.get_zones(key.x, key.y, BiomeMapType::Biome)? else { return Ok(()); };
        for x in key.x * 8..key.x * 8 + 8 {
            for y in key.y * 8..key.y * 8 + 8 {
                let pixel = pixels[((y - key.y * 8) * 8 + (x - key.x * 8)) as usize];
                self.replace_square(chunks, key, x, y, map, pixel, ctx)?;
            }
        }
        Ok(())
    }

    /// WorldGenChunk.replaceSquare: iterate the live objects, drawing one feature when a sprite
    /// name appears in the biome's replacements map.
    fn replace_square<R: java::JavaRandom>(
        &mut self, chunks: &mut ChunksCache, key: ChunkCoord, x: i32, y: i32,
        map: &BiomeMap, pixel: i32, ctx: &mut LoadContext<R>,
    ) -> Result<(), String> {
        let Some(lookup) = map.get_entry(pixel) else { return Ok(()); };
        let Some(biome) = self.get_map_biome(x, y, lookup.biome.as_deref())? else { return Ok(()); };
        let Some(replacements) = &biome.replacements else { return Ok(()); };
        if replacements.borrow().is_empty() { return Ok(()); }
        let lx = x - key.x * 8;
        let ly = y - key.y * 8;
        let mut rnd = self.params.get_random(key.x * 8 + x, key.y * 8 + y);
        let mut i = 0;
        loop {
            let current_name = chunks.get(key).and_then(|c| c.get_grid_square(lx, ly, 0))
                .and_then(|sq| sq.objects.get(i)).and_then(|o| o.sprite)
                .and_then(|spr| ctx.sprites.sprite(spr).name.clone());
            let Some(name) = current_name else { break; };
            let features = replacements.borrow().get(&name).cloned();
            if let Some(features) = features {
                let mut prefilter = 0.0f32;
                for feature in &features { prefilter += feature.probability.get_value(); }
                let feature = self.tile.find_feature(&features, prefilter, prefilter, &mut rnd);
                if let Some(feature) = feature {
                    if feature.probability.get_value() != 0.0 {
                        let group = feature.tile_groups.get(rnd.next_int_bound(feature.tile_groups.len() as i32) as usize);
                        if let Some(tile) = group.and_then(|g| g.tiles.first()) {
                            let sq = chunks.get_mut(key).and_then(|c| c.get_grid_square_mut(lx, ly, 0)).ok_or("missing square")?;
                            sq.objects.remove(i);
                            self.tile.apply_tile(tile, sq, ctx)?;
                            self.chunk_states.entry((key.x, key.y)).or_default().attachments_partial.push((x, y, 0));
                        }
                    }
                }
            }
            // The original increments i even if the current entry was removed.
            i += 1;
        }
        Ok(())
    }

    /// WorldGenChunk.priority: returns false if either tile is missing in the priority map.
    pub fn priority(&self, tile: &str, remote: &str) -> bool {
        match (self.priorities.get(tile), self.priorities.get(remote)) {
            (Some(a), Some(b)) => a < b,
            _ => false,
        }
    }

    /// WorldGenChunk.isProtected: Java's `String.matches` of its converted wildcard patterns.
    pub fn is_protected(&self, protected: &[String], tile: &str) -> bool {
        protected.iter().any(|pattern| matches_tile_pattern(pattern, tile))
    }

    /// WorldGenChunk.cleanChunk: remove matching groundcover from floors whose FloorMaterial
    /// matches the requested pattern. The Java `toRemove` list is not cleared between squares;
    /// the port uses sprite identity so repeated removal from an earlier square is harmless.
    pub fn clean_chunk(
        &self, chunks: &mut ChunksCache, key: ChunkCoord, material: &str, prefix: &str,
        sprites: &IsoSpriteManager, alias: &crate::zombie::core::tile_property_alias_map::TilePropertyAliasMap,
    ) -> usize {
        let mut cleaned = 0;
        for x in 0..8 {
            for y in 0..8 {
                let Some(sq) = chunks.get_mut(key).and_then(|c| c.get_grid_square_mut(x, y, 0)) else { continue; };
                let Some(floor) = sq.get_floor(sprites) else { continue; };
                let floor_material = sq.objects[floor].sprite
                    .and_then(|s| sprites.sprite(s).properties.get(alias, "FloorMaterial"));
                if floor_material.is_some_and(|fm| matches_tile_pattern(material, fm)) {
                    let before = sq.objects.len();
                    sq.objects.retain(|o| !o.sprite.and_then(|s| sprites.sprite(s).name.as_deref()).is_some_and(|name| name.starts_with(prefix)));
                    cleaned += before - sq.objects.len();
                }
            }
        }
        cleaned
    }
}

/// WorldGenChunk.<init>: filter the `loadBiomes` Java map with Collectors.toMap, excluding
/// entries whose generate flag is false. The new map preserves Java hash iteration semantics.
fn filter_generated(source: JavaHashMap<Biome>) -> JavaHashMap<Biome> {
    let mut result = JavaHashMap::default();
    for (key, biome) in source.iter() {
        if biome.generate { result.put(key.to_owned(), biome.clone()); }
    }
    result
}

/// IsoChunk.hasEmptySquaresOnLevelZero: a level-0 square is empty only if it is missing or
/// objectless *and* there are no nonempty squares below it down to minLevel.
fn has_empty_squares_on_level_zero(chunk: &crate::zombie::iso::iso_chunk::IsoChunk) -> bool {
    for y in 0..8 {
        for x in 0..8 {
            let empty = chunk.get_grid_square(x, y, 0).is_none_or(|sq| sq.objects.is_empty());
            if empty {
                let has_below = (chunk.get_min_level()..0).any(|z| chunk.get_grid_square(x, y, z).is_some_and(|sq| !sq.objects.is_empty()));
                if !has_below { return true; }
            }
        }
    }
    false
}

/// IsoGridSquare.getTree/getBush/getGrass: return the first matching object in insertion order.
fn find_first(square: &IsoGridSquare, pred: impl Fn(&IsoObject) -> bool) -> Option<usize> {
    square.objects.iter().position(pred)
}

/// IsoGridSquare.DeleteTileObject: a null object is a no-op; the port removes by index instead
/// of holding a Java object identity.
fn delete_first(square: &mut IsoGridSquare, pred: impl Fn(&IsoObject) -> bool) {
    if let Some(index) = find_first(square, pred) {
        square.objects.remove(index);
        square.properties_dirty = true;
    }
}
