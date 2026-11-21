//! zombie.iso.worldgen.enums.TileReplacementRetValue record: a tile placement's outcome.

use crate::zombie::iso::sprite::iso_sprite_manager::SpriteRef;

use super::tile_replacement::TileReplacement;

/// zombie.iso.worldgen.enums.TileReplacementRetValue (retval, placed): the result of WorldGenTile.applyTile
/// and the sprite that was placed. The original's `IsoSprite placed` is always a sprite OF THE MANAGER
/// (`IsoSpriteManager.instance.namedMap.get(tile)` or the fallback `IsoSprite.getSprite(instance,
/// "carpentry_02_58", 0)`), never a standalone object — so the port carries the engine's sprite handle
/// (`SpriteRef`, the reference every other sprite collection uses), not a copy of the sprite data, and None is
/// the original's null.
#[derive(Clone, Debug)]
pub struct TileReplacementRetValue {
    pub retval: TileReplacement,
    pub placed: Option<SpriteRef>,
}

impl TileReplacementRetValue {
    /// zombie.iso.worldgen.enums.TileReplacementRetValue.TileReplacementRetValue(TileReplacement, IsoSprite)
    pub fn new(retval: TileReplacement, placed: Option<SpriteRef>) -> Self {
        TileReplacementRetValue { retval, placed }
    }
}
