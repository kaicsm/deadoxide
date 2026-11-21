//! zombie.iso.worldgen.biomes.BiomeNoise record.

/// zombie.iso.worldgen.biomes.BiomeNoise (landscape, plant, bush, temperature, hygrometry, ore): the six simplex
/// fields the procedural generation samples at a position. `landscape` is the noise of the generator's `tree`
/// field; the original's WorldGenChunk consumes these through BiomeAbstract subclasses, not ported yet.
#[derive(Debug, Clone, Copy)]
pub struct BiomeNoise {
    pub landscape: f64,
    pub plant: f64,
    pub bush: f64,
    pub temperature: f64,
    pub hygrometry: f64,
    pub ore: f64,
}
