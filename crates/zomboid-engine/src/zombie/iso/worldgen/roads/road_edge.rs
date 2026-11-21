//! zombie.iso.worldgen.roads.RoadEdge: one edge of the road network, split at its subnexus into two roads.

use crate::zombie::iso::worldgen::biomes::tile_group::TileGroup;

use super::road::{Road, Vector2i};

/// zombie.iso.worldgen.roads.RoadEdge: the connection between two nexus points, bent at `subnexus` into a
/// vertical and a horizontal road.
#[derive(Clone, Debug)]
pub struct RoadEdge {
    /// RoadEdge.a: the endpoint with the greater `lengthSquared()` (the parameter swap below).
    pub a: Vector2i,
    /// RoadEdge.b.
    pub b: Vector2i,
    /// RoadEdge.subnexus: `Vector2i(a.x, b.y)`, the corner the edge turns at.
    pub subnexus: Vector2i,
    /// RoadEdge.roads: the two roads of the edge (a to subnexus, subnexus to b).
    pub roads: Vec<Road>,
}

impl RoadEdge {
    /// zombie.iso.worldgen.roads.RoadEdge.&lt;init&gt;(Vector2i, Vector2i, List&lt;TileGroup&gt;, double).
    /// The same strict `lengthSquared() &gt;` swap as `Road`: equal lengths exchange the pair.
    /// `subnexus = Vector2i(this.a.x, this.b.y)`. The two roads share one tile list in Java - the SAME
    /// `List&lt;TileGroup&gt;` object goes into both `Road`s (and every other road of the generator: it is
    /// `config.tiles()`); nothing ever mutates through it (`Road` only exposes `getSingleFeatures()`), so
    /// the port hands each road its own clone without observable difference.
    pub fn new(a: Vector2i, b: Vector2i, tiles: &[TileGroup], probability: f64) -> Self {
        let test = a.length_squared() > b.length_squared();
        let sa = if test { a } else { b };
        let sb = if test { b } else { a };
        let subnexus = Vector2i::new(sa.x, sb.y);
        let roads = vec![
            Road::new(sa, subnexus, tiles.to_vec(), probability),
            Road::new(subnexus, sb, tiles.to_vec(), probability),
        ];
        RoadEdge {
            a: sa,
            b: sb,
            subnexus,
            roads,
        }
    }

    /// zombie.iso.worldgen.roads.RoadEdge.equals: value equals of `a` and `b` (JOML Vector2i equals).
    /// (hashCode is not ported: the edge sets are deduped with this predicate and only their content is
    /// read back, never their hash order.)
    pub fn java_equals(&self, o: &RoadEdge) -> bool {
        self.a == o.a && self.b == o.b
    }
}
