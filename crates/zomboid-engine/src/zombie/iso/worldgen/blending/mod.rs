//! `zombie.iso.worldgen.blending.Blending`, including the directional handshake
//! with neighbouring loaded chunks and the deterministic terrain/tree transition.

use crate::zombie::iso::iso_grid_square::IsoGridSquare;
use crate::zombie::iso::iso_object::{IsoObject, ObjectKind};
use crate::zombie::iso::sprite::iso_sprite_manager::IsoSpriteManager;
use crate::zombie::core::tile_property_alias_map::TilePropertyAliasMap;

use super::chunks_cache::ChunksCache;
use super::utils::chunk_coord::ChunkCoord;
use super::world_gen_chunk::{WorldGenChunk, WorldGenChunkState};

/// Original enum declaration order; `opposite` and `plane` match the Java constants.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BlendDirection { North, South, West, East }

impl BlendDirection {
    pub const ALL: [Self; 4] = [Self::North, Self::South, Self::West, Self::East];
    pub fn index(self) -> usize { match self { Self::North => 0, Self::South => 1, Self::West => 2, Self::East => 3 } }
    pub fn opposite(self) -> Self { match self { Self::North => Self::South, Self::South => Self::North, Self::West => Self::East, Self::East => Self::West } }
    pub fn offset(self) -> (i32, i32) { match self { Self::North => (0,-1), Self::South => (0,1), Self::West => (-1,0), Self::East => (1,0) } }
    pub fn plane(self) -> (i32, i32) { match self { Self::North | Self::South => (0,1), Self::West | Self::East => (1,0) } }
}

/// Java's `Blending` has a stateless `WorldGenTile` and a two-sprite grass list.
#[derive(Default)]
pub struct Blending;

impl Blending {
    pub const PLANTS_ADDED: [&'static str; 2] = ["e_newgrass_1_40", "e_newgrass_1_42"];

    /// Apply all four directional handshakes in Java enum order.
    /// `ChunksCache` may contain a 3x3 region; neighbours not present or not loaded are skipped.
    pub fn apply_blending(
        &self,
        generator: &mut WorldGenChunk,
        chunks: &mut ChunksCache,
        key: ChunkCoord,
        sprites: &mut IsoSpriteManager,
        _alias: &TilePropertyAliasMap,
    ) -> Result<(), String> {
        for dir in BlendDirection::ALL {
            let index = dir.index();
            if generator.chunk_states.get(&(key.x, key.y))
                .is_some_and(|s| s.blending_modified[index]) { continue; }
            let (dx, dy) = dir.offset();
            let source_key = ChunkCoord::new(key.x + dx, key.y + dy);
            if !chunks.get(source_key).is_some_and(|ch| ch.loaded) { continue; }
            let current = generator.chunk_states.get(&(key.x, key.y)).cloned().unwrap_or_default();
            let opposite = dir.opposite();
            let secondary = ChunkCoord::new(key.x - dx, key.y - dy);
            if current.blending_done_partial && !chunks.get(secondary).is_some_and(|ch| ch.loaded) {
                continue;
            }
            if generator.chunk_states.get(&(source_key.x, source_key.y))
                .is_some_and(|state| state.blending_done_full) {
                self.remove_trees(generator, chunks, key, dir, sprites, &current)?;
                self.change_ground(generator, chunks, key, dir, sprites, &current)?;
            }
            generator.chunk_states.entry((key.x, key.y)).or_default().blending_modified[index] = true;
            generator.chunk_states.entry((source_key.x, source_key.y))
                .or_default().blending_modified[opposite.index()] = true;
        }
        Ok(())
    }

    /// `Blending.getSquare`: for partial chunks the origin is shifted by the recorded depth.
    fn square<'a>(
        &self, chunks: &'a ChunksCache, key: ChunkCoord, dir: BlendDirection,
        x: i32, y: i32, state: &WorldGenChunkState,
    ) -> Result<&'a IsoGridSquare, String> {
        let (wx, wy) = self.square_pos(key, dir, x, y, state);
        square_at(chunks, wx, wy).ok_or_else(|| format!(
            "Square is null at [{}, {}]+({}, {}) | {:?}", key.x * 8, key.y * 8, x, y, dir))
    }

    fn square_pos(&self, key: ChunkCoord, dir: BlendDirection, x: i32, y: i32, state: &WorldGenChunkState) -> (i32, i32) {
        let (bx, by) = (key.x * 8, key.y * 8);
        if state.blending_done_partial {
            match dir {
                BlendDirection::North => (bx + x, by + state.modification_depth[0] + y),
                BlendDirection::South => (bx + x, by + state.modification_depth[1] - y),
                BlendDirection::West => (bx + state.modification_depth[2] + x, by + y),
                BlendDirection::East => (bx + state.modification_depth[3] - x, by + y),
            }
        } else { (bx + x, by + y) }
    }

    /// `Blending.removeTrees`: consumes an RNG draw for every square, even those with no tree.
    pub fn remove_trees(
        &self, generator: &WorldGenChunk, chunks: &mut ChunksCache, key: ChunkCoord,
        dir: BlendDirection, sprites: &mut IsoSpriteManager, state: &WorldGenChunkState,
    ) -> Result<(), String> {
        let mut rnd = generator.params.get_random(key.x, key.y);
        for x in 0..8 {
            for y in 0..8 {
                let threshold = match dir {
                    BlendDirection::North => y * 10,
                    BlendDirection::South => if state.blending_done_partial { y * 10 } else { (7 - y) * 10 },
                    BlendDirection::West => x * 10,
                    BlendDirection::East => if state.blending_done_partial { x * 10 } else { (7 - x) * 10 },
                };
                if rnd.next_int_bound(100) < threshold { continue; }
                let (wx, wy) = self.square_pos(key, dir, x, y, state);
                // Java fetches the square even when it contains no tree. Null is an error.
                self.square(chunks, key, dir, x, y, state)?;
                let sq = square_at_mut(chunks, wx, wy).ok_or("missing blending square")?;
                if let Some(index) = sq.objects.iter().position(|o| matches!(o.kind, ObjectKind::IsoTree(_))) {
                    sq.objects.remove(index);
                    sq.properties_dirty = true;
                    if rnd.next_int_bound(100) >= 25 {
                        let grass = Self::PLANTS_ADDED[rnd.next_int_bound(2) as usize];
                        let sprite = sprites.get_sprite(grass);
                        sq.objects.push(IsoObject::new(ObjectKind::IsoObject, Some(sprite)));
                    }
                }
            }
        }
        Ok(())
    }

    /// `Blending.changeGround`: shares a fresh per-chunk RNG stream with all directions.
    pub fn change_ground(
        &self, generator: &mut WorldGenChunk, chunks: &mut ChunksCache, key: ChunkCoord,
        dir: BlendDirection, sprites: &mut IsoSpriteManager, state: &WorldGenChunkState,
    ) -> Result<(), String> {
        let mut rnd = generator.params.get_random(key.x, key.y);
        for i in 0..8 {
            let (x, y) = match dir {
                BlendDirection::North => (key.x * 8 + i, key.y * 8 - 1),
                BlendDirection::South => (key.x * 8 + i, key.y * 8 + 8),
                BlendDirection::West => (key.x * 8 - 1, key.y * 8 + i),
                BlendDirection::East => (key.x * 8 + 8, key.y * 8 + i),
            };
            let biome = generator.get_biome(x, y)?;
            let group = generator.tile.get_ground(&biome, &mut rnd);
            let Some(ground_name) = group.and_then(|g| g.tiles.first()) else { continue; };
            let ground_name = ground_name.to_owned();
            let depth = rnd.next_int_bound(4);
            for j in 0..depth {
                let (px, py) = dir.plane();
                let local_x = i * py + j * px;
                let local_y = i * px + j * py;
                self.square(chunks, key, dir, local_x, local_y, state)?;
                let (wx, wy) = self.square_pos(key, dir, local_x, local_y, state);
                let should_replace = square_at(chunks, wx, wy)
                    .and_then(|sq| sq.get_floor(sprites).and_then(|floor| sq.objects[floor].sprite))
                    .and_then(|sprite| sprites.sprite(sprite).name.as_deref())
                    .is_some_and(|name| name.contains("blends_natural_01"));
                if !should_replace { continue; }
                let spr = sprites.named_map.get(&ground_name).copied()
                    .ok_or_else(|| format!("Blending: missing tile definition {ground_name}"))?;
                let sq = square_at_mut(chunks, wx, wy).ok_or("missing blending square")?;
                generator.tile.set_ground(spr, sq, sprites);
                let plants: Vec<String> = Self::PLANTS_ADDED.iter().map(|p| (*p).to_owned()).collect();
                generator.tile.delete_named_tiles(sq, sprites, &plants);
                if let Some(plant) = generator.tile.get_plant(&biome, &mut rnd)
                    .and_then(|g| g.tiles.first()) {
                    let spr = sprites.get_sprite(plant);
                    sq.objects.push(IsoObject::new(ObjectKind::IsoObject, Some(spr)));
                    sq.properties_dirty = true;
                }
            }
        }
        Ok(())
    }
}

pub(crate) fn square_at(chunks: &ChunksCache, x: i32, y: i32) -> Option<&IsoGridSquare> {
    let key = ChunkCoord::new(x.div_euclid(8), y.div_euclid(8));
    chunks.get(key)?.get_grid_square(x.rem_euclid(8), y.rem_euclid(8), 0)
}

pub(crate) fn square_at_mut(chunks: &mut ChunksCache, x: i32, y: i32) -> Option<&mut IsoGridSquare> {
    let key = ChunkCoord::new(x.div_euclid(8), y.div_euclid(8));
    chunks.get_mut(key)?.get_grid_square_mut(x.rem_euclid(8), y.rem_euclid(8), 0)
}
