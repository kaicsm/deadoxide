//! zombie.iso.worldgen.roads.RoadNexus: one Delaunay point and the road edges leading away from it.

use crate::zombie::iso::worldgen::biomes::tile_group::TileGroup;

use super::road::Vector2i;
use super::road_edge::RoadEdge;

/// zombie.iso.worldgen.roads.RoadNexus: a triangulation point with the remotes it connects to and the road
/// edge built for each of them.
#[derive(Clone, Debug)]
pub struct RoadNexus {
    /// RoadNexus.delaunayPoint.
    pub delaunay_point: Vector2i,
    /// RoadNexus.delaunayRemotes.
    pub delaunay_remotes: Vec<Vector2i>,
    /// RoadNexus.roadEdges: one edge per remote, in the remotes' order.
    pub road_edges: Vec<RoadEdge>,
}

impl RoadNexus {
    /// zombie.iso.worldgen.roads.RoadNexus.&lt;init&gt;(Vector2i, List&lt;Vector2i&gt;, List&lt;TileGroup&gt;,
    /// double): for every remote, in the remotes' order, `new RoadEdge(delaunayPoint, remote, tileGroups,
    /// probability)`.
    pub fn new(
        delaunay_point: Vector2i,
        delaunay_remotes: Vec<Vector2i>,
        tile_groups: &[TileGroup],
        probability: f64,
    ) -> Self {
        let mut road_edges = Vec::new();
        for remote in &delaunay_remotes {
            road_edges.push(RoadEdge::new(delaunay_point, *remote, tile_groups, probability));
        }
        RoadNexus {
            delaunay_point,
            delaunay_remotes,
            road_edges,
        }
    }
}
