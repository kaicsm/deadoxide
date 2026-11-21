//! zombie.iso.worldgen.enums.TileLookup: why a biome's tile lookup placed nothing.

/// zombie.iso.worldgen.enums.TileLookup (declaration order): WorldGenTile's getBiomeTile returns one of these
/// with the chosen tile name, and setTiles switches on them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TileLookup {
    NoFeatures,
    NoSpecificType,
    FeaturesListEmpty,
    NoFeature,
    TileGroupsEmpty,
    TooBig,
    CantBePlaced,
    AlreadyPlaced,
    Success,
}

impl TileLookup {
    /// TileLookup.values(): the constants in declaration order.
    pub const ALL: &[TileLookup; 9] = &[
        TileLookup::NoFeatures,
        TileLookup::NoSpecificType,
        TileLookup::FeaturesListEmpty,
        TileLookup::NoFeature,
        TileLookup::TileGroupsEmpty,
        TileLookup::TooBig,
        TileLookup::CantBePlaced,
        TileLookup::AlreadyPlaced,
        TileLookup::Success,
    ];

    /// `Enum.name()`: the Java constant names.
    pub fn name(&self) -> &'static str {
        match self {
            TileLookup::NoFeatures => "NO_FEATURES",
            TileLookup::NoSpecificType => "NO_SPECIFIC_TYPE",
            TileLookup::FeaturesListEmpty => "FEATURES_LIST_EMPTY",
            TileLookup::NoFeature => "NO_FEATURE",
            TileLookup::TileGroupsEmpty => "TILE_GROUPS_EMPTY",
            TileLookup::TooBig => "TOO_BIG",
            TileLookup::CantBePlaced => "CANT_BE_PLACED",
            TileLookup::AlreadyPlaced => "ALREADY_PLACED",
            TileLookup::Success => "SUCCESS",
        }
    }
}
