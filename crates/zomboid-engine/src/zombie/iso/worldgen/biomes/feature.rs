//! zombie.iso.worldgen.biomes.Feature record.

use super::probability::Probability;
use super::tile_group::TileGroup;

/// zombie.iso.worldgen.biomes.Feature (tileGroups, minSize, maxSize, probability): one candidate of a biome's
/// feature list. An empty tile-group list leaves the original's minSize at Integer.MAX_VALUE and maxSize at 0.
#[derive(Clone, Debug)]
pub struct Feature {
    pub tile_groups: Vec<TileGroup>,
    pub min_size: i32,
    pub max_size: i32,
    pub probability: Probability,
}
