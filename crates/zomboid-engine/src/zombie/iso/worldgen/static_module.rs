//! zombie.iso.worldgen.StaticModule record.

use super::biomes::biome::Biome;
use super::prefab_structure::PrefabStructure;

/// zombie.iso.worldgen.StaticModule (biome, prefab, xmin, xmax, ymin, ymax): one entry of
/// `worldgen.static_modules` - a biome and/or a prefab structure placed inside a rectangle of the world
/// map's pixels (WorldGenReader.loadStaticModules demands at least one of the two).
#[derive(Clone, Debug)]
pub struct StaticModule {
    /// The module's inline `biome` table; `None` is the original's null (loadBiome of a null table).
    pub biome: Option<Biome>,
    /// The module's `prefab` table; `None` is the original's null (loadPrefab of a null table).
    pub prefab: Option<PrefabStructure>,
    pub xmin: i32,
    pub xmax: i32,
    pub ymin: i32,
    pub ymax: i32,
}

impl StaticModule {
    /// StaticModule's canonical constructor.
    pub fn new(
        biome: Option<Biome>,
        prefab: Option<PrefabStructure>,
        xmin: i32,
        xmax: i32,
        ymin: i32,
        ymax: i32,
    ) -> Self {
        StaticModule {
            biome,
            prefab,
            xmin,
            xmax,
            ymin,
            ymax,
        }
    }
}
