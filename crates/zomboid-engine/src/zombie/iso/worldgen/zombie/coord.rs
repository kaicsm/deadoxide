//! zombie.iso.worldgen.zombie.Coord record.

/// zombie.iso.worldgen.zombie.Coord (x, y): one generated Voronoi point - a `Random.nextDouble()`
/// per axis, offset by the coordinates of the section the point belongs to.
#[derive(Clone, Copy, Debug)]
pub struct Coord {
    pub x: f64,
    pub y: f64,
}

impl Coord {
    /// zombie.iso.worldgen.zombie.Coord's record constructor.
    pub fn new(x: f64, y: f64) -> Self {
        Coord { x, y }
    }
}
