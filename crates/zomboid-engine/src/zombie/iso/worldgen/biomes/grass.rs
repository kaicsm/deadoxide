//! zombie.iso.worldgen.biomes.Grass record.

/// zombie.iso.worldgen.biomes.Grass (fernChance, noGrassDiv, noGrassStages, grassStages): the grass growth
/// parameters of a biome. `Grass.DEFAULT` is the static the zones code uses; WorldGenReader.loadBiome's own
/// default is a different value (0.7F / 3.0F).
#[derive(Clone, Debug)]
pub struct Grass {
    pub fern_chance: f32,
    pub no_grass_div: f32,
    pub no_grass_stages: Vec<f64>,
    pub grass_stages: Vec<f64>,
}

impl Grass {
    /// zombie.iso.worldgen.biomes.Grass.DEFAULT.
    pub fn default_constant() -> Self {
        Grass {
            fern_chance: 0.3,
            no_grass_div: 12.0,
            no_grass_stages: vec![0.4],
            grass_stages: vec![0.33, 0.5],
        }
    }
}
