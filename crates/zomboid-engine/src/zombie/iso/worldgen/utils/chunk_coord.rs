//! zombie.iso.worldgen.utils.ChunkCoord record: a chunk coordinate.

/// zombie.iso.worldgen.utils.ChunkCoord (x, y): one 10x10-square chunk of a cell (`BiomeRaster`'s grid).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ChunkCoord {
    pub x: i32,
    pub y: i32,
}

impl ChunkCoord {
    /// zombie.iso.worldgen.utils.ChunkCoord.ChunkCoord(int, int)
    pub fn new(x: i32, y: i32) -> Self {
        ChunkCoord { x, y }
    }

    /// zombie.iso.worldgen.utils.ChunkCoord.hashCode: the record's generated ObjectMethods hash -
    /// `h = 0; h = 31 * h + Integer.hashCode(x); h = 31 * h + y` (records start at 0, unlike
    /// Objects.hash/Arrays.hashCode which start at 1), wrapping like the int arithmetic it is.
    pub fn java_hash_code(&self) -> i32 {
        31i32.wrapping_mul(self.x).wrapping_add(self.y)
    }
}
