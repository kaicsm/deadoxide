//! zombie.iso.worldgen.zombie.ClosestSelection / ClosestSelectionType: which of the sorted squared
//! distances to a cell's Voronoi points becomes its noise.

use std::cmp::Ordering;

use super::coord::Coord;

/// zombie.iso.worldgen.zombie.ClosestSelectionType: `FIRST, SECOND, SECOND_MINUS_FIRST` (declaration
/// order). The enum implements zombie.iso.worldgen.zombie.ClosestSelection
/// (`double getClosest(double, double, List&lt;Coord&gt;)`) and is that interface's only implementor,
/// so the port folds the interface into the enum (ZombieVoronoiEntry's `ClosestSelection` field
/// keeps the enum type).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClosestSelectionType {
    First,
    Second,
    SecondMinusFirst,
}

impl ClosestSelectionType {
    /// ClosestSelectionType.values(): the constants in declaration order.
    pub const ALL: &'static [ClosestSelectionType; 3] = &[
        ClosestSelectionType::First,
        ClosestSelectionType::Second,
        ClosestSelectionType::SecondMinusFirst,
    ];

    /// `Enum.name()`.
    pub fn name(&self) -> &'static str {
        match self {
            ClosestSelectionType::First => "FIRST",
            ClosestSelectionType::Second => "SECOND",
            ClosestSelectionType::SecondMinusFirst => "SECOND_MINUS_FIRST",
        }
    }

    /// ClosestSelectionType.valueOf(String): the constant of that name, else the original's
    /// `java.lang.IllegalArgumentException` (the exception's text as the `Err`).
    pub fn from_name(name: &str) -> Result<Self, String> {
        Self::ALL
            .iter()
            .copied()
            .find(|c| c.name() == name)
            .ok_or_else(|| {
                format!("java.lang.IllegalArgumentException: No enum constant ClosestSelectionType.{name}")
            })
    }

    /// ClosestSelectionType.getClosest(double, double, List&lt;Coord&gt;) - each constant's
    /// ClosestSelection override: every point's squared distance
    /// `(x - c.x()) * (x - c.x()) + (y - c.y()) * (y - c.y())`, sorted
    /// (`points.stream().map(..).sorted().toList()`, a stable sort in `Double.compareTo` order), then
    /// `get(0)` / `get(1)` / `get(1) - get(0)`. The list holds the points of 9 sections (at least 2
    /// whenever numberPoints is at least 1); a shorter one is the original's
    /// `java.lang.IndexOutOfBoundsException` (the exception's text as the `Err`).
    pub fn get_closest(&self, x: f64, y: f64, points: &[Coord]) -> Result<f64, String> {
        let mut distances: Vec<f64> = points
            .iter()
            .map(|c| (x - c.x) * (x - c.x) + (y - c.y) * (y - c.y))
            .collect();
        distances.sort_by(|a, b| java_double_compare(*a, *b));
        match self {
            ClosestSelectionType::First => list_get(&distances, 0),
            ClosestSelectionType::Second => list_get(&distances, 1),
            ClosestSelectionType::SecondMinusFirst => {
                // l.get(1) - l.get(0): the left operand first, like the original
                Ok(list_get(&distances, 1)? - list_get(&distances, 0)?)
            }
        }
    }
}

/// The `get(0)` / `get(1)` of the overrides (`java.util.ImmutableCollections`' index-checked
/// `List.get`, the JDK 9+ `Objects.checkIndex` message): the element, else the original's
/// `java.lang.IndexOutOfBoundsException` (the exception's text as the `Err`, like
/// `ClosestSelectionType::from_name`'s `IllegalArgumentException`).
fn list_get(distances: &[f64], index: usize) -> Result<f64, String> {
    distances.get(index).copied().ok_or_else(|| {
        let length = distances.len();
        format!("java.lang.IndexOutOfBoundsException: Index {index} out of bounds for length {length}")
    })
}

/// `Double.compareTo`, the comparator of `Stream.sorted()` on the boxed distances: numeric order,
/// except that `-0.0` orders below `+0.0` and NaN - equal to NaN - orders above every other value
/// (including `Double.POSITIVE_INFINITY`).
fn java_double_compare(a: f64, b: f64) -> Ordering {
    match (a.is_nan(), b.is_nan()) {
        (true, true) => Ordering::Equal,
        (true, false) => Ordering::Greater,
        (false, true) => Ordering::Less,
        (false, false) => {
            if a < b {
                Ordering::Less
            } else if a > b {
                Ordering::Greater
            } else if a == 0.0 {
                // a == b numerically; the only tie Double.compareTo breaks is the two zeros
                match (a.is_sign_negative(), b.is_sign_negative()) {
                    (false, true) => Ordering::Greater,
                    (true, false) => Ordering::Less,
                    _ => Ordering::Equal,
                }
            } else {
                Ordering::Equal
            }
        }
    }
}
