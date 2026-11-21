//! zombie.iso.worldgen.biomes.TileGroup record.

/// zombie.iso.worldgen.biomes.TileGroup (sx, sy, tiles): a rectangular group of tile names; a plain string in
/// the Lua is the 1x1 group.
#[derive(Clone, Debug)]
pub struct TileGroup {
    pub sx: i32,
    pub sy: i32,
    pub tiles: Vec<String>,
}
