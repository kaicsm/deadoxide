//! The IsoChunk.LoadBrandNew -> WorldGenChunk.generateChunks -> IsoChunk.update pipeline.
//!
//! Each requested brand-new chunk gets a fresh 2x2 group of lot-loaded chunks. Java discards
//! the other three chunks after generating the group; keeping them would accidentally change
//! the RNG, the map state, and the blending state of later chunk requests.

use std::path::Path;
use std::rc::Rc;

use crate::se::krka::kahlua::j2se::kahlua_table_impl::KahluaTableImpl;
use crate::se::krka::kahlua::vm::object::Object;
use crate::java::JavaRandom;
use crate::zombie::core::tile_property_alias_map::TilePropertyAliasMap;
use crate::zombie::game_window::ClientStartup;
use crate::zombie::iso::cell_loader::{self, LoadContext};
use crate::zombie::iso::iso_chunk::IsoChunk;
use crate::zombie::iso::chunk_save::ChunkSave;
use crate::zombie::iso::sprite::iso_sprite_manager::IsoSpriteManager;
use crate::zombie::lua::exposed_game;
use crate::zombie::lua::lua_manager::LuaManager;
use crate::zombie::sandbox_options::SandboxOptions;

use super::attachments::AttachmentsHandler;
use super::blending::Blending;
use super::chunks_cache::ChunksCache;
use super::maps::biome_map::BiomeMap;
use super::utils::chunk_coord::ChunkCoord;
use super::world_gen_chunk::{WorldGenCell, WorldGenChunk, WorldGenChunkState};
use super::world_gen_params::WorldGenParams;
use super::zones::world_gen_zone::WorldGenZone;

/// Stateful WorldGen runtime. The map raster cache and the state of *kept* chunks survive
/// separate chunk requests, unlike the three pooled companions of each brand-new load.
pub struct WorldGenRuntime {
    pub generator: WorldGenChunk,
    pub biome_map: BiomeMap,
    pub blending: Blending,
    pub attachments: AttachmentsHandler,
}

impl WorldGenRuntime {
    /// IsoWorld's WorldGenChunk / BiomeMap / AttachmentsHandler initialization, using the
    /// original Lua `worldgen` and `biome_map_config` globals and the selected game's map.
    /// Call only after tile definitions have been loaded into `sprites`.
    pub fn from_game(
        root: &Path,
        map_names: &str,
        params: WorldGenParams,
        sandbox: &SandboxOptions,
        meta_bounds: (i32, i32, i32, i32),
        sprites: &IsoSpriteManager,
        alias: &TilePropertyAliasMap,
    ) -> Result<Self, String> {
        let startup = Rc::new(ClientStartup::init(root)
            .map_err(|e| format!("WorldGen client startup: {e}"))?);
        let mut lua = LuaManager::init_after(root, Some(Rc::clone(&startup)))
            .map_err(|e| format!("WorldGen LuaManager: {e:?}"))?;
        exposed_game::init_shared(&lua.statics)
            .map_err(|e| format!("WorldGen initShared: {e}"))?;
        for dir in ["shared", "server"] {
            lua.load_dir_base(dir)
                .map_err(|e| format!("WorldGen LoadDirBase({dir}): {e:?}"))?;
        }
        let table = KahluaTableImpl::rawget(&lua.env, &Object::str("worldgen"))
            .map_err(|e| format!("WorldGen table: {e:?}"))?;
        let Object::Table(table) = table else {
            return Err("WorldGen: the Lua `worldgen` table is missing".into());
        };
        let biome_map = BiomeMap::from_lua_environment(map_names, &startup.fs, &lua.env, sandbox)?;
        let generator = WorldGenChunk::from_worldgen_table(
            params.get_seed(), params, &table, sandbox, meta_bounds,
        )?;
        let mut attachments = AttachmentsHandler::new();
        attachments.load_attachments(sprites, alias)?;
        Ok(Self { generator, biome_map, blending: Blending, attachments })
    }

    /// IsoChunk.LoadBrandNew: floorDiv(wx, 2) * 2 selects the 2x2 generation corner.
    /// Reload all four source chunks from their lotpacks *for each requested chunk*, run
    /// WorldGenChunk.generateChunks, and retain only the requested chunk's state and objects.
    pub fn load_brand_new<R: JavaRandom>(
        &mut self,
        wx: i32,
        wy: i32,
        ctx: &mut LoadContext<R>,
        cell: WorldGenCell,
        zones: &mut [WorldGenZone],
        errors: &mut Vec<String>,
    ) -> Result<IsoChunk, String> {
        let min_x = wx.div_euclid(2) * 2;
        let min_y = wy.div_euclid(2) * 2;
        let mut cache = ChunksCache::new();
        for x in min_x..min_x + 2 {
            for y in min_y..min_y + 2 {
                let mut chunk = IsoChunk::new(x, y);
                cell_loader::load_cell_binary_chunk(ctx, x, y, &mut chunk, errors);
                cache.put(ChunkCoord::new(x, y), chunk);
            }
        }
        // The three companion chunks are returned to IsoChunkMap.chunkStore by Java.
        // Their generation metadata must not overwrite states from previously kept chunks.
        let retained_states = std::mem::take(&mut self.generator.chunk_states);
        let generation = self.generator.generate_chunks(
            &mut cache, ctx, cell, Some(&self.biome_map), zones,
        );
        let selected_state = self.generator.chunk_states.remove(&(wx, wy));
        self.generator.chunk_states = retained_states;
        generation?;
        if let Some(state) = selected_state {
            self.generator.chunk_states.insert((wx, wy), state);
        }
        cache.remove(ChunkCoord::new(wx, wy))
            .ok_or_else(|| format!("WorldGen: missing generated chunk {wx},{wy}"))
    }

    /// IsoChunk.LoadFromDiskOrBufferInternal: recover blending and attachment handshake flags
    /// from a saved chunk before its next IsoChunk.update, rather than resetting to defaults.
    pub fn register_saved_chunk(&mut self, chunk: &IsoChunk) {
        let Some(save) = &chunk.save else { return; };
        self.generator.chunk_states.insert((chunk.wx, chunk.wy), WorldGenChunkState {
            blending_done_full: save.blending_done_full,
            blending_done_partial: save.blending_done_partial,
            blending_modified: save.blending_modified,
            attachments_done_full: save.attachments_done_full,
            attachments_state: save.attachments_state,
            modification_depth: save.blending_depth.map(i32::from),
            attachments_partial: save.attachments_partial.clone().unwrap_or_default(),
            attachments_partial_initialized: save.attachments_partial.is_some(),
            ..WorldGenChunkState::default()
        });
    }

    /// IsoChunk.update's WorldGen portion. All chunks are temporarily moved into a cache
    /// so directional handshakes can mutate *both* neighbours' live squares and state.
    /// The original input order is preserved and chunks are restored even after an error.
    /// No save files are written by this operation.
    pub fn postprocess_loaded(
        &mut self,
        chunks: &mut [IsoChunk],
        sprites: &mut IsoSpriteManager,
        alias: &TilePropertyAliasMap,
    ) -> Result<(), String> {
        self.postprocess_selected(chunks, sprites, alias, None)
    }

    /// IsoChunkMap.updateInternal: when a new chunk is integrated, only its own
    /// WorldGen handshakes and those of the adjacent chunks can become newly
    /// satisfiable. Unrelated chunks retain the completion flags from prior updates.
    /// The 3x3 expansion is also required for attachments, which wait for all
    /// neighbours. The full-region method above is retained for initial loading.
    pub fn postprocess_near(
        &mut self,
        chunks: &mut [IsoChunk],
        sprites: &mut IsoSpriteManager,
        alias: &TilePropertyAliasMap,
        touched: &std::collections::HashSet<(i32, i32)>,
    ) -> Result<(), String> {
        if touched.is_empty() { return Ok(()); }
        self.postprocess_selected(chunks, sprites, alias, Some(touched))
    }

    fn postprocess_selected(
        &mut self,
        chunks: &mut [IsoChunk],
        sprites: &mut IsoSpriteManager,
        alias: &TilePropertyAliasMap,
        touched: Option<&std::collections::HashSet<(i32, i32)>>,
    ) -> Result<(), String> {
        let keys: Vec<_> = chunks.iter()
            .map(|ch| ChunkCoord::new(ch.wx, ch.wy)).collect();
        let mut unique = std::collections::HashSet::new();
        for key in &keys {
            if !unique.insert(*key) {
                return Err(format!("WorldGen: duplicate chunk {key:?}"));
            }
        }
        let mut cache = ChunksCache::new();
        for chunk in chunks.iter_mut() {
            let key = ChunkCoord::new(chunk.wx, chunk.wy);
            cache.put(key, std::mem::take(chunk));
        }
        let mut failure = None;
        // IsoChunkMap.updateInternal visits the grid in (y, x) order. The
        // initial full-region path retains its original iteration order for
        // already validated oracles; incremental updates use the live grid order.
        let mut targets: Vec<_> = keys.iter().copied().filter(|key| {
            touched.is_none_or(|changed| changed.iter().any(|&(x, y)| {
                (key.x - x).abs() <= 1 && (key.y - y).abs() <= 1
            }))
        }).collect();
        if touched.is_some() { targets.sort_by_key(|key| (key.y, key.x)); }
        for key in targets {
            if !cache.get(key).is_some_and(|chunk| chunk.loaded) { continue; }
            if let Err(e) = self.generator.update_worldgen_chunk(
                &mut cache, key, &self.blending, &self.attachments, sprites, alias,
            ) {
                failure = Some(format!("WorldGen chunk {},{}: {e}", key.x, key.y));
                break;
            }
        }
        // IsoChunk.Save reads these handshake flags from the chunk itself. Mirror the
        // generated and previously loaded chunks into their native save headers.
        for (slot, key) in chunks.iter_mut().zip(keys) {
            if let Some(mut chunk) = cache.remove(key) {
                if let Some(state) = self.generator.chunk_states.get(&(key.x, key.y)) {
                    let save = chunk.save.get_or_insert_with(|| Box::new(ChunkSave::default()));
                    save.blending_done_full = state.blending_done_full;
                    save.blending_done_partial = state.blending_done_partial;
                    save.blending_modified = state.blending_modified;
                    save.blending_depth = state.modification_depth.map(|v| v as i8);
                    save.attachments_done_full = state.attachments_done_full;
                    save.attachments_state = state.attachments_state;
                    save.attachments_partial = state.attachments_partial_initialized
                        .then(|| state.attachments_partial.clone());
                }
                *slot = chunk;
            }
        }
        match failure { Some(e) => Err(e), None => Ok(()) }
    }
}
