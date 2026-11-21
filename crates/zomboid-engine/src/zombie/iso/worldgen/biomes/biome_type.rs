//! zombie.iso.worldgen.biomes.BiomeType: the six parameter enums a biome carries, each with the Lua key the
//! `worldgen.selection` table indexes it by (BiomeType.keys), plus java.util.EnumSet over them.

/// The BiomeType enums: `name()` is the Java constant name (`Enum.valueOf` reads it back from the Lua),
/// `KEY` is `BiomeType.keys.get(<class>)`. The Java constants are SCREAMING_SNAKE_CASE (`LIGHT_FOREST`,
/// `VERY_LOW`); the Rust variants keep the crate's PascalCase and each maps to its Java name explicitly,
/// the way feature_type.rs does - `stringify!` on the variant would produce the wrong name.
pub trait BiomeEnum: Copy + PartialEq + std::fmt::Debug + 'static {
    /// BiomeType.keys: the selection table's key of this enum's class.
    const KEY: &'static str;
    /// `<Enum>.values()`: the constants in declaration (ordinal) order.
    const ALL: &'static [Self];

    /// `Enum.name()`.
    fn name(self) -> &'static str;
    /// `Enum.ordinal()`: the position in `ALL`.
    fn ordinal(self) -> usize {
        Self::ALL.iter().position(|c| *c == self).unwrap_or(0)
    }
    /// `Enum.valueOf(Class, String)`: `None` is the original's IllegalArgumentException.
    fn from_name(name: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|c| c.name() == name)
    }
}

macro_rules! biome_enum {
    ($ty:ident, $key:literal, $doc:literal, $( $variant:ident => $name:literal, )+) => {
        #[doc = $doc]
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum $ty {
            $( $variant, )+
        }

        impl BiomeEnum for $ty {
            const KEY: &'static str = $key;
            const ALL: &'static [Self] = &[ $( $ty::$variant, )+ ];

            fn name(self) -> &'static str {
                match self {
                    $( $ty::$variant => $name, )+
                }
            }
        }
    };
}

biome_enum!(
    Landscape,
    "landscape",
    "zombie.iso.worldgen.biomes.BiomeType.Landscape.",
    LightForest => "LIGHT_FOREST",
    Forest => "FOREST",
    Plain => "PLAIN",
    None => "NONE",
);

biome_enum!(
    Plant,
    "plant",
    "zombie.iso.worldgen.biomes.BiomeType.Plant.",
    Flower => "FLOWER",
    Grass => "GRASS",
    None => "NONE",
);

biome_enum!(
    Bush,
    "bush",
    "zombie.iso.worldgen.biomes.BiomeType.Bush.",
    Dry => "DRY",
    Regular => "REGULAR",
    Fat => "FAT",
    None => "NONE",
);

biome_enum!(
    Temperature,
    "temperature",
    "zombie.iso.worldgen.biomes.BiomeType.Temperature.",
    Cold => "COLD",
    Medium => "MEDIUM",
    Hot => "HOT",
    None => "NONE",
);

biome_enum!(
    Hygrometry,
    "hygrometry",
    "zombie.iso.worldgen.biomes.BiomeType.Hygrometry.",
    Flooding => "FLOODING",
    Rain => "RAIN",
    Dry => "DRY",
    None => "NONE",
);

biome_enum!(
    OreLevel,
    "ore_level",
    "zombie.iso.worldgen.biomes.BiomeType.OreLevel.",
    VeryLow => "VERY_LOW",
    Low => "LOW",
    Medium => "MEDIUM",
    High => "HIGH",
    VeryHigh => "VERY_HIGH",
    None => "NONE",
);

/// java.util.EnumSet&lt;T extends Enum&lt;T&gt;&gt;: iteration in ordinal order (the bit-vector sets EnumSet.of and
/// EnumSet.copyOf build), no duplicates; equality is set equality.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EnumSet<T: BiomeEnum> {
    items: Vec<T>,
}

impl<T: BiomeEnum> EnumSet<T> {
    /// `EnumSet.allOf(Class)`.
    pub fn all_of() -> Self {
        EnumSet {
            items: T::ALL.to_vec(),
        }
    }

    /// `EnumSet.of`.
    pub fn of(one: T) -> Self {
        EnumSet { items: vec![one] }
    }

    /// `EnumSet.copyOf(Collection)`: the members, once each, in ordinal order.
    pub fn copy_of(items: &[T]) -> Self {
        let mut sorted = items.to_vec();
        sorted.sort_by_key(|c| c.ordinal());
        sorted.dedup();
        EnumSet { items: sorted }
    }

    /// `EnumSet.contains`.
    pub fn contains(&self, item: T) -> bool {
        self.items.contains(&item)
    }

    /// The set's members, in ordinal order.
    pub fn iter(&self) -> impl Iterator<Item = T> + '_ {
        self.items.iter().copied()
    }

    /// `Objects.equals(set, EnumSet.allOf(...))`: what WorldGenReader.loadBiomes' merge tests.
    pub fn is_all_of(&self) -> bool {
        *self == Self::all_of()
    }
}
