//! zombie.iso.worldgen.utils.triangulation.EdgeDistancePack: an edge with the distance of a point to it.

use std::cmp::Ordering;

use super::edge2d::Edge2D;

/// zombie.iso.worldgen.utils.triangulation.EdgeDistancePack: `Comparable` by distance. The sorts of the
/// triangulation (`Arrays.sort` of these) go through `compare_to`; nothing else needs the comparison, so
/// `java_double_compare` lives here as the single implementation Triangle2D and TriangleSoup sort by.
#[derive(Clone, Debug)]
pub struct EdgeDistancePack {
    pub edge: Edge2D,
    pub distance: f64,
}

/// java.lang.Double.compare(double, double): IEEE order first (`a < b`, then `a > b`), and for the values
/// both comparisons leave undecided the bit patterns of `Double.doubleToLongBits`, compared as SIGNED longs —
/// which makes NaN (canonicalized to 0x7ff8000000000000 like doubleToLongBits does, above every finite and
/// infinite bit pattern) the largest value and -0.0 (0x8000000000000000, i64::MIN) less than +0.0.
pub(crate) fn java_double_compare(a: f64, b: f64) -> Ordering {
    if a < b {
        return Ordering::Less;
    }
    if a > b {
        return Ordering::Greater;
    }
    const CANONICAL_NAN: u64 = 0x7ff8_0000_0000_0000;
    let a = (if a.is_nan() { CANONICAL_NAN } else { a.to_bits() }) as i64;
    let b = (if b.is_nan() { CANONICAL_NAN } else { b.to_bits() }) as i64;
    a.cmp(&b)
}

impl EdgeDistancePack {
    /// zombie.iso.worldgen.utils.triangulation.EdgeDistancePack.EdgeDistancePack(Edge2D, double)
    pub fn new(edge: Edge2D, distance: f64) -> Self {
        EdgeDistancePack { edge, distance }
    }

    /// The probe's `new EdgeDistancePack(null, distance)`: a pack whose edge is never read (compareTo only
    /// compares the distance). The port's Edge2D cannot be null, so this marks it unreachable instead.
    pub fn new_null(distance: f64) -> Self {
        EdgeDistancePack {
            edge: Edge2D {
                a: std::rc::Rc::new(super::vector2d::Vector2D::new(f64::NAN, f64::NAN)),
                b: std::rc::Rc::new(super::vector2d::Vector2D::new(f64::NAN, f64::NAN)),
            },
            distance,
        }
    }

    /// zombie.iso.worldgen.utils.triangulation.EdgeDistancePack.compareTo:
    /// `Double.compare(this.distance, o.distance)`.
    pub fn compare_to(&self, o: &EdgeDistancePack) -> Ordering {
        java_double_compare(self.distance, o.distance)
    }
}
