//! zombie.iso.worldgen.enums.TileLookupRetValue record: a biome tile lookup's outcome.

use super::tile_lookup::TileLookup;

/// zombie.iso.worldgen.enums.TileLookupRetValue (tileLookup, tile): why a getBiomeTile lookup ended and the
/// tile name it chose — the String is null in the original on every early failure, Some only on the paths
/// that picked a tile.
#[derive(Clone, Debug)]
pub struct TileLookupRetValue {
    pub tile_lookup: TileLookup,
    pub tile: Option<String>,
}

impl TileLookupRetValue {
    /// zombie.iso.worldgen.enums.TileLookupRetValue.TileLookupRetValue(TileLookup, String)
    pub fn new(tile_lookup: TileLookup, tile: Option<String>) -> Self {
        TileLookupRetValue { tile_lookup, tile }
    }
}
