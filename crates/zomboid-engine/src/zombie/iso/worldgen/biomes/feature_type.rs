//! zombie.iso.worldgen.biomes.FeatureType: the five kinds of feature a biome places on a square.

/// zombie.iso.worldgen.biomes.FeatureType: `GROUND, PLANT, BUSH, TREE, ORE` (declaration order;
/// `FeatureType.valueOf` reads the name back from the Lua, `toString` keys the placements table).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FeatureType {
    Ground,
    Plant,
    Bush,
    Tree,
    Ore,
}

impl FeatureType {
    /// FeatureType.values(): the constants in declaration order.
    pub const ALL: &'static [FeatureType] = &[
        FeatureType::Ground,
        FeatureType::Plant,
        FeatureType::Bush,
        FeatureType::Tree,
        FeatureType::Ore,
    ];

    /// `Enum.name()`.
    pub fn name(self) -> &'static str {
        match self {
            FeatureType::Ground => "GROUND",
            FeatureType::Plant => "PLANT",
            FeatureType::Bush => "BUSH",
            FeatureType::Tree => "TREE",
            FeatureType::Ore => "ORE",
        }
    }

    /// The position in `ALL` (an index into the per-type arrays the port stores the Java HashMap's keys in).
    pub fn index(self) -> usize {
        self.ordinal()
    }

    /// `Enum.ordinal()`.
    pub fn ordinal(self) -> usize {
        Self::ALL.iter().position(|c| *c == self).unwrap_or(0)
    }

    /// `FeatureType.valueOf(String)`: `None` is the original's IllegalArgumentException.
    pub fn from_name(name: &str) -> Option<FeatureType> {
        Self::ALL.iter().copied().find(|c| c.name() == name)
    }
}
