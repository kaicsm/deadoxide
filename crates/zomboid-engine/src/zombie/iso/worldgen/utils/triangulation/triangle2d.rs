//! zombie.iso.worldgen.utils.triangulation.Triangle2D: one triangle of the Delaunay triangulation.

use std::fmt;
use std::rc::Rc;

use super::edge2d::Edge2D;
use super::edge_distance_pack::EdgeDistancePack;
use super::vector2d::Vector2D;

/// zombie.iso.worldgen.utils.triangulation.Triangle2D: three vertices. No equals/hashCode override in Java —
/// the `==`/`!=` on vertices and soup triangles are reference comparisons, `Rc::ptr_eq` in the port.
#[derive(Clone, Debug)]
pub struct Triangle2D {
    pub a: Rc<Vector2D>,
    pub b: Rc<Vector2D>,
    pub c: Rc<Vector2D>,
}

/// java.lang.Math.signum(double): the argument itself for +0.0, -0.0 (sign kept) and NaN, else 1.0 / -1.0.
fn signum(d: f64) -> f64 {
    if d == 0.0 || d.is_nan() {
        d
    } else if d > 0.0 {
        1.0
    } else {
        -1.0
    }
}

impl Triangle2D {
    /// zombie.iso.worldgen.utils.triangulation.Triangle2D.Triangle2D(Vector2D, Vector2D, Vector2D)
    pub fn new(a: Rc<Vector2D>, b: Rc<Vector2D>, c: Rc<Vector2D>) -> Self {
        Triangle2D { a, b, c }
    }

    /// zombie.iso.worldgen.utils.triangulation.Triangle2D.contains: the three cross products of the point
    /// against the sides, the first two compared and then the first against the third. A point exactly on a
    /// side has a 0.0 cross product, whose signum (+0.0) equals no ±1.0 signum — such a point is contained by
    /// no triangle (only -0.0 and +0.0 compare equal, `==` on doubles in both languages).
    pub fn contains(&self, point: &Vector2D) -> bool {
        let pab = point.sub(&self.a).cross(&self.b.sub(&self.a));
        let pbc = point.sub(&self.b).cross(&self.c.sub(&self.b));
        if !Self::has_same_sign(pab, pbc) {
            return false;
        }
        let pca = point.sub(&self.c).cross(&self.a.sub(&self.c));
        Self::has_same_sign(pab, pca)
    }

    /// zombie.iso.worldgen.utils.triangulation.Triangle2D.isPointInCircumcircle: the 3x3 determinant exactly
    /// as written (the point-subtracted coordinates and their squares), positive when the triangle is
    /// oriented CCW, negative otherwise (NaN det: both comparisons false).
    pub fn is_point_in_circumcircle(&self, point: &Vector2D) -> bool {
        let a11 = self.a.x - point.x;
        let a21 = self.b.x - point.x;
        let a31 = self.c.x - point.x;
        let a12 = self.a.y - point.y;
        let a22 = self.b.y - point.y;
        let a32 = self.c.y - point.y;
        let a13 = (self.a.x - point.x) * (self.a.x - point.x)
            + (self.a.y - point.y) * (self.a.y - point.y);
        let a23 = (self.b.x - point.x) * (self.b.x - point.x)
            + (self.b.y - point.y) * (self.b.y - point.y);
        let a33 = (self.c.x - point.x) * (self.c.x - point.x)
            + (self.c.y - point.y) * (self.c.y - point.y);
        let det = a11 * a22 * a33
            + a12 * a23 * a31
            + a13 * a21 * a32
            - a13 * a22 * a31
            - a12 * a21 * a33
            - a11 * a23 * a32;
        if self.is_oriented_ccw() {
            det > 0.0
        } else {
            det < 0.0
        }
    }

    /// zombie.iso.worldgen.utils.triangulation.Triangle2D.isOrientedCCW: the 2x2 determinant against vertex
    /// c, positive = CCW.
    pub fn is_oriented_ccw(&self) -> bool {
        let a11 = self.a.x - self.c.x;
        let a21 = self.b.x - self.c.x;
        let a12 = self.a.y - self.c.y;
        let a22 = self.b.y - self.c.y;
        let det = a11 * a22 - a12 * a21;
        det > 0.0
    }

    /// zombie.iso.worldgen.utils.triangulation.Triangle2D.isNeighbour:
    /// `(a == edge.a || b == edge.a || c == edge.a) && (a == edge.b || b == edge.b || c == edge.b)` —
    /// reference identity: both endpoints of the edge among this triangle's vertices.
    pub fn is_neighbour(&self, edge: &Edge2D) -> bool {
        (Rc::ptr_eq(&self.a, &edge.a) || Rc::ptr_eq(&self.b, &edge.a) || Rc::ptr_eq(&self.c, &edge.a))
            && (Rc::ptr_eq(&self.a, &edge.b)
                || Rc::ptr_eq(&self.b, &edge.b)
                || Rc::ptr_eq(&self.c, &edge.b))
    }

    /// zombie.iso.worldgen.utils.triangulation.Triangle2D.getNoneEdgeVertex: the first of a, b, c that is
    /// neither endpoint of the edge (identity `!=`); null — None — when all three are endpoints.
    pub fn get_none_edge_vertex(&self, edge: &Edge2D) -> Option<Rc<Vector2D>> {
        if !Rc::ptr_eq(&self.a, &edge.a) && !Rc::ptr_eq(&self.a, &edge.b) {
            Some(self.a.clone())
        } else if !Rc::ptr_eq(&self.b, &edge.a) && !Rc::ptr_eq(&self.b, &edge.b) {
            Some(self.b.clone())
        } else if !Rc::ptr_eq(&self.c, &edge.a) && !Rc::ptr_eq(&self.c, &edge.b) {
            Some(self.c.clone())
        } else {
            None
        }
    }

    /// zombie.iso.worldgen.utils.triangulation.Triangle2D.hasVertex: reference identity against all three.
    pub fn has_vertex(&self, vertex: &Rc<Vector2D>) -> bool {
        Rc::ptr_eq(&self.a, vertex) || Rc::ptr_eq(&self.b, vertex) || Rc::ptr_eq(&self.c, vertex)
    }

    /// zombie.iso.worldgen.utils.triangulation.Triangle2D.findNearestEdge: one EdgeDistancePack per side
    /// (a,b), (b,c), (c,a) — each with its OWN `new Edge2D(...)` for the pack and ANOTHER for
    /// computeClosestPoint, both from the same vertex pair so both get the constructor's same endpoint swap —
    /// then `Arrays.sort((Object[])edges)` (TimSort, stable, the Comparable's Double.compare order) and the
    /// first. Rust's `sort_by` is stable, the same tie behavior.
    pub fn find_nearest_edge(&self, point: &Vector2D) -> EdgeDistancePack {
        let mut edges = [
            EdgeDistancePack::new(
                Edge2D::new(self.a.clone(), self.b.clone()),
                Self::compute_closest_point(&Edge2D::new(self.a.clone(), self.b.clone()), point)
                    .sub(point)
                    .mag(),
            ),
            EdgeDistancePack::new(
                Edge2D::new(self.b.clone(), self.c.clone()),
                Self::compute_closest_point(&Edge2D::new(self.b.clone(), self.c.clone()), point)
                    .sub(point)
                    .mag(),
            ),
            EdgeDistancePack::new(
                Edge2D::new(self.c.clone(), self.a.clone()),
                Self::compute_closest_point(&Edge2D::new(self.c.clone(), self.a.clone()), point)
                    .sub(point)
                    .mag(),
            ),
        ];
        edges.sort_by(|a, b| a.compare_to(b));
        let [first, ..] = edges;
        first
    }

    /// zombie.iso.worldgen.utils.triangulation.Triangle2D.computeClosestPoint(Edge2D, Vector2D): the
    /// projection parameter t clamped to [0, 1] (NaN stays NaN: both comparisons false), then
    /// `edge.a.add(ab.mult(t))`.
    fn compute_closest_point(edge: &Edge2D, point: &Vector2D) -> Vector2D {
        let ab = edge.b.sub(&edge.a);
        let t = point.sub(&edge.a).dot(&ab) / ab.dot(&ab);
        let t = if t < 0.0 {
            0.0
        } else if t > 1.0 {
            1.0
        } else {
            t
        };
        edge.a.add(&ab.mult(t))
    }

    /// zombie.iso.worldgen.utils.triangulation.Triangle2D.hasSameSign:
    /// `Math.signum(a) == Math.signum(b)` — NaN's signum is NaN, which equals nothing, not even NaN's.
    fn has_same_sign(a: f64, b: f64) -> bool {
        signum(a) == signum(b)
    }
}

impl fmt::Display for Triangle2D {
    /// zombie.iso.worldgen.utils.triangulation.Triangle2D.toString:
    /// `"Triangle2D[" + this.a + ", " + this.b + ", " + this.c + "]"`, each vertex its Vector2D toString.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Triangle2D[{}, {}, {}]", self.a, self.b, self.c)
    }
}
