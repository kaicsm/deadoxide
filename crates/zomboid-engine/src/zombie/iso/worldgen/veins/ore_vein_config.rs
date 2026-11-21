//! zombie.iso.worldgen.veins.OreVeinConfig.

use crate::zombie::iso::worldgen::biomes::tile_group::TileGroup;

/// zombie.iso.worldgen.veins.OreVeinConfig: one vein type's parameters - the ore tiles, the center blob's
/// radius and probability, the arms' count/distance/angle ranges and probability, and the probability the
/// vein appears in a cell at all. The Java getters (`getTiles`, `getCenterRadius`, ...) read the fields the
/// port makes public.
#[derive(Clone, Debug)]
pub struct OreVeinConfig {
    pub tiles: Vec<TileGroup>,
    pub center_radius: i32,
    pub center_prob: f32,
    pub arms_amount_min: i32,
    pub arms_amount_max: i32,
    pub arms_dist_min: i32,
    pub arms_dist_max: i32,
    pub arms_delta_angle: i32,
    pub arms_prob: f32,
    pub probability: f32,
}

impl OreVeinConfig {
    /// OreVeinConfig's constructor, the Java parameter order (tiles, centerRadius, centerProb,
    /// armsAmountMin, armsAmountMax, armsDistMin, armsDistMax, armsDeltaAngle, armsProb, probability).
    pub fn new(
        tiles: Vec<TileGroup>,
        center_radius: i32,
        center_prob: f32,
        arms_amount_min: i32,
        arms_amount_max: i32,
        arms_dist_min: i32,
        arms_dist_max: i32,
        arms_delta_angle: i32,
        arms_prob: f32,
        probability: f32,
    ) -> Self {
        OreVeinConfig {
            tiles,
            center_radius,
            center_prob,
            arms_amount_min,
            arms_amount_max,
            arms_dist_min,
            arms_dist_max,
            arms_delta_angle,
            arms_prob,
            probability,
        }
    }
}
