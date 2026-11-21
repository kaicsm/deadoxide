//! zombie.iso.worldgen.utils.SquareCoord record: an iso square coordinate.

use crate::java_nio::{ByteBuffer, ByteBufferWriter};

/// zombie.iso.worldgen.utils.SquareCoord (x, y, z): one square of the world; `IsoGridSquare.getCoords` builds
/// it, `IsoChunk` saves the pending-attachment squares through `load`/`save`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SquareCoord {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl SquareCoord {
    /// zombie.iso.worldgen.utils.SquareCoord.SquareCoord(int, int, int)
    pub fn new(x: i32, y: i32, z: i32) -> Self {
        SquareCoord { x, y, z }
    }

    /// zombie.iso.worldgen.utils.SquareCoord.load(ByteBuffer): three `getInt` in x, y, z order. The original's
    /// `ByteBuffer.getInt` throws BufferUnderflowException past the limit; the port's read buffer returns it as
    /// `Err` instead (first read that underflows wins).
    pub fn load(bb: &mut ByteBuffer) -> Result<Self, String> {
        let x = bb.get_int()?;
        let y = bb.get_int()?;
        let z = bb.get_int()?;
        Ok(SquareCoord::new(x, y, z))
    }

    /// zombie.iso.worldgen.utils.SquareCoord.save(ByteBuffer): three `putInt` in x, y, z order. The original
    /// writes into the same `java.nio.ByteBuffer` it reads from; the port's `ByteBuffer` is read-only, so the
    /// write side is the `ByteBufferWriter` (`put_int` there returns nothing, and so does this).
    pub fn save(&self, bb: &mut ByteBufferWriter) {
        bb.put_int(self.x);
        bb.put_int(self.y);
        bb.put_int(self.z);
    }

    /// zombie.iso.worldgen.utils.SquareCoord.hashCode: the record's generated ObjectMethods hash -
    /// `h = 0; h = 31 * h + Integer.hashCode(x); h = 31 * h + y; h = 31 * h + z` (records start at 0,
    /// unlike Objects.hash/Arrays.hashCode which start at 1), wrapping like the int arithmetic it is.
    pub fn java_hash_code(&self) -> i32 {
        31i32
            .wrapping_mul(31i32.wrapping_mul(self.x).wrapping_add(self.y))
            .wrapping_add(self.z)
    }
}
