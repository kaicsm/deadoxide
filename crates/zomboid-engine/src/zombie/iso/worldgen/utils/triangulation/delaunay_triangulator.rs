//! zombie.iso.worldgen.utils.triangulation.DelaunayTriangulator: the incremental Delaunay triangulation the
//! road generator runs over the road nexus points.

use std::rc::Rc;

use super::edge2d::Edge2D;
use super::triangle2d::Triangle2D;
use super::triangle_soup::TriangleSoup;
use super::vector2d::Vector2D;

/// zombie.iso.worldgen.utils.triangulation.DelaunayTriangulator: Bowyer-Watson insertion over an enclosing
/// super triangle. The original's `List<Vector2D> pointSet` holds object references shared with the soup's
/// triangles; the port's `Rc<Vector2D>` clones are the same identities.
pub struct DelaunayTriangulator {
    point_set: Vec<Rc<Vector2D>>,
    soup: TriangleSoup,
}

/// java.lang.Math.max(double, double): NaN if `a` is NaN (and `a >= b` is false for a NaN `b`, so a NaN
/// anywhere wins), and +0.0 above -0.0 (`doubleToRawLongBits(a) == -0.0` picks `b`).
fn math_max(a: f64, b: f64) -> f64 {
    if a.is_nan() {
        return a;
    }
    if a == 0.0 && b == 0.0 && a.to_bits() == (-0.0f64).to_bits() {
        return b;
    }
    if a >= b {
        a
    } else {
        b
    }
}

/// The NullPointerException `first.getNoneEdgeVertex(edge)` / `second.getNoneEdgeVertex(edge)` throws when
/// the soup holds no triangle sharing the nearest edge (or no second one). The game's caller (RoadGenerator)
/// always adds the points (-1, 9895) and (12598, 901), which alone keep the max coordinate positive — and
/// with a positive max the super triangle strictly encloses every point of the set, so the nearest edge is
/// always interior and shared by two triangles. The helpful-message format the game's JRE prints.
fn triangle2d_null_npe(local: &str) -> String {
    format!(
        "java.lang.NullPointerException: Cannot invoke \"zombie.iso.worldgen.utils.triangulation.Triangle2D.getNoneEdgeVertex(zombie.iso.worldgen.utils.triangulation.Edge2D)\" because \"{local}\" is null"
    )
}

/// The NullPointerException `new Edge2D(a, b)` throws on a null none-edge vertex — `b.magSqrt()` where the
/// vertex is the second parameter (triangulate's edge split), `a.magSqrt()` where it is the first
/// (legalizeEdge). Only reachable when a triangle's three vertices are all endpoints of the edge, which needs
/// duplicate points in the set. The original assigns the null into the triangles first; the port fails at the
/// same value without inserting them — an Err either way.
fn edge2d_ctor_npe(param: &str) -> String {
    format!(
        "java.lang.NullPointerException: Cannot invoke \"zombie.iso.worldgen.utils.triangulation.Vector2D.magSqrt()\" because \"{param}\" is null"
    )
}

impl DelaunayTriangulator {
    /// zombie.iso.worldgen.utils.triangulation.DelaunayTriangulator.DelaunayTriangulator(List<Vector2D>)
    pub fn new(point_set: Vec<Rc<Vector2D>>) -> Self {
        DelaunayTriangulator {
            point_set,
            soup: TriangleSoup::new(),
        }
    }

    /// zombie.iso.worldgen.utils.triangulation.DelaunayTriangulator.triangulate: a fresh soup; fewer than
    /// three points is the NotEnoughPointsException (the Err string is the exception's toString, the class
    /// itself is not ported). The super triangle is `48 * max(max(x, y))` around the point set: corners
    /// `(0, 3m)`, `(3m, 0)`, `(-3m, -3m)` with `m = 16 * max`. Per point: the containing triangle splits into
    /// three (each with its legalizeEdge), or — none contains it, a point exactly on an edge — the nearest
    /// edge's two triangles split into four (four legalizeEdge calls). The super corners' triangles go last.
    pub fn triangulate(&mut self) -> Result<(), String> {
        self.soup = TriangleSoup::new();
        // `this.pointSet != null && this.pointSet.size() >= 3` — the port's field is never null.
        if self.point_set.len() >= 3 {
            let mut max_of_any_coordinate = 0.0;
            for vector in &self.point_set {
                max_of_any_coordinate =
                    math_max(math_max(vector.x, vector.y), max_of_any_coordinate);
            }
            max_of_any_coordinate *= 16.0;
            let p1 = Rc::new(Vector2D::new(0.0, 3.0 * max_of_any_coordinate));
            let p2 = Rc::new(Vector2D::new(3.0 * max_of_any_coordinate, 0.0));
            let p3 = Rc::new(Vector2D::new(
                -3.0 * max_of_any_coordinate,
                -3.0 * max_of_any_coordinate,
            ));
            let super_triangle = Rc::new(Triangle2D::new(p1, p2, p3));
            self.soup.add(super_triangle.clone());

            for i in 0..self.point_set.len() {
                let point = self.point_set[i].clone();
                let triangle = self.soup.find_containing_triangle(&point);
                match triangle {
                    Some(triangle) => {
                        let a = triangle.a.clone();
                        let b = triangle.b.clone();
                        let c = triangle.c.clone();
                        self.soup.remove(&triangle);
                        let first = Rc::new(Triangle2D::new(a.clone(), b.clone(), point.clone()));
                        let second = Rc::new(Triangle2D::new(b.clone(), c.clone(), point.clone()));
                        let third = Rc::new(Triangle2D::new(c.clone(), a.clone(), point.clone()));
                        self.soup.add(first.clone());
                        self.soup.add(second.clone());
                        self.soup.add(third.clone());
                        self.legalize_edge(&first, Edge2D::new(a.clone(), b.clone()), &point)?;
                        self.legalize_edge(&second, Edge2D::new(b.clone(), c.clone()), &point)?;
                        self.legalize_edge(&third, Edge2D::new(c.clone(), a.clone()), &point)?;
                    }
                    None => {
                        let edge = self.soup.find_nearest_edge(&point);
                        let first = self
                            .soup
                            .find_one_triangle_sharing(&edge)
                            .ok_or_else(|| triangle2d_null_npe("first"))?;
                        let second = self
                            .soup
                            .find_neighbour(&first, &edge)
                            .ok_or_else(|| triangle2d_null_npe("second"))?;
                        let first_none_edge_vertex = first
                            .get_none_edge_vertex(&edge)
                            .ok_or_else(|| edge2d_ctor_npe("b"))?;
                        let second_none_edge_vertex = second
                            .get_none_edge_vertex(&edge)
                            .ok_or_else(|| edge2d_ctor_npe("b"))?;
                        self.soup.remove(&first);
                        self.soup.remove(&second);
                        let triangle1 = Rc::new(Triangle2D::new(
                            edge.a.clone(),
                            first_none_edge_vertex.clone(),
                            point.clone(),
                        ));
                        let triangle2 = Rc::new(Triangle2D::new(
                            edge.b.clone(),
                            first_none_edge_vertex.clone(),
                            point.clone(),
                        ));
                        let triangle3 = Rc::new(Triangle2D::new(
                            edge.a.clone(),
                            second_none_edge_vertex.clone(),
                            point.clone(),
                        ));
                        let triangle4 = Rc::new(Triangle2D::new(
                            edge.b.clone(),
                            second_none_edge_vertex.clone(),
                            point.clone(),
                        ));
                        self.soup.add(triangle1.clone());
                        self.soup.add(triangle2.clone());
                        self.soup.add(triangle3.clone());
                        self.soup.add(triangle4.clone());
                        self.legalize_edge(
                            &triangle1,
                            Edge2D::new(edge.a.clone(), first_none_edge_vertex.clone()),
                            &point,
                        )?;
                        self.legalize_edge(
                            &triangle2,
                            Edge2D::new(edge.b.clone(), first_none_edge_vertex.clone()),
                            &point,
                        )?;
                        self.legalize_edge(
                            &triangle3,
                            Edge2D::new(edge.a.clone(), second_none_edge_vertex.clone()),
                            &point,
                        )?;
                        self.legalize_edge(
                            &triangle4,
                            Edge2D::new(edge.b.clone(), second_none_edge_vertex.clone()),
                            &point,
                        )?;
                    }
                }
            }

            self.soup.remove_triangles_using(&super_triangle.a);
            self.soup.remove_triangles_using(&super_triangle.b);
            self.soup.remove_triangles_using(&super_triangle.c);
            Ok(())
        } else {
            Err("zombie.iso.worldgen.utils.triangulation.NotEnoughPointsException: Less than three points in point set.".to_string())
        }
    }

    /// zombie.iso.worldgen.utils.triangulation.DelaunayTriangulator.legalizeEdge: when the neighbour across
    /// `edge` has `newVertex` inside its circumcircle, both triangles flip into two built on the neighbour's
    /// none-edge vertex, then both new edges legalize in turn (the recursion exactly as written).
    fn legalize_edge(
        &mut self,
        triangle: &Rc<Triangle2D>,
        edge: Edge2D,
        new_vertex: &Rc<Vector2D>,
    ) -> Result<(), String> {
        let neighbour_triangle = self.soup.find_neighbour(triangle, &edge);
        if let Some(neighbour_triangle) = neighbour_triangle {
            if neighbour_triangle.is_point_in_circumcircle(new_vertex) {
                self.soup.remove(triangle);
                self.soup.remove(&neighbour_triangle);
                let none_edge_vertex = neighbour_triangle
                    .get_none_edge_vertex(&edge)
                    .ok_or_else(|| edge2d_ctor_npe("a"))?;
                let first_triangle = Rc::new(Triangle2D::new(
                    none_edge_vertex.clone(),
                    edge.a.clone(),
                    new_vertex.clone(),
                ));
                let second_triangle = Rc::new(Triangle2D::new(
                    none_edge_vertex.clone(),
                    edge.b.clone(),
                    new_vertex.clone(),
                ));
                self.soup.add(first_triangle.clone());
                self.soup.add(second_triangle.clone());
                self.legalize_edge(
                    &first_triangle,
                    Edge2D::new(none_edge_vertex.clone(), edge.a.clone()),
                    new_vertex,
                )?;
                self.legalize_edge(
                    &second_triangle,
                    Edge2D::new(none_edge_vertex.clone(), edge.b.clone()),
                    new_vertex,
                )?;
            }
        }
        Ok(())
    }

    /// zombie.iso.worldgen.utils.triangulation.DelaunayTriangulator.getPointSet
    pub fn get_point_set(&self) -> &[Rc<Vector2D>] {
        &self.point_set
    }

    /// zombie.iso.worldgen.utils.triangulation.DelaunayTriangulator.getTriangles: the soup's own list.
    pub fn get_triangles(&self) -> &[Rc<Triangle2D>] {
        self.soup.get_triangles()
    }

    /// zombie.iso.worldgen.utils.triangulation.DelaunayTriangulator.getEdges: the three edges of every
    /// triangle (the third is `(a, c)`, not `(c, a)`) deduped. The original dedups into a
    /// `HashSet<Edge2D>` whose iteration order follows the identity-hash of the Edge2D objects — NOT
    /// reproducible in Rust — so the port dedups by `Edge2D::java_equals` (what the HashSet's equals-based
    /// add does) keeping the FIRST insertion, and the returned order is the triangles' own. Callers must sort
    /// before any byte-for-byte comparison.
    pub fn get_edges(&self) -> Vec<Edge2D> {
        let mut edges: Vec<Edge2D> = Vec::new();
        for triangle in self.soup.get_triangles() {
            for edge in [
                Edge2D::new(triangle.a.clone(), triangle.b.clone()),
                Edge2D::new(triangle.b.clone(), triangle.c.clone()),
                Edge2D::new(triangle.a.clone(), triangle.c.clone()),
            ] {
                if !edges.iter().any(|e| e.java_equals(&edge)) {
                    edges.push(edge);
                }
            }
        }
        edges
    }

    /// zombie.iso.worldgen.utils.triangulation.DelaunayTriangulator.shuffle():
    /// `Collections.shuffle(pointSet)` — the Fisher-Yates loop both of its RandomAccess and toArray paths
    /// run, `for (int i = size; i > 1; i--) swap(list, i-1, rnd.nextInt(i))` (the original's no-argument
    /// overload seeds from a shared static Random; the port takes the Random in).
    pub fn shuffle(&mut self, rnd: &mut crate::java::Random) {
        let size = self.point_set.len();
        let mut i = size;
        while i > 1 {
            let j = rnd.next_int_bound(i as i32) as usize;
            self.point_set.swap(i - 1, j);
            i -= 1;
        }
    }

    /// zombie.iso.worldgen.utils.triangulation.DelaunayTriangulator.shuffle(int[]):
    /// `temp.add(this.pointSet.get(permutation[i]))` for every index, the temp list replacing the point set
    /// (the Rc clones are the same point objects). An out-of-range index is the original's
    /// IndexOutOfBoundsException, a panic here.
    pub fn shuffle_permutation(&mut self, permutation: &[usize]) {
        let temp: Vec<Rc<Vector2D>> = permutation
            .iter()
            .map(|i| self.point_set[*i].clone())
            .collect();
        self.point_set = temp;
    }
}
