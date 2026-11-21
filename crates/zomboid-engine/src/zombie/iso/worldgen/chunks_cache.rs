//! zombie.iso.worldgen.ChunksCache: a two-by-two group of chunks and its world-space bounds.

use std::collections::HashMap;

use crate::zombie::iso::iso_chunk::IsoChunk;
use crate::zombie::iso::iso_grid_square::IsoGridSquare;

use super::utils::chunk_coord::ChunkCoord;

/// zombie.iso.worldgen.ChunksCache: the original extends HashMap<ChunkCoord, IsoChunk>, so entries
/// are owned here by value. Bounds only expand on `put`, including when an existing key is replaced.
#[derive(Debug, Default)]
pub struct ChunksCache {
    chunks: HashMap<ChunkCoord, IsoChunk>,
    min_chunk_x: i32,
    max_chunk_x: i32,
    min_chunk_y: i32,
    max_chunk_y: i32,
    initialized: bool,
}

impl ChunksCache {
    /// ChunksCache.<init>: the Java bounds begin at Integer.MAX_VALUE / Integer.MIN_VALUE.
    pub fn new() -> Self {
        Self::default()
    }

    /// ChunksCache.put: update the extrema before inserting the chunk into the HashMap.
    pub fn put(&mut self, key: ChunkCoord, value: IsoChunk) -> Option<IsoChunk> {
        if !self.initialized {
            self.min_chunk_x = key.x;
            self.max_chunk_x = key.x;
            self.min_chunk_y = key.y;
            self.max_chunk_y = key.y;
            self.initialized = true;
        } else {
            self.min_chunk_x = self.min_chunk_x.min(key.x);
            self.max_chunk_x = self.max_chunk_x.max(key.x);
            self.min_chunk_y = self.min_chunk_y.min(key.y);
            self.max_chunk_y = self.max_chunk_y.max(key.y);
        }
        self.chunks.insert(key, value)
    }

    /// java.util.HashMap.get(ChunkCoord).
    pub fn get(&self, key: ChunkCoord) -> Option<&IsoChunk> {
        self.chunks.get(&key)
    }

    /// java.util.HashMap.get(ChunkCoord), with a mutable chunk reference.
    pub fn get_mut(&mut self, key: ChunkCoord) -> Option<&mut IsoChunk> {
        self.chunks.get_mut(&key)
    }

    /// ChunksCache.getMinChunkX.
    pub fn get_min_chunk_x(&self) -> i32 {
        if self.initialized { self.min_chunk_x } else { i32::MAX }
    }

    /// ChunksCache.getMaxChunkX.
    pub fn get_max_chunk_x(&self) -> i32 {
        if self.initialized { self.max_chunk_x } else { i32::MIN }
    }

    /// ChunksCache.getMinChunkY.
    pub fn get_min_chunk_y(&self) -> i32 {
        if self.initialized { self.min_chunk_y } else { i32::MAX }
    }

    /// ChunksCache.getMaxChunkY.
    pub fn get_max_chunk_y(&self) -> i32 {
        if self.initialized { self.max_chunk_y } else { i32::MIN }
    }

    /// ChunksCache.getMinTileX.
    pub fn get_min_tile_x(&self) -> i32 {
        if self.initialized { self.min_chunk_x * 8 } else { i32::MAX }
    }

    /// ChunksCache.getMaxTileX.
    pub fn get_max_tile_x(&self) -> i32 {
        if self.initialized { (self.max_chunk_x + 1) * 8 } else { i32::MIN }
    }

    /// ChunksCache.getMinTileY.
    pub fn get_min_tile_y(&self) -> i32 {
        if self.initialized { self.min_chunk_y * 8 } else { i32::MAX }
    }

    /// ChunksCache.getMaxTileY.
    pub fn get_max_tile_y(&self) -> i32 {
        if self.initialized { (self.max_chunk_y + 1) * 8 } else { i32::MIN }
    }

    /// ChunksCache.getGridSquare: cache-local coordinates, not absolute world coordinates. Java's
    /// integer division truncates toward zero; no Euclidean remainder is used here.
    pub fn get_grid_square(&self, local_x: i32, local_y: i32, z: i32) -> Option<&IsoGridSquare> {
        let cx = local_x / 8;
        let cy = local_y / 8;
        let chunk = self.get(ChunkCoord::new(self.get_min_chunk_x() + cx, self.get_min_chunk_y() + cy))?;
        chunk.get_grid_square(local_x - cx * 8, local_y - cy * 8, z)
    }

    /// ChunksCache.getGridSquare, mutable variant for the engine's owned square storage.
    pub fn get_grid_square_mut(&mut self, local_x: i32, local_y: i32, z: i32) -> Option<&mut IsoGridSquare> {
        let cx = local_x / 8;
        let cy = local_y / 8;
        let key = ChunkCoord::new(self.get_min_chunk_x() + cx, self.get_min_chunk_y() + cy);
        self.get_mut(key)?.get_grid_square_mut(local_x - cx * 8, local_y - cy * 8, z)
    }

    /// HashMap.containsKey.
    pub fn contains(&self, key: ChunkCoord) -> bool {
        self.chunks.contains_key(&key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounds_and_cross_chunk_square_lookup() {
        let mut cache = ChunksCache::new();
        let mut a = IsoChunk::new(4, -2);
        let mut b = IsoChunk::new(5, -2);
        a.set_square(7, 3, 0, Some(IsoGridSquare::new(39, -13, 0)));
        b.set_square(0, 3, 0, Some(IsoGridSquare::new(40, -13, 0)));
        cache.put(ChunkCoord::new(4, -2), a);
        cache.put(ChunkCoord::new(5, -2), b);
        assert_eq!((cache.get_min_tile_x(), cache.get_max_tile_x()), (32, 48));
        assert_eq!((cache.get_min_tile_y(), cache.get_max_tile_y()), (-16, -8));
        assert_eq!(cache.get_grid_square(7, 3, 0).map(|sq| sq.x), Some(39));
        assert_eq!(cache.get_grid_square(8, 3, 0).map(|sq| sq.x), Some(40));
    }
}
