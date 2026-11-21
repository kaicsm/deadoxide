//! zombie.iso.worldgen.utils.triangulation.Edge2D: an edge of the triangulation.

use std::rc::Rc;

use super::vector2d::Vector2D;

/// zombie.iso.worldgen.utils.triangulation.Edge2D: two endpoints, the one farther from the origin first.
///
/// Java's hashCode is `Objects.hash(this.a, this.b)`, and Vector2D has no hashCode override — the identity
/// hash of the endpoint objects, which is NOT reproducible in Rust; no hash is implemented here. The one place
/// that relies on Edge2D hashing (`DelaunayTriangulator.getEdges`' HashSet) instead dedups by `java_equals`
/// in insertion order — see there.
#[derive(Clone, Debug)]
pub struct Edge2D {
    pub a: Rc<Vector2D>,
    pub b: Rc<Vector2D>,
}

impl Edge2D {
    /// zombie.iso.worldgen.utils.triangulation.Edge2D.Edge2D(Vector2D, Vector2D):
    /// `boolean test = a.magSqrt() > b.magSqrt(); this.a = test ? a : b; this.b = test ? b : a;`
    /// — the endpoint farther from the origin first; on EQUAL magSqrt the parameter pair is REVERSED
    /// (`this.a = b, this.b = a`). A null endpoint would be the constructor's NullPointerException at
    /// `a.magSqrt()` or `b.magSqrt()`; the port's `Rc` cannot be null.
    pub fn new(a: Rc<Vector2D>, b: Rc<Vector2D>) -> Self {
        let test = a.mag_sqrt() > b.mag_sqrt();
        if test {
            Edge2D { a, b }
        } else {
            Edge2D { a: b, b: a }
        }
    }

    /// zombie.iso.worldgen.utils.triangulation.Edge2D.mag: `Math.sqrt(this.magSqrt())`
    pub fn mag(&self) -> f64 {
        self.mag_sqrt().sqrt()
    }

    /// zombie.iso.worldgen.utils.triangulation.Edge2D.magSqrt:
    /// `(a.x - b.x) * (a.x - b.x) + (a.y - b.y) * (a.y - b.y)`
    pub fn mag_sqrt(&self) -> f64 {
        (self.a.x - self.b.x) * (self.a.x - self.b.x)
            + (self.a.y - self.b.y) * (self.a.y - self.b.y)
    }

    /// zombie.iso.worldgen.utils.triangulation.Edge2D.equals:
    /// `Objects.equals(this.a, o.a) && Objects.equals(this.b, o.b)` — Vector2D has no equals override, so
    /// `Objects.equals` is reference equality: BOTH endpoints must be the same instances (a null endpoint
    /// never occurs: the constructor dereferences both). The `this == o` shortcut of the original is the same
    /// test when both endpoints match.
    pub fn java_equals(&self, o: &Edge2D) -> bool {
        Rc::ptr_eq(&self.a, &o.a) && Rc::ptr_eq(&self.b, &o.b)
    }
}
