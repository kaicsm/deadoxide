//! zombie.iso.worldgen.veins.OreVein: one ore vein — a center square inside a cell plus `armsAmount` arms
//! radiating out of it, all placed from one `OreVeinConfig`'s ranges and a `java.util.Random`.

use crate::java;
use crate::java_fdlibm;
use crate::zombie::iso::vector2::Vector2;
use crate::zombie::iso::worldgen::biomes::tile_group::TileGroup;

use super::ore_vein_config::OreVeinConfig;

/// zombie.iso.worldgen.veins.OreVein: the original's private final fields in declaration order.
/// `toString` (OreVein.java:82-96, the debug dump `Veins.get` logs) is not ported.
#[derive(Clone, Debug)]
pub struct OreVein {
    pub center_cell_x: i32,
    pub center_cell_y: i32,
    pub depth: i32,
    pub center_square_x: i32,
    pub center_square_y: i32,
    pub arms_amount: i32,
    pub arms_orientation: Vec<f32>,
    pub arms_distance: Vec<f32>,
    pub start_point: Vector2,
    pub end_points: Vec<Vector2>,
    pub tile_groups: Vec<TileGroup>,
    pub arms_prob: f32,
    pub center_prob: f32,
    pub center_radius: i32,
}

impl OreVein {
    /// zombie.iso.worldgen.veins.OreVein.&lt;init&gt;(int cellX, int cellY, OreVeinConfig, Random): the
    /// constructor's draws in order — centerSquareX = rnd.nextInt(256) (OreVein.java:29), centerSquareY
    /// (OreVein.java:30), armsAmount = rnd.nextInt(armsAmountMax - armsAmountMin + 1) + armsAmountMin
    /// (OreVein.java:32; a non-positive bound throws IllegalArgumentException in Java, the port's
    /// `next_int_bound` panics on it), offsetAngle = rnd.nextFloat() * 360.0F (OreVein.java:36), then per
    /// arm the rao roll and the distance roll (OreVein.java:40, 42) — endPoints computed between them, with
    /// no draw of their own. `deltaAngle = 360.0F / armsAmount` divides in float with the int count widened
    /// (infinite when armsAmount is 0 — and then the loop never runs, as in Java). The three arrays are
    /// built by pushing inside the loop instead of `new float[armsAmount]` up front: a negative armsAmount
    /// (possible only with a negative armsAmountMin) throws NegativeArraySizeException in Java where the
    /// port yields empty arrays.
    pub fn new(cell_x: i32, cell_y: i32, config: &OreVeinConfig, rnd: &mut java::Random) -> Self {
        let center_square_x = rnd.next_int_bound(256);
        let center_square_y = rnd.next_int_bound(256);
        // new Vector2(int, int) (OreVein.java:31): the wrapping int sums widen to float.
        let start_point = Vector2::new(
            (cell_x.wrapping_mul(256).wrapping_add(center_square_x)) as f32,
            (cell_y.wrapping_mul(256).wrapping_add(center_square_y)) as f32,
        );
        let arms_amount = rnd
            .next_int_bound(
                config
                    .arms_amount_max
                    .wrapping_sub(config.arms_amount_min)
                    .wrapping_add(1),
            )
            .wrapping_add(config.arms_amount_min);
        let mut arms_orientation: Vec<f32> = Vec::with_capacity(arms_amount.max(0) as usize);
        let mut arms_distance: Vec<f32> = Vec::with_capacity(arms_amount.max(0) as usize);
        let mut end_points: Vec<Vector2> = Vec::with_capacity(arms_amount.max(0) as usize);
        let offset_angle = rnd.next_float() * 360.0;
        let delta_angle = 360.0 / arms_amount as f32;

        for i in 0..arms_amount {
            // (rnd.nextFloat() * 2.0F - 1.0F) * armsDeltaAngle (OreVein.java:40): all float, the int
            // config field widened.
            let rao = (rnd.next_float() * 2.0 - 1.0) * config.arms_delta_angle as f32;
            // deltaAngle * i + offsetAngle + rao (OreVein.java:41): left to right, the loop index widened
            // to float.
            let orientation = delta_angle * i as f32 + offset_angle + rao;
            // rnd.nextFloat() * armsDistMax + armsDistMin (OreVein.java:42): float, both int config
            // fields widened.
            let distance = rnd.next_float() * config.arms_dist_max as f32 + config.arms_dist_min as f32;
            // OreVein.java:43-46: Math.toRadians widens the float angle to double; Math.sin/Math.cos (the
            // fdlibm ones) and the product with the float distance are double; the (float) cast hits the
            // product, and adding startPoint.x / startPoint.y is float again. The unary minus binds to
            // Math.cos before the multiply.
            let radians = to_radians(f64::from(orientation));
            let end_point = Vector2::new(
                start_point.x + (java_fdlibm::sin(radians) * f64::from(distance)) as f32,
                start_point.y + ((-java_fdlibm::cos(radians)) * f64::from(distance)) as f32,
            );
            arms_orientation.push(orientation);
            arms_distance.push(distance);
            end_points.push(end_point);
        }

        OreVein {
            center_cell_x: cell_x,
            center_cell_y: cell_y,
            depth: 0,
            center_square_x,
            center_square_y,
            arms_amount,
            arms_orientation,
            arms_distance,
            start_point,
            end_points,
            // this.tileGroups = config.getTiles() (OreVein.java:49) shares the config's List in Java;
            // nothing ever mutates it (here or in the original vein code), so the port clones the Vec.
            tile_groups: config.tiles.clone(),
            arms_prob: config.arms_prob,
            center_prob: config.center_prob,
            center_radius: config.center_radius,
        }
    }

    /// zombie.iso.worldgen.veins.OreVein.isValid(int x, int y, Random rnd) (OreVein.java:55-75).
    pub fn is_valid(&self, x: i32, y: i32, rnd: &mut java::Random) -> bool {
        // new Vector2(startPoint.x - x, startPoint.y - y) (OreVein.java:56): the int coordinates widen to
        // float.
        let d2 = Vector2::new(
            self.start_point.x - x as f32,
            self.start_point.y - y as f32,
        );
        let length_squared = d2.get_length_squared();
        // OreVein.java:58 — `lengthSquared < this.centerRadius * this.centerRadius && rnd.nextFloat() <
        // this.centerProb`: the radii multiply as ints (wrapping) and the product widens to float for the
        // comparison; the && short-circuits, so the draw happens only when the radius check passes.
        if length_squared < (self.center_radius.wrapping_mul(self.center_radius)) as f32
            && rnd.next_float() < self.center_prob
        {
            return true;
        }

        for i in 0..self.arms_amount {
            // OreVein.java:62 — `!(lengthSquared > this.armsDistance[i] * this.armsDistance[i])`: the
            // distances multiply as floats; the negation means the arm is tested when lengthSquared is
            // less than or equal to the squared distance — and also when either operand is NaN.
            if !(length_squared
                > self.arms_distance[i as usize] * self.arms_distance[i as usize])
            {
                let mut d1p = Vector2::new(
                    self.end_points[i as usize].x - self.start_point.x,
                    self.end_points[i as usize].y - self.start_point.y,
                );
                let mut d3p = Vector2::new(
                    self.end_points[i as usize].x - x as f32,
                    self.end_points[i as usize].y - y as f32,
                );
                d1p.normalize();
                d3p.normalize();
                // OreVein.java:67 — Math.abs(float) is the sign-bit clear, which is Rust's f32::abs; the
                // chain short-circuits, so the draw happens only when both closeness checks pass.
                if (d1p.x - d3p.x).abs() < 0.001
                    && (d1p.y - d3p.y).abs() < 0.001
                    && rnd.next_float() < self.arms_prob
                {
                    return true;
                }
            }
        }

        false
    }

    /// zombie.iso.worldgen.veins.OreVein.getSingleFeatures.
    pub fn get_single_features(&self) -> &[TileGroup] {
        &self.tile_groups
    }
}

/// java.lang.Math.toRadians (JDK 17+): one multiplication by the double constant 0.017453292519943295
/// (PI/180), the same helper thunder_storm.rs ports. The float angle widens to double at the call.
fn to_radians(angdeg: f64) -> f64 {
    angdeg * 0.017453292519943295
}
