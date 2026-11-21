//! zombie.iso.worldgen.roads.RoadGenerator: the road network of a generated world - Delaunay nexus points,
//! their filtered edges, the nexus/edge structures built from them and the chunk-keyed edge lookup getRoads
//! reads.

use std::collections::HashMap;
use std::rc::Rc;

use crate::zombie::core::math::pz_math::fastfloor;
use crate::zombie::iso::worldgen::utils::triangulation::delaunay_triangulator::DelaunayTriangulator;
use crate::zombie::iso::worldgen::utils::triangulation::edge2d::Edge2D;
use crate::zombie::iso::worldgen::utils::triangulation::vector2d::Vector2D;
use crate::zombie::iso::worldgen::world_gen_params::WorldGenParams;

use super::road::{Road, Vector2i};
use super::road_config::RoadConfig;
use super::road_direction::RoadDirection;
use super::road_edge::RoadEdge;
use super::road_nexus::RoadNexus;

/// zombie.iso.worldgen.roads.RoadGenerator. The original reads `IsoWorld.instance.metaGrid.minX..maxX` /
/// `minY..maxY` for the point loop and again in getRoads; the port has no statics, so the constructor takes
/// the bounds and stores them for getRoads.
pub struct RoadGenerator {
    /// RoadGenerator.seed.
    pub seed: i64,
    /// RoadGenerator.offset.
    pub offset: i64,
    /// RoadGenerator.config.
    pub config: RoadConfig,
    /// RoadGenerator.roadDelaunayPoints. genRoadDelaunayPoints collects them into a
    /// `HashSet&lt;Vector2D&gt;`, but Vector2D overrides neither hashCode nor equals, so the set is
    /// identity-keyed: it never actually dedups (every point is a fresh object) and its iteration order is
    /// the identity-hash bucket order, which is not reproducible. The port keeps INSERTION order (the loop
    /// order, then the two fixed points); callers sort their output before comparing, so the difference is
    /// not observable.
    pub road_delaunay_points: Vec<Rc<Vector2D>>,
    /// RoadGenerator.roadDelaunayEdges: every edge of the triangulation, in `getEdges()` order.
    pub road_delaunay_edges: Vec<Edge2D>,
    /// RoadGenerator.roadDelaunayEdgesFiltered: the edges with `magSqrt() &lt; config.filter`.
    pub road_delaunay_edges_filtered: Vec<Edge2D>,
    /// RoadGenerator.roadNexus.
    pub road_nexus: Vec<RoadNexus>,
    /// RoadGenerator.roadEdges: `Map&lt;ChunkCoord, Set&lt;RoadEdge&gt;&gt;`, keyed here by the (x, y) the
    /// original's ChunkCoord carries. The original only ever calls `get` on it (getRoads), never iterates
    /// it, so Java's HashMap hash order is unobservable; each value is the HashSet's content as a Vec in
    /// insertion order, deduped by RoadEdge's value equals - that Vec order shows up only in the order
    /// getRoads meets the roads, which the callers sort away.
    pub road_edges: HashMap<(i32, i32), Vec<RoadEdge>>,
    /// `IsoWorld.instance.metaGrid.minX` as the constructor read it (getRoads re-reads it there).
    meta_min_x: i32,
    /// `IsoWorld.instance.metaGrid.maxX`.
    meta_max_x: i32,
    /// `IsoWorld.instance.metaGrid.minY`.
    meta_min_y: i32,
    /// `IsoWorld.instance.metaGrid.maxY`.
    meta_max_y: i32,
}

impl RoadGenerator {
    /// zombie.iso.worldgen.roads.RoadGenerator.&lt;init&gt;(long, RoadConfig, long): seed/offset/config,
    /// then genRoadDelaunayPoints, genRoadDelaunayEdges, filterEdges, genRoadNexus and getRoadEdgesMap, in
    /// that order. `meta_min_x..meta_max_x` / `meta_min_y..meta_max_y` stand for the
    /// `IsoWorld.instance.metaGrid` bounds the original reads; `params` is the `WorldGenParams.INSTANCE`
    /// its getRandom(x, y, offset) runs on. The `Err` is genRoadDelaunayEdges'
    /// `RuntimeException("Not enough points in triangulation")` (the caught NotEnoughPointsException).
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        seed: i64,
        config: RoadConfig,
        offset: i64,
        meta_min_x: i32,
        meta_max_x: i32,
        meta_min_y: i32,
        meta_max_y: i32,
        params: &WorldGenParams,
    ) -> Result<Self, String> {
        let road_delaunay_points = Self::gen_road_delaunay_points(
            offset,
            &config,
            meta_min_x,
            meta_max_x,
            meta_min_y,
            meta_max_y,
            params,
        );
        // the same point objects as the field (the Rc handles are cloned, the Vector2Ds shared)
        let road_delaunay_edges = Self::gen_road_delaunay_edges(road_delaunay_points.clone())?;
        let road_delaunay_edges_filtered = Self::filter_edges(&road_delaunay_edges, &config);
        let road_nexus = Self::gen_road_nexus(
            &road_delaunay_points,
            &road_delaunay_edges_filtered,
            &config,
        );
        let road_edges = Self::get_road_edges_map(&road_nexus);
        Ok(RoadGenerator {
            seed,
            offset,
            config,
            road_delaunay_points,
            road_delaunay_edges,
            road_delaunay_edges_filtered,
            road_nexus,
            road_edges,
            meta_min_x,
            meta_max_x,
            meta_min_y,
            meta_max_y,
        })
    }

    /// RoadGenerator.genRoadDelaunayPoints: for every (x, y) of the metaGrid bounds (x outer, y inner,
    /// both inclusive) a FRESH `Random rand = WorldGenParams.INSTANCE.getRandom(x, y, offset)`; on
    /// `rand.nextFloat() &lt; config.probability` (the float widens to double for the compare) the point is
    /// `Vector2D(x * 256 + (int)(256.0F * rand.nextFloat()), y * 256 + (int)(256.0F * rand.nextFloat()))`
    /// - the x draw first, then the y draw, and `(int)` of the f32 truncates toward zero (the product is
    /// in [0, 256), so it is a floor). After the loop the two fixed points (-1.0, 9895.0) and
    /// (12598.0, 901.0) are added. The collecting HashSet keeps insertion order here (see the field doc).
    fn gen_road_delaunay_points(
        offset: i64,
        config: &RoadConfig,
        meta_min_x: i32,
        meta_max_x: i32,
        meta_min_y: i32,
        meta_max_y: i32,
        params: &WorldGenParams,
    ) -> Vec<Rc<Vector2D>> {
        // Set<Vector2D> nexus = new HashSet<>() - identity-keyed, so this list is what it accumulated,
        // in insertion order.
        let mut nexus: Vec<Rc<Vector2D>> = Vec::new();

        for x in meta_min_x..=meta_max_x {
            for y in meta_min_y..=meta_max_y {
                let mut rand = params.get_random_with_offset(x, y, offset);
                if f64::from(rand.next_float()) < config.probability {
                    // x * 256 + (int)(256.0F * rand.nextFloat()), then the y draw
                    let px = x * 256 + (256.0f32 * rand.next_float()) as i32;
                    let py = y * 256 + (256.0f32 * rand.next_float()) as i32;
                    nexus.push(Rc::new(Vector2D {
                        x: f64::from(px),
                        y: f64::from(py),
                    }));
                }
            }
        }

        nexus.push(Rc::new(Vector2D {
            x: -1.0,
            y: 9895.0,
        }));
        nexus.push(Rc::new(Vector2D {
            x: 12598.0,
            y: 901.0,
        }));
        nexus
    }

    /// RoadGenerator.genRoadDelaunayEdges(List&lt;Vector2D&gt;): the Delaunay triangulation of all the
    /// points, in the order they were collected. The caught `NotEnoughPointsException` is rethrown as
    /// `RuntimeException("Not enough points in triangulation")`, which is the port's `Err`.
    fn gen_road_delaunay_edges(road_nexus: Vec<Rc<Vector2D>>) -> Result<Vec<Edge2D>, String> {
        let mut triangulator = DelaunayTriangulator::new(road_nexus);
        triangulator
            .triangulate()
            .map_err(|_| "java.lang.RuntimeException: Not enough points in triangulation".to_string())?;
        Ok(triangulator.get_edges())
    }

    /// RoadGenerator.filterEdges(List&lt;Edge2D&gt;): keep `e.magSqrt() &lt; config.filter`. The kept
    /// edges are copied as struct literals over the same endpoint `Rc`s - `Edge2D::new` could not be used,
    /// it re-orders endpoints of equal `magSqrt()` - so both lists share the endpoint objects exactly as
    /// the original's two lists share the same Edge2D objects (nothing mutates an edge, so the copy is not
    /// observable).
    fn filter_edges(road_delaunay_edges: &[Edge2D], config: &RoadConfig) -> Vec<Edge2D> {
        road_delaunay_edges
            .iter()
            .filter(|e| e.mag_sqrt() < config.filter)
            .map(|e| Edge2D {
                a: e.a.clone(),
                b: e.b.clone(),
            })
            .collect()
    }

    /// RoadGenerator.genRoadNexus(List&lt;Vector2D&gt;, List&lt;Edge2D&gt;): for every point, in list
    /// order, `Vector2i((int)p.x, (int)p.y)` (`(int)` of the f64 truncates toward zero; Java saturates at
    /// i32::MIN/MAX, the same as Rust's `as`), then the other endpoints of the filtered edges touching it,
    /// in edge order: an edge counts when its a matches (pushing b), ELSE IF its b matches (pushing a) -
    /// the else-if matters, an edge matching both ways only pushes its b. The nexus is
    /// `new RoadNexus(point, edges, config.tiles, config.probaRoads)`.
    fn gen_road_nexus(
        road_delaunay_points: &[Rc<Vector2D>],
        road_delaunay_edges_filtered: &[Edge2D],
        config: &RoadConfig,
    ) -> Vec<RoadNexus> {
        let mut nexus = Vec::new();

        for p in road_delaunay_points {
            let point = Vector2i::new(p.x as i32, p.y as i32);
            let mut edges = Vec::new();

            for e in road_delaunay_edges_filtered {
                if e.a.x as i32 == point.x && e.a.y as i32 == point.y {
                    edges.push(Vector2i::new(e.b.x as i32, e.b.y as i32));
                } else if e.b.x as i32 == point.x && e.b.y as i32 == point.y {
                    edges.push(Vector2i::new(e.a.x as i32, e.a.y as i32));
                }
            }

            nexus.push(RoadNexus::new(
                point,
                edges,
                &config.tiles,
                config.proba_roads,
            ));
        }

        nexus
    }

    /// RoadGenerator.getRoadNexus.
    pub fn get_road_nexus(&self) -> &[RoadNexus] {
        &self.road_nexus
    }

    /// RoadGenerator.getRoadEdges(List&lt;RoadNexus&gt;): every nexus' road edges, in order.
    fn get_road_edges(road_nexus: &[RoadNexus]) -> Vec<&RoadEdge> {
        let mut road_edges = Vec::new();
        for nexus in road_nexus {
            road_edges.extend(nexus.road_edges.iter());
        }
        road_edges
    }

    /// RoadGenerator.getRoadEdgesMap(List&lt;RoadNexus&gt;): per edge (in getRoadEdges order) the chunks of
    /// its a, b and subnexus - `ChunkCoord(PZMath.fastfloor(x / 8.0F), ...)`, the int widening to f32 for
    /// the division - each mapping to the set the edge is inserted into, DEDUPED by RoadEdge's value equals
    /// with the first insertion kept (a, b and subnexus may share a chunk; the original then adds the same
    /// edge to the same set up to three times). The port stores a clone per key where the original shares
    /// one object between the sets - nothing mutates an edge, so the difference is not observable.
    fn get_road_edges_map(road_nexus: &[RoadNexus]) -> HashMap<(i32, i32), Vec<RoadEdge>> {
        let mut road_edges_map: HashMap<(i32, i32), Vec<RoadEdge>> = HashMap::new();

        for road_edge in Self::get_road_edges(road_nexus) {
            let coord_a = (
                fastfloor(road_edge.a.x as f32 / 8.0f32),
                fastfloor(road_edge.a.y as f32 / 8.0f32),
            );
            let coord_b = (
                fastfloor(road_edge.b.x as f32 / 8.0f32),
                fastfloor(road_edge.b.y as f32 / 8.0f32),
            );
            let coord_sub = (
                fastfloor(road_edge.subnexus.x as f32 / 8.0f32),
                fastfloor(road_edge.subnexus.y as f32 / 8.0f32),
            );
            for coord in [coord_a, coord_b, coord_sub] {
                let set = road_edges_map.entry(coord).or_default();
                if !set.iter().any(|e| e.java_equals(road_edge)) {
                    set.push(road_edge.clone());
                }
            }
        }

        road_edges_map
    }

    /// RoadGenerator.getRoads(int cx, int cy): the roads covering cell (cx, cy). First x runs from
    /// `metaGrid.minX * 32` to `metaGrid.maxX * 32` inclusive: the edges at ChunkCoord(x, cy), each edge's
    /// WE roads whose chunk span `min(ca.x, cb.x) &lt;= cx &lt;= max(ca.x, cb.x)` covers cx. Then y runs
    /// likewise at ChunkCoord(cx, y) for the NS roads compared against cy. The result is Java's
    /// `HashSet&lt;Road&gt;` - deduped by Road's value equals (the same road is met at several chunks),
    /// here keeping INSERTION order instead of the hash order; the callers sort before comparing. The
    /// borrows come from `road_edges`' stored edges.
    pub fn get_roads(&self, cx: i32, cy: i32) -> Vec<&Road> {
        let mut roads: Vec<&Road> = Vec::new();
        // roads.add(road): HashSet.add deduped by Road's value equals, the first insertion kept.
        fn add<'a>(roads: &mut Vec<&'a Road>, road: &'a Road) {
            if !roads.iter().any(|r| r.java_equals(road)) {
                roads.push(road);
            }
        }

        for x in (self.meta_min_x * 32)..=(self.meta_max_x * 32) {
            if let Some(edges) = self.road_edges.get(&(x, cy)) {
                for re in edges {
                    for road in &re.roads {
                        if road.direction == RoadDirection::WE
                            && road.ca.x.min(road.cb.x) <= cx
                            && road.ca.x.max(road.cb.x) >= cx
                        {
                            add(&mut roads, road);
                        }
                    }
                }
            }
        }

        for y in (self.meta_min_y * 32)..=(self.meta_max_y * 32) {
            if let Some(edges) = self.road_edges.get(&(cx, y)) {
                for re in edges {
                    for road in &re.roads {
                        if road.direction == RoadDirection::NS
                            && road.ca.y.min(road.cb.y) <= cy
                            && road.ca.y.max(road.cb.y) >= cy
                        {
                            add(&mut roads, road);
                        }
                    }
                }
            }
        }

        roads
    }
}
