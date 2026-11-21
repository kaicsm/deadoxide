//! zombie.iso.worldgen.zombie.ZombieVoronoi: the cached 32x32 zombie-density noise of one
//! `zombie_voronoi` entry - per square, the sorted-distance pick against the Voronoi points of the
//! 9 sections around it.

use std::collections::HashMap;

use crate::java::Random;
use crate::se::krka::kahlua::vm::object::{R, TableRef, Throwable};

use super::closest_selection::ClosestSelectionType;
use super::coord::Coord;
use super::hash_util::hash2d;
use super::zombie_voronoi_entry::ZombieVoronoiEntry;
use super::zombie_voronoi_reader::get_entries;

/// zombie.iso.worldgen.zombie.ZombieVoronoi. The original's `synchronized (this.cellCache)` blocks
/// are the port's `&mut self` (exclusive access; the computation between the two blocks runs
/// unlocked there, which no other thread can observe through this port).
pub struct ZombieVoronoi {
    pub seed: i64,
    pub number_points: i32,
    pub closest_point: ClosestSelectionType,
    pub scale: f64,
    pub cutoff: f64,
    /// `public final Map<CellCoord, double[]> cellCache`: only `containsKey`/`get`, `put` and
    /// `remove` are observed (the iteration order never is), so a `HashMap` keyed by the cell's
    /// `(x, y)` - the CellCoord record's fields - is faithful.
    pub cell_cache: HashMap<(i32, i32), Vec<f64>>,
}

impl ZombieVoronoi {
    /// ZombieVoronoi(long, int, ClosestSelection, double, double).
    pub fn new(
        seed: i64,
        number_points: i32,
        closest_point: ClosestSelectionType,
        scale: f64,
        cutoff: f64,
    ) -> Self {
        ZombieVoronoi {
            seed,
            number_points,
            closest_point,
            scale,
            cutoff,
            cell_cache: HashMap::new(),
        }
    }

    /// ZombieVoronoi(long, ZombieVoronoiEntry) - the private constructor getVoronois uses:
    /// `this(seed, entry.numberPoints(), entry.closestPoint(), entry.scale(), entry.cutoff())`.
    pub fn from_entry(seed: i64, entry: &ZombieVoronoiEntry) -> Self {
        Self::new(
            seed,
            entry.number_points,
            entry.closest_point,
            entry.scale,
            entry.cutoff,
        )
    }

    /// ZombieVoronoi.getVoronois(long): one ZombieVoronoi per entry of the `zombie_voronoi` table -
    /// the caller performs the original's `LuaManager.env.rawget("zombie_voronoi")` (and its
    /// `(KahluaTable)` cast, and the NPE a null table would raise) and passes the table in - with
    /// the entry's index (as a long) added to the seed, wrapping.
    pub fn get_voronois(seed: i64, zombie_voronoi_table: &TableRef) -> R<Vec<ZombieVoronoi>> {
        let entries = get_entries(zombie_voronoi_table)?;
        let mut voronois = Vec::new();
        for (i, entry) in entries.iter().enumerate() {
            voronois.push(ZombieVoronoi::from_entry(seed.wrapping_add(i as i64), entry));
        }
        Ok(voronois)
    }

    /// ZombieVoronoi.evaluateCellNoise(int, int): the cell's 32x32 chunk noise, cached. Per square
    /// `(x + wx * 32) / this.scale` - the int arithmetic (wrapping) widened to double for the
    /// division - at the index `x + y * 32` of the chunk. A cached cell returns the cached array
    /// (the original returns the same `double[]` it stored; the port's `&[f64]` borrows it). The
    /// one exception evaluateNoise can raise - getClosest's IndexOutOfBoundsException, on a
    /// numberPoints below 1 - propagates, leaving the cache untouched.
    pub fn evaluate_cell_noise(&mut self, wx: i32, wy: i32) -> R<&[f64]> {
        // The hit's borrow must not span the miss path's insert, so the guard runs before the lookup.
        if self.cell_cache.contains_key(&(wx, wy)) {
            return Ok(self.cell_cache[&(wx, wy)].as_slice());
        }
        let mut chunk_noise = vec![0.0; 1024];
        // one points list for the whole cell - evaluateNoise clears it at the start of every call
        let mut points: Vec<Coord> = Vec::new();
        // new Random(): the seed the constructor picks is unobservable, evaluateNoise re-seeds the
        // rng before its first draw
        let mut rng = Random::new(0);
        for x in 0i32..32 {
            for y in 0i32..32 {
                chunk_noise[(x + y * 32) as usize] = self
                    .evaluate_noise(
                        x.wrapping_add(wx.wrapping_mul(32)) as f64 / self.scale,
                        y.wrapping_add(wy.wrapping_mul(32)) as f64 / self.scale,
                        &mut rng,
                        &mut points,
                    )
                    .map_err(index_out_of_bounds)?;
            }
        }
        // cellCache.put(..) and the return of the same array (the port's borrow of the stored Vec)
        self.cell_cache.insert((wx, wy), chunk_noise);
        Ok(self.cell_cache[&(wx, wy)].as_slice())
    }

    /// ZombieVoronoi.evaluateCellCutoff(int, int): `SandboxOptions.instance.zombieVoronoiNoise
    /// .getValue() ? this.cutoff : 0.0` - the sandbox option is a parameter (the port has no
    /// statics) - then the (wx, wy, cutoff) overload.
    pub fn evaluate_cell_cutoff(
        &mut self,
        wx: i32,
        wy: i32,
        zombie_voronoi_noise_on: bool,
    ) -> R<Vec<f64>> {
        let cutoff = if zombie_voronoi_noise_on {
            self.cutoff
        } else {
            0.0
        };
        self.evaluate_cell_cutoff_with(wx, wy, cutoff)
    }

    /// ZombieVoronoi.evaluateCellCutoff(int, int, double): the cell noise through
    /// `noise.map(x -> x < cutoff ? 0.0 : 1.0)` (a NaN noise compares false to `<`, like the
    /// original's).
    pub fn evaluate_cell_cutoff_with(&mut self, wx: i32, wy: i32, cutoff: f64) -> R<Vec<f64>> {
        let noise = self.evaluate_cell_noise(wx, wy)?;
        Ok(noise
            .iter()
            .map(|&x| if x < cutoff { 0.0 } else { 1.0 })
            .collect())
    }

    /// ZombieVoronoi.releaseCell(int, int): drops the cell's cached noise (the original discards the
    /// removed array).
    pub fn release_cell(&mut self, cell_x: i32, cell_y: i32) {
        self.cell_cache.remove(&(cell_x, cell_y));
    }

    /// ZombieVoronoi.evaluateNoise(double, double, Random, List&lt;Coord&gt;) - private in the
    /// original: the floor of (x, y) as ints, then the 9 sections around it (xOffset the outer
    /// loop, wrapping int adds); per section `HashUtil.hash2D(this.seed, secX, secY)`'s int result
    /// re-seeds the Random - the `setSeed(long)` call sign-extends the int - then numberPoints
    /// draws of `nextDouble()`, the x draw first, each a Coord offset by the same section.
    /// getClosest runs on the original (x, y) and the accumulated points; its `Err` propagates.
    fn evaluate_noise(
        &self,
        x: f64,
        y: f64,
        rng: &mut Random,
        points: &mut Vec<Coord>,
    ) -> Result<f64, String> {
        points.clear();
        let i_x = x.floor() as i32;
        let i_y = y.floor() as i32;
        for x_offset in -1..=1 {
            let sec_x = i_x.wrapping_add(x_offset);
            for y_offset in -1..=1 {
                let sec_y = i_y.wrapping_add(y_offset);
                let hash = hash2d(self.seed, sec_x as i64, sec_y as i64);
                // rng.setSeed(hash): the int widened to the long parameter, sign-extended
                rng.set_seed(hash as i64);
                for _ in 0..self.number_points {
                    points.push(Coord::new(
                        rng.next_double() + sec_x as f64,
                        rng.next_double() + sec_y as f64,
                    ));
                }
            }
        }
        self.closest_point.get_closest(x, y, points)
    }
}

/// The `java.lang.IndexOutOfBoundsException` `ClosestSelectionType.getClosest` raised on a
/// too-short points list (its text is what `evaluateNoise`'s `Err` carries) as the port's
/// `Throwable`: the class and the message the text holds.
fn index_out_of_bounds(message: String) -> Throwable {
    let message = match message.strip_prefix("java.lang.IndexOutOfBoundsException: ") {
        Some(m) => m.to_string(),
        None => message,
    };
    Throwable::Java {
        class: "java.lang.IndexOutOfBoundsException",
        message: Some(message),
    }
}
