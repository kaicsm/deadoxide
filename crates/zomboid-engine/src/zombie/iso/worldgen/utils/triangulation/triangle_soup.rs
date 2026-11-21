//! zombie.iso.worldgen.utils.triangulation.TriangleSoup: the working set of triangles.

use std::rc::Rc;

use super::edge2d::Edge2D;
use super::edge_distance_pack::EdgeDistancePack;
use super::triangle2d::Triangle2D;
use super::vector2d::Vector2D;

/// zombie.iso.worldgen.utils.triangulation.TriangleSoup (package-private in the Java; only the
/// DelaunayTriangulator drives it): the triangles in insertion order.
#[derive(Default)]
pub struct TriangleSoup {
    triangles: Vec<Rc<Triangle2D>>,
}

impl TriangleSoup {
    /// zombie.iso.worldgen.utils.triangulation.TriangleSoup.TriangleSoup()
    pub fn new() -> Self {
        TriangleSoup {
            triangles: Vec::new(),
        }
    }

    /// zombie.iso.worldgen.utils.triangulation.TriangleSoup.add
    pub fn add(&mut self, triangle: Rc<Triangle2D>) {
        self.triangles.push(triangle);
    }

    /// zombie.iso.worldgen.utils.triangulation.TriangleSoup.remove: `List.remove(Object)` — removes the
    /// FIRST element `==` the argument (Triangle2D has no equals override: reference identity), a no-op when
    /// no element matches.
    pub fn remove(&mut self, triangle: &Rc<Triangle2D>) {
        if let Some(i) = self
            .triangles
            .iter()
            .position(|t| Rc::ptr_eq(t, triangle))
        {
            self.triangles.remove(i);
        }
    }

    /// zombie.iso.worldgen.utils.triangulation.TriangleSoup.getTriangles: the original hands out its live
    /// list; the port hands a read-only view of it.
    pub fn get_triangles(&self) -> &[Rc<Triangle2D>] {
        &self.triangles
    }

    /// zombie.iso.worldgen.utils.triangulation.TriangleSoup.findContainingTriangle: the first triangle in
    /// list order that contains the point.
    pub fn find_containing_triangle(&self, point: &Vector2D) -> Option<Rc<Triangle2D>> {
        self.triangles
            .iter()
            .find(|t| t.contains(point))
            .cloned()
    }

    /// zombie.iso.worldgen.utils.triangulation.TriangleSoup.findNeighbour: the first triangle that is a
    /// neighbour of the edge AND is not the argument (`triangleFromSoup != triangle`, identity — the checks
    /// in the original's order).
    pub fn find_neighbour(
        &self,
        triangle: &Rc<Triangle2D>,
        edge: &Edge2D,
    ) -> Option<Rc<Triangle2D>> {
        self.triangles
            .iter()
            .find(|t| t.is_neighbour(edge) && !Rc::ptr_eq(t, triangle))
            .cloned()
    }

    /// zombie.iso.worldgen.utils.triangulation.TriangleSoup.findOneTriangleSharing: the first triangle that
    /// is a neighbour of the edge.
    pub fn find_one_triangle_sharing(&self, edge: &Edge2D) -> Option<Rc<Triangle2D>> {
        self.triangles.iter().find(|t| t.is_neighbour(edge)).cloned()
    }

    /// zombie.iso.worldgen.utils.triangulation.TriangleSoup.findNearestEdge: each triangle's own
    /// findNearestEdge (itself the sorted best of that triangle's three sides), then
    /// `Arrays.sort((Object[])edgeDistancePacks)` — TimSort, stable, Double.compare — and the first pack's
    /// edge. Rust's `sort_by` is stable, the same tie behavior. An empty soup is the original's
    /// `edgeDistancePacks[0]` ArrayIndexOutOfBoundsException, reproduced by the panic.
    pub fn find_nearest_edge(&self, point: &Vector2D) -> Edge2D {
        let mut edge_list: Vec<EdgeDistancePack> = self
            .triangles
            .iter()
            .map(|t| t.find_nearest_edge(point))
            .collect();
        edge_list.sort_by(|a, b| a.compare_to(b));
        edge_list
            .into_iter()
            .next()
            .expect("java.lang.ArrayIndexOutOfBoundsException: Index 0 out of bounds for length 0")
            .edge
    }

    /// zombie.iso.worldgen.utils.triangulation.TriangleSoup.removeTrianglesUsing: the original collects the
    /// triangles with the vertex (identity hasVertex) and removeAll's them — every collected triangle is in
    /// the soup and removal is by identity, so the single order-preserving retain below is the same
    /// two-step.
    pub fn remove_triangles_using(&mut self, vertex: &Rc<Vector2D>) {
        self.triangles.retain(|t| !t.has_vertex(vertex));
    }
}
