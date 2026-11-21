//! zombie.iso.worldgen.zombie.ZombieVoronoiEntry record.

use super::closest_selection::ClosestSelectionType;

/// zombie.iso.worldgen.zombie.ZombieVoronoiEntry (numberPoints, closestPoint, scale, cutoff): one
/// `zombie_voronoi` Lua entry. The canonical record constructor takes the parsed ClosestSelection;
/// the String overload (`from_name`) goes through `ClosestSelectionType.valueOf`.
#[derive(Clone, Copy, Debug)]
pub struct ZombieVoronoiEntry {
    pub number_points: i32,
    pub closest_point: ClosestSelectionType,
    pub scale: f64,
    pub cutoff: f64,
}

impl ZombieVoronoiEntry {
    /// ZombieVoronoiEntry's String overload:
    /// `this(numberPoints, ClosestSelectionType.valueOf(closestPoint), scale, cutoff)` - the
    /// `valueOf` `java.lang.IllegalArgumentException` text as the `Err`.
    pub fn from_name(
        number_points: i32,
        closest_point: &str,
        scale: f64,
        cutoff: f64,
    ) -> Result<Self, String> {
        Ok(ZombieVoronoiEntry {
            number_points,
            closest_point: ClosestSelectionType::from_name(closest_point)?,
            scale,
            cutoff,
        })
    }
}
