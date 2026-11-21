//! zombie.iso.worldgen.biomes.Biome / BiomeAbstract: one biome as WorldGenReader loads it.

use crate::java::JavaHashMap;

use super::biome_type::{Bush, EnumSet, Hygrometry, Landscape, OreLevel, Plant, Temperature};
use super::feature::Feature;
use super::feature_type::FeatureType;
use super::grass::Grass;
use super::probability::Probability;
use super::tile_group::TileGroup;

/// `Map<FeatureType, List<Feature>>`: a slot per type, present only when the Lua (or the parent merge) put it.
/// The original's HashMap is only ever read by `get(type)` and filled per type, so the port keeps the five
/// slots instead of the enum-keyed hash table (whose iteration order the original never depends on).
#[derive(Clone, Debug, Default)]
pub struct BiomeFeatures {
    pub ground: Option<Vec<Feature>>,
    pub plant: Option<Vec<Feature>>,
    pub bush: Option<Vec<Feature>>,
    pub tree: Option<Vec<Feature>>,
    pub ore: Option<Vec<Feature>>,
}

impl BiomeFeatures {
    /// `map.get(type)`.
    pub fn get(&self, feature_type: FeatureType) -> Option<&Vec<Feature>> {
        match feature_type {
            FeatureType::Ground => self.ground.as_ref(),
            FeatureType::Plant => self.plant.as_ref(),
            FeatureType::Bush => self.bush.as_ref(),
            FeatureType::Tree => self.tree.as_ref(),
            FeatureType::Ore => self.ore.as_ref(),
        }
    }

    /// `map.put(type, list)`.
    pub fn put(&mut self, feature_type: FeatureType, list: Vec<Feature>) {
        match feature_type {
            FeatureType::Ground => self.ground = Some(list),
            FeatureType::Plant => self.plant = Some(list),
            FeatureType::Bush => self.bush = Some(list),
            FeatureType::Tree => self.tree = Some(list),
            FeatureType::Ore => self.ore = Some(list),
        }
    }

    /// `map.isEmpty()`.
    pub fn is_empty(&self) -> bool {
        self.ground.is_none()
            && self.plant.is_none()
            && self.bush.is_none()
            && self.tree.is_none()
            && self.ore.is_none()
    }
}

/// `Map<FeatureType, List<String>>` as loadPlacements builds it: every type present, GENERIC first.
#[derive(Clone, Debug, Default)]
pub struct Placements {
    pub by_type: [Vec<String>; 5],
}

impl Placements {
    /// `map.get(type)`.
    pub fn get(&self, feature_type: FeatureType) -> &Vec<String> {
        &self.by_type[feature_type.index()]
    }
}

/// `Map<FeatureType, List<IBiome>>`: the sub-biomes of one outer feature type.
#[derive(Clone, Debug, Default)]
pub struct SubBiomeInner {
    pub by_type: [Option<Vec<Biome>>; 5],
}

impl SubBiomeInner {
    /// `map.get(type)`.
    pub fn get(&self, feature_type: FeatureType) -> Option<&Vec<Biome>> {
        self.by_type[feature_type.index()].as_ref()
    }
}

/// `Map<FeatureType, Map<FeatureType, List<IBiome>>>`: `subbiomes` in the Lua.
#[derive(Clone, Debug, Default)]
pub struct SubBiomes {
    pub by_type: [Option<SubBiomeInner>; 5],
}

impl SubBiomes {
    /// `map.isEmpty()`.
    pub fn is_empty(&self) -> bool {
        self.by_type.iter().all(|m| m.is_none())
    }

    /// `map.get(type)`.
    pub fn get(&self, feature_type: FeatureType) -> Option<&SubBiomeInner> {
        self.by_type[feature_type.index()].as_ref()
    }

    /// `map.put(type, inner)`.
    pub fn put(&mut self, feature_type: FeatureType, inner: SubBiomeInner) {
        self.by_type[feature_type.index()] = Some(inner);
    }
}

/// zombie.iso.worldgen.biomes.Biome, the fields of BiomeAbstract plus the two the subclass owns. A `None`
/// feature set, placements or grass is the original's null; the six EnumSets always have a value (the
/// constructor keeps the allOf default when the reader passes null). `replacements` is null only for
/// DEFAULT_BIOME: loadReplacements always builds a map, and the parent merge of WorldGenReader.loadBiomes
/// builds every merged biome its own FRESH map (`putAll` of each chain member, root first) - the original
/// never aliases one biome's map into another.
#[derive(Clone, Debug)]
pub struct Biome {
    pub name: String,
    pub parent: Option<String>,
    pub generate: bool,
    pub features: Option<BiomeFeatures>,
    pub replacements: Option<JavaHashMap<Vec<Feature>>>,
    pub landscape: EnumSet<Landscape>,
    pub plant: EnumSet<Plant>,
    pub bush: EnumSet<Bush>,
    pub temperature: EnumSet<Temperature>,
    pub hygrometry: EnumSet<Hygrometry>,
    pub ore_level: EnumSet<OreLevel>,
    pub zombies: f32,
    pub placements: Option<Placements>,
    pub sub_biomes: SubBiomes,
    pub protected_list: Vec<String>,
    pub grass: Option<Grass>,
}

impl Biome {
    /// zombie.iso.worldgen.biomes.Biome.&lt;init&gt;: a null EnumSet parameter keeps BiomeAbstract's allOf
    /// default (the reader's loadBiomeType returns null when the Lua has no such list).
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        name: String,
        parent: Option<String>,
        generate: bool,
        features: Option<BiomeFeatures>,
        replacements: Option<JavaHashMap<Vec<Feature>>>,
        landscape: Option<EnumSet<Landscape>>,
        plant: Option<EnumSet<Plant>>,
        bush: Option<EnumSet<Bush>>,
        temperature: Option<EnumSet<Temperature>>,
        hygrometry: Option<EnumSet<Hygrometry>>,
        ore_level: Option<EnumSet<OreLevel>>,
        zombies: f32,
        placements: Option<Placements>,
        sub_biomes: SubBiomes,
        protected_list: Vec<String>,
        grass: Option<Grass>,
    ) -> Self {
        let mut b = Biome {
            name,
            parent,
            generate,
            features,
            replacements,
            landscape: EnumSet::all_of(),
            plant: EnumSet::all_of(),
            bush: EnumSet::all_of(),
            temperature: EnumSet::all_of(),
            hygrometry: EnumSet::all_of(),
            ore_level: EnumSet::all_of(),
            zombies,
            placements,
            sub_biomes,
            protected_list,
            grass,
        };
        if let Some(v) = landscape {
            b.landscape = v;
        }
        if let Some(v) = plant {
            b.plant = v;
        }
        if let Some(v) = bush {
            b.bush = v;
        }
        if let Some(v) = temperature {
            b.temperature = v;
        }
        if let Some(v) = hygrometry {
            b.hygrometry = v;
        }
        if let Some(v) = ore_level {
            b.ore_level = v;
        }
        b
    }

    /// zombie.iso.worldgen.biomes.Biome.DEFAULT_BIOME: what BiomeRegistry returns when no biome matches.
    /// A method (not a constant) because the original's static is one shared object the port rebuilds.
    pub fn default_biome() -> Self {
        let mut features = BiomeFeatures::default();
        features.put(
            FeatureType::Ground,
            vec![Feature {
                tile_groups: vec![TileGroup {
                    sx: 1,
                    sy: 1,
                    tiles: vec!["carpentry_02_58".to_string()],
                }],
                min_size: 1,
                max_size: 1,
                probability: Probability::proba_double(1.0),
            }],
        );
        Biome::new(
            "DEFAULT".to_string(),
            None,
            true,
            Some(features),
            None,
            Some(EnumSet::of(Landscape::None)),
            Some(EnumSet::of(Plant::None)),
            Some(EnumSet::of(Bush::None)),
            Some(EnumSet::of(Temperature::None)),
            Some(EnumSet::of(Hygrometry::None)),
            Some(EnumSet::of(OreLevel::None)),
            0.0,
            Some(Placements::default()),
            SubBiomes::default(),
            Vec::new(),
            Some(Grass {
                fern_chance: 0.0,
                no_grass_div: 1.0,
                no_grass_stages: vec![0.4],
                grass_stages: vec![0.33, 0.5],
            }),
        )
    }
}
