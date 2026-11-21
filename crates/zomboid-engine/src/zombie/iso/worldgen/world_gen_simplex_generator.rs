//! zombie.iso.worldgen.WorldGenSimplexGenerator: the six simplex fields (tree, plant, bush, temperature,
//! hygrometry, ore) plus the biome selector that the procedural world generation samples. Every field is seeded
//! from `new Random(seed + N*100L)` with three `nextDouble` draws (offsetX, offsetY, depth), scale 128.0 (the
//! selector 16.0), offsetNoise 0.0; WGSimplex.noise calls SimplexNoise.noise((x + offsetX) / scale,
//! (y + offsetY) / scale, depth) - the 3D form.

use crate::java;
use crate::zombie::iso::weather::simplex_noise;
use crate::zombie::iso::worldgen::biomes::biome_noise::BiomeNoise;

/// zombie.iso.worldgen.WorldGenSimplexGenerator.WGSimplex record (offsetX, offsetY, depth, scale, offsetNoise).
/// `offsetNoise` is a record component the original's noise never reads.
pub struct WgSimplex {
    pub offset_x: f64,
    pub offset_y: f64,
    pub depth: f64,
    pub scale: f64,
    pub offset_noise: f64,
}

impl WgSimplex {
    /// The record constructor's three draws in argument order: offsetX, offsetY, depth.
    fn from_random(rnd: &mut java::Random, scale: f64) -> Self {
        WgSimplex {
            offset_x: rnd.next_double(),
            offset_y: rnd.next_double(),
            depth: rnd.next_double(),
            scale,
            offset_noise: 0.0,
        }
    }

    /// zombie.iso.worldgen.WorldGenSimplexGenerator.WGSimplex.noise(double, double).
    pub fn noise(&self, x: f64, y: f64) -> f64 {
        simplex_noise::noise3(
            (x + self.offset_x) / self.scale,
            (y + self.offset_y) / self.scale,
            self.depth,
        )
    }
}

/// zombie.iso.worldgen.WorldGenSimplexGenerator.WGSimplexNoise record.
struct WgSimplexNoise {
    tree: WgSimplex,
    plant: WgSimplex,
    bush: WgSimplex,
    temperature: WgSimplex,
    hygrometry: WgSimplex,
    ore: WgSimplex,
}

/// zombie.iso.worldgen.WorldGenSimplexGenerator.
pub struct WorldGenSimplexGenerator {
    noises: WgSimplexNoise,
    selector: WgSimplex,
}

impl WorldGenSimplexGenerator {
    /// zombie.iso.worldgen.WorldGenSimplexGenerator(long): the field seeds are seed+100..seed+600 and the
    /// selector seed+700, each seeding three nextDouble draws.
    pub fn new(seed: i64) -> Self {
        let field = |add: i64| {
            WgSimplex::from_random(&mut java::Random::new(seed.wrapping_add(add)), 128.0)
        };
        WorldGenSimplexGenerator {
            noises: WgSimplexNoise {
                tree: field(100),
                plant: field(200),
                bush: field(300),
                temperature: field(400),
                hygrometry: field(500),
                ore: field(600),
            },
            selector: WgSimplex::from_random(&mut java::Random::new(seed.wrapping_add(700)), 16.0),
        }
    }

    /// zombie.iso.worldgen.WorldGenSimplexGenerator.noise(double, double): the six biome fields at a position.
    pub fn noise(&self, x: f64, y: f64) -> BiomeNoise {
        BiomeNoise {
            landscape: self.noises.tree.noise(x, y),
            plant: self.noises.plant.noise(x, y),
            bush: self.noises.bush.noise(x, y),
            temperature: self.noises.temperature.noise(x, y),
            hygrometry: self.noises.hygrometry.noise(x, y),
            ore: self.noises.ore.noise(x, y),
        }
    }

    /// zombie.iso.worldgen.WorldGenSimplexGenerator.selector(double, double).
    pub fn selector(&self, x: f64, y: f64) -> f64 {
        (self.selector.noise(x, y) + 1.0) / 2.0
    }
}
