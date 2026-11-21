//! zombie.iso.worldgen.roads.Road: one road of the generated road network, plus the `org.joml.Vector2i`
//! slice the roads family uses.

use crate::zombie::core::math::pz_math::fastfloor;
use crate::zombie::iso::worldgen::biomes::tile_group::TileGroup;
use crate::zombie::iso::worldgen::utils::chunk_coord::ChunkCoord;

use super::road_direction::RoadDirection;

/// org.joml.Vector2i, the slice the worldgen uses: int x/y, `lengthSquared() = x*x + y*y`, value equals
/// (JOML overrides equals/hashCode). `Hash` is deliberately NOT implemented: JOML's `hashCode` is only
/// observable through the iteration order of a hash-based collection of vectors, and the callers sort that
/// order away before comparing (no such collection exists in the roads family anyway - the points and
/// remotes are plain lists).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Vector2i {
    pub x: i32,
    pub y: i32,
}

impl Vector2i {
    /// org.joml.Vector2i.Vector2i(int, int).
    pub fn new(x: i32, y: i32) -> Self {
        Vector2i { x, y }
    }

    /// org.joml.Vector2i.lengthSquared: `x * x + y * y`.
    pub fn length_squared(&self) -> i32 {
        self.x * self.x + self.y * self.y
    }
}

/// zombie.iso.worldgen.roads.Road: one straight road from `a` to `b`, with the chunks its endpoints sit in
/// and the tile groups it is drawn from.
#[derive(Clone, Debug)]
pub struct Road {
    /// Road.a: the endpoint with the greater `lengthSquared()` (the parameter swap below).
    pub a: Vector2i,
    /// Road.b.
    pub b: Vector2i,
    /// Road.ca: the world chunk of `a`.
    pub ca: ChunkCoord,
    /// Road.cb: the world chunk of `b`.
    pub cb: ChunkCoord,
    /// Road.direction: `NS` when the endpoints share their x, else `WE`.
    pub direction: RoadDirection,
    /// Road.tileGroups (getSingleFeatures).
    pub tile_groups: Vec<TileGroup>,
    /// Road.probability.
    pub probability: f64,
}

impl Road {
    /// zombie.iso.worldgen.roads.Road.&lt;init&gt;(Vector2i, Vector2i, List&lt;TileGroup&gt;, double).
    /// `test = a.lengthSquared() > b.lengthSquared()` is a strict compare, so equal lengths swap the pair
    /// (`test` false gives `this.a = b; this.b = a`). The chunk is `PZMath.fastfloor(x / 8.0F)`: the int
    /// widens to f32 for the division, then fastfloor. `direction = a.x == b.x ? NS : WE` reads the
    /// PARAMETERS, as written - the swap only exchanges the same two values, so the comparison is
    /// equivalent either way.
    pub fn new(a: Vector2i, b: Vector2i, tile_groups: Vec<TileGroup>, probability: f64) -> Self {
        let test = a.length_squared() > b.length_squared();
        let sa = if test { a } else { b };
        let sb = if test { b } else { a };
        let ca = ChunkCoord::new(fastfloor(sa.x as f32 / 8.0f32), fastfloor(sa.y as f32 / 8.0f32));
        let cb = ChunkCoord::new(fastfloor(sb.x as f32 / 8.0f32), fastfloor(sb.y as f32 / 8.0f32));
        let direction = if a.x == b.x {
            RoadDirection::NS
        } else {
            RoadDirection::WE
        };
        Road {
            a: sa,
            b: sb,
            ca,
            cb,
            direction,
            tile_groups,
            probability,
        }
    }

    /// zombie.iso.worldgen.roads.Road.getSingleFeatures.
    pub fn get_single_features(&self) -> &[TileGroup] {
        &self.tile_groups
    }

    /// zombie.iso.worldgen.roads.Road.getProbability.
    pub fn get_probability(&self) -> f64 {
        self.probability
    }

    /// zombie.iso.worldgen.roads.Road.equals: value equals of `a`, `b` (JOML Vector2i equals) and the same
    /// direction. (hashCode is not ported: Road is only hashed in getRoads' HashSet, whose iteration order
    /// the callers sort away.)
    pub fn java_equals(&self, o: &Road) -> bool {
        self.a == o.a && self.b == o.b && self.direction == o.direction
    }
}
