//! zombie.iso.worldgen.utils.CellCoord record: a world cell coordinate.

/// zombie.iso.worldgen.utils.CellCoord (x, y): a cell of the 300x300-square meta grid (`BiomeMap`'s cache key).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CellCoord {
    pub x: i32,
    pub y: i32,
}

impl CellCoord {
    /// zombie.iso.worldgen.utils.CellCoord.CellCoord(int, int)
    pub fn new(x: i32, y: i32) -> Self {
        CellCoord { x, y }
    }

    /// zombie.iso.worldgen.utils.CellCoord.hashCode: the record's generated ObjectMethods hash -
    /// `h = 0; h = 31 * h + Integer.hashCode(x); h = 31 * h + y` (records start at 0, unlike
    /// Objects.hash/Arrays.hashCode which start at 1), wrapping like the int arithmetic it is.
    pub fn java_hash_code(&self) -> i32 {
        31i32.wrapping_mul(self.x).wrapping_add(self.y)
    }
}
