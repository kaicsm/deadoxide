//! zombie.iso.worldgen.roads.RoadConfig record.

use crate::zombie::iso::worldgen::biomes::tile_group::TileGroup;

/// zombie.iso.worldgen.roads.RoadConfig (tiles, probaRoads, probability, filter): the roads of one
/// `worldgen.roads` entry - the tile groups a road is drawn from, `probaRoads` the chance a nexus point
/// becomes a road junction, `filter` the edge length cutoff, `probability` the chance a cell grows roads
/// at all.
#[derive(Clone, Debug)]
pub struct RoadConfig {
    pub tiles: Vec<TileGroup>,
    pub proba_roads: f64,
    pub probability: f64,
    pub filter: f64,
}

impl RoadConfig {
    /// RoadConfig's canonical constructor.
    pub fn new(tiles: Vec<TileGroup>, proba_roads: f64, probability: f64, filter: f64) -> Self {
        RoadConfig {
            tiles,
            proba_roads,
            probability,
            filter,
        }
    }
}
