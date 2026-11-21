//! zombie.iso.worldgen.utils.triangulation.Vector2D: a point of the Delaunay triangulation.

use std::fmt;

use crate::java::double_to_string;

/// zombie.iso.worldgen.utils.triangulation.Vector2D: two public doubles. NO equals/hashCode override in Java:
/// two Vector2D are never value-equal, and the identity comparisons of Edge2D/Triangle2D (`==` on the
/// endpoints) are `Rc::ptr_eq` in the port — so do NOT derive PartialEq.
#[derive(Clone, Debug)]
pub struct Vector2D {
    pub x: f64,
    pub y: f64,
}

impl Vector2D {
    /// zombie.iso.worldgen.utils.triangulation.Vector2D.Vector2D(double, double)
    pub fn new(x: f64, y: f64) -> Self {
        Vector2D { x, y }
    }

    /// zombie.iso.worldgen.utils.triangulation.Vector2D.sub
    pub fn sub(&self, vector: &Vector2D) -> Vector2D {
        Vector2D::new(self.x - vector.x, self.y - vector.y)
    }

    /// zombie.iso.worldgen.utils.triangulation.Vector2D.add
    pub fn add(&self, vector: &Vector2D) -> Vector2D {
        Vector2D::new(self.x + vector.x, self.y + vector.y)
    }

    /// zombie.iso.worldgen.utils.triangulation.Vector2D.mult
    pub fn mult(&self, scalar: f64) -> Vector2D {
        Vector2D::new(self.x * scalar, self.y * scalar)
    }

    /// zombie.iso.worldgen.utils.triangulation.Vector2D.mag: `Math.sqrt(this.x * this.x + this.y * this.y)`
    /// (sqrt is exact IEEE 754 on both sides).
    pub fn mag(&self) -> f64 {
        (self.x * self.x + self.y * self.y).sqrt()
    }

    /// zombie.iso.worldgen.utils.triangulation.Vector2D.magSqrt: `this.x * this.x + this.y * this.y`
    /// (the squared magnitude, despite the name).
    pub fn mag_sqrt(&self) -> f64 {
        self.x * self.x + self.y * self.y
    }

    /// zombie.iso.worldgen.utils.triangulation.Vector2D.dot
    pub fn dot(&self, vector: &Vector2D) -> f64 {
        self.x * vector.x + self.y * vector.y
    }

    /// zombie.iso.worldgen.utils.triangulation.Vector2D.cross: `this.y * vector.x - this.x * vector.y`
    pub fn cross(&self, vector: &Vector2D) -> f64 {
        self.y * vector.x - self.x * vector.y
    }
}

impl fmt::Display for Vector2D {
    /// zombie.iso.worldgen.utils.triangulation.Vector2D.toString:
    /// `"Vector2D[" + this.x + ", " + this.y + "]"`, the doubles as Java's Double.toString.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Vector2D[{}, {}]",
            double_to_string(self.x),
            double_to_string(self.y)
        )
    }
}
