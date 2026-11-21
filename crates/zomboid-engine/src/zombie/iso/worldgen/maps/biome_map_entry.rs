//! zombie.iso.worldgen.maps.BiomeMapEntry record.

/// zombie.iso.worldgen.maps.BiomeMapEntry (pixel, biome, ore, zone): one biome-map colour - the pixel
/// value and the biome, ore vein and zone names it selects (any of the three null when the Lua leaves it
/// out; WorldGenReader.loadString's default).
#[derive(Clone, Debug)]
pub struct BiomeMapEntry {
    pub pixel: i32,
    pub biome: Option<String>,
    pub ore: Option<String>,
    pub zone: Option<String>,
}

impl BiomeMapEntry {
    /// BiomeMapEntry's canonical constructor.
    pub fn new(
        pixel: i32,
        biome: Option<String>,
        ore: Option<String>,
        zone: Option<String>,
    ) -> Self {
        BiomeMapEntry {
            pixel,
            biome,
            ore,
            zone,
        }
    }
}
