//! zombie.iso.worldgen.WorldGenTile: feature selection, pending multi-tile groups and object placement.

use crate::java::{self, JavaRandom};
use crate::zombie::iso::cell_loader::{self, LoadContext};
use crate::zombie::iso::iso_grid_square::IsoGridSquare;
use crate::zombie::iso::iso_object::{IsoObject, ObjectKind};
use crate::zombie::iso::sprite::iso_sprite_manager::{IsoSpriteManager, SpriteRef};
use crate::zombie::iso::sprite_details::iso_flag_type::IsoFlagType;

use super::biomes::biome::Biome;
use super::biomes::feature::Feature;
use super::biomes::feature_type::FeatureType;
use super::biomes::tile_group::TileGroup;
use super::chunks_cache::ChunksCache;
use super::enums::tile_lookup::TileLookup;
use super::enums::tile_lookup_ret_value::TileLookupRetValue;
use super::enums::tile_replacement::TileReplacement;
use super::enums::tile_replacement_ret_value::TileReplacementRetValue;
use super::roads::road::Road;
use super::utils::chunk_coord::ChunkCoord;
use super::veins::ore_vein::OreVein;
use super::world_gen_utils::can_place;

pub const NO_TREE: &str = "$no_tree";
pub const NO_BUSH: &str = "$no_bush";
pub const NO_GRASS: &str = "$no_grass";
pub const ANY: &str = "$any";
pub const SUBBIOME: &str = "$subbiome";

/// WorldGenChunk.generateChunks: the shared 16x16 pending tile array for each FeatureType.
/// One array spans all four 8x8 chunks; resetting it per chunk would change the Java's placement.
#[derive(Clone, Debug)]
pub struct PendingTiles {
    values: [Vec<Option<String>>; 5],
}

impl Default for PendingTiles {
    fn default() -> Self {
        Self::new()
    }
}

impl PendingTiles {
    /// WorldGenChunk.generateChunks: `new String[256]` for every FeatureType.
    pub fn new() -> Self {
        PendingTiles { values: std::array::from_fn(|_| vec![None; 256]) }
    }

    /// EnumMap.get(type)[x + y * 16].
    pub fn get(&self, feature_type: FeatureType, x: i32, y: i32) -> Option<&str> {
        let index = usize::try_from(x + y * 16).ok()?;
        self.values[feature_type.index()].get(index)?.as_deref()
    }

    /// EnumMap.get(type)[x + y * 16] = value.
    pub fn set(&mut self, feature_type: FeatureType, x: i32, y: i32, value: Option<String>) {
        if (0..16).contains(&x) && (0..16).contains(&y) {
            self.values[feature_type.index()][(x + y * 16) as usize] = value;
        }
    }
}

/// zombie.iso.worldgen.WorldGenTile: its Java class is stateless; all game state is supplied by
/// the caller to replace the Java singletons.
#[derive(Clone, Copy, Debug, Default)]
pub struct WorldGenTile;

impl WorldGenTile {
    /// WorldGenTile.findFeature: one nextFloat for a nonempty candidate list, then accumulate the
    /// *filtered* features scaled by the ratio of unfiltered to filtered total probability.
    pub fn find_feature<'a>(
        &self,
        features: &'a [Feature],
        prefilter_proba: f32,
        postfilter_proba: f32,
        rnd: &mut java::Random,
    ) -> Option<&'a Feature> {
        if features.is_empty() {
            return None;
        }
        let rnd_value = rnd.next_float();
        let mut probability = 0.0f32;
        for feature in features {
            probability += feature.probability.get_value() / postfilter_proba * prefilter_proba;
            if !(rnd_value >= probability) {
                return Some(feature);
            }
        }
        None
    }

    /// WorldGenTile.getBiomeTile: one attempt at choosing a feature group of `target_size`, using
    /// the 16x16 shared `toBeDone` and checking future squares before reserving them.
    #[allow(clippy::too_many_arguments)]
    pub fn get_biome_tile(
        &self,
        biome: &Biome,
        feature_type: FeatureType,
        chunks: &ChunksCache,
        wx: i32,
        wy: i32,
        x: i32,
        y: i32,
        z: i32,
        tile_x: i32,
        tile_y: i32,
        to_be_done: &mut PendingTiles,
        rnd: &mut java::Random,
        target_size: i32,
        sprites: &IsoSpriteManager,
    ) -> TileLookupRetValue {
        self.get_biome_tile_depth(biome, feature_type, chunks, wx, wy, x, y, z, tile_x, tile_y, to_be_done, rnd, target_size, 0, sprites)
    }

    #[allow(clippy::too_many_arguments)]
    fn get_biome_tile_depth(
        &self,
        biome: &Biome,
        feature_type: FeatureType,
        chunks: &ChunksCache,
        wx: i32,
        wy: i32,
        x: i32,
        y: i32,
        z: i32,
        tile_x: i32,
        tile_y: i32,
        to_be_done: &mut PendingTiles,
        rnd: &mut java::Random,
        target_size: i32,
        depth: i32,
        sprites: &IsoSpriteManager,
    ) -> TileLookupRetValue {
        use TileLookup as L;
        let mut tile = to_be_done.get(feature_type, tile_x, tile_y).map(str::to_owned);
        if tile.as_deref().is_none_or(|t| t.is_empty() || t == ANY || t == SUBBIOME) {
            let Some(features) = &biome.features else {
                return TileLookupRetValue::new(L::NoFeatures, None);
            };
            let Some(feature_list) = features.get(feature_type) else {
                return TileLookupRetValue::new(L::NoSpecificType, None);
            };
            if feature_list.is_empty() {
                return TileLookupRetValue::new(L::FeaturesListEmpty, None);
            }
            let filtered: Vec<Feature> = feature_list.iter().filter(|f| f.min_size <= target_size).cloned().collect();
            let mut prefilter_proba = 0.0f32;
            for feature in feature_list {
                prefilter_proba += feature.probability.get_value();
            }
            let mut postfilter_proba = 0.0f32;
            for feature in &filtered {
                postfilter_proba += feature.probability.get_value();
            }
            let Some(feature) = self.find_feature(&filtered, prefilter_proba, postfilter_proba, rnd) else {
                return TileLookupRetValue::new(L::NoFeature, None);
            };
            let tile_groups: Vec<&TileGroup> = feature.tile_groups.iter().filter(|g| g.sx <= target_size && g.sy <= target_size).collect();
            if tile_groups.is_empty() {
                return TileLookupRetValue::new(L::TileGroupsEmpty, None);
            }
            let group = tile_groups[rnd.next_int_bound(tile_groups.len() as i32) as usize];
            if tile_x + group.sx - 1 >= 16 || tile_y + group.sy - 1 >= 16 {
                return TileLookupRetValue::new(L::TooBig, None);
            }
            let cx = x - wx * 8;
            let cy = y - wy * 8;
            if let Some(placements) = &biome.placements {
                if !self.check_future_squares(group, chunks, cx, cy, z, placements.get(feature_type), sprites) {
                    return TileLookupRetValue::new(L::CantBePlaced, None);
                }
            }
            for ix in 0..group.sx {
                for iy in 0..group.sy {
                    if let Some(existing) = to_be_done.get(feature_type, ix + tile_x, iy + tile_y) {
                        if !existing.starts_with('$') {
                            return TileLookupRetValue::new(L::AlreadyPlaced, None);
                        }
                    }
                }
            }
            for ix in 0..group.sx {
                for iy in 0..group.sy {
                    let group_index = (ix + iy * group.sx) as usize;
                    to_be_done.set(feature_type, ix + tile_x, iy + tile_y, group.tiles.get(group_index).cloned());
                }
            }
            tile = to_be_done.get(feature_type, tile_x, tile_y).map(str::to_owned);
            if tile.as_deref() == Some(ANY) {
                to_be_done.set(feature_type, tile_x, tile_y, None);
                if depth < 1 {
                    return self.get_biome_tile_depth(biome, feature_type, chunks, wx, wy, x, y, z, tile_x, tile_y, to_be_done, rnd, target_size, depth + 1, sprites);
                }
            }
        }
        TileLookupRetValue::new(L::Success, tile)
    }

    /// WorldGenTile.checkFutureSquares: the starting square is skipped. An absent neighbour, absent
    /// floor, human structure, or floor not allowed by the biome rejects a multi-tile group.
    #[allow(clippy::too_many_arguments)]
    pub fn check_future_squares(
        &self,
        group: &TileGroup,
        chunks: &ChunksCache,
        tile_x: i32,
        tile_y: i32,
        z: i32,
        placement: &[String],
        sprites: &IsoSpriteManager,
    ) -> bool {
        if group.sx == 1 && group.sy == 1 {
            return true;
        }
        for ix in 0..group.sx {
            for iy in 0..group.sy {
                if ix == 0 && iy == 0 {
                    continue;
                }
                let Some(square) = chunks.get_grid_square(tile_x + ix, tile_y + iy, z) else {
                    return false;
                };
                let Some(floor_index) = square.get_floor(sprites) else {
                    return false;
                };
                if self.is_human_structure(square, sprites) {
                    return false;
                }
                let floor = &square.objects[floor_index];
                let floor_name = floor.sprite.and_then(|spr| sprites.sprite(spr).name.as_deref());
                if !floor_name.is_some_and(|name| can_place(placement, name)) {
                    return false;
                }
            }
        }
        true
    }

    /// WorldGenTile.isHumanStructure: more than a floor and natural groundcover, but no tree.
    pub fn is_human_structure(&self, square: &IsoGridSquare, sprites: &IsoSpriteManager) -> bool {
        let num_grasses = square.objects.iter().filter(|o| is_grass_like(o, sprites)).count();
        let num_bushes = square.objects.iter().filter(|o| is_bush(o, sprites)).count();
        let num_ores = square.objects.iter().filter(|o| is_ore(o, sprites)).count();
        let has_tree = square.objects.iter().any(|o| matches!(&o.kind, ObjectKind::IsoTree(_)));
        square.objects.len().saturating_sub(num_grasses + num_bushes + num_ores) > 1 && !has_tree
    }

    /// WorldGenTile.setTiles: sixteen attempts with `8 >> i` (the ninth reaches zero), matching
    /// the return/continue behavior of the Java lookup loop.
    #[allow(clippy::too_many_arguments)]
    pub fn set_tiles<R: JavaRandom>(
        &self,
        biome: &Biome,
        feature_type: FeatureType,
        chunks: &mut ChunksCache,
        chunk: ChunkCoord,
        x: i32,
        y: i32,
        z: i32,
        tile_x: i32,
        tile_y: i32,
        to_be_done: &mut PendingTiles,
        rnd: &mut java::Random,
        ctx: &mut LoadContext<R>,
    ) -> Result<TileReplacementRetValue, String> {
        for i in 0..16 {
            let target_size = 8 >> i;
            if target_size == 0 {
                return Ok(TileReplacementRetValue::new(TileReplacement::Failure, None));
            }
            let lookup = self.get_biome_tile(biome, feature_type, chunks, chunk.x, chunk.y, x, y, z, tile_x, tile_y, to_be_done, rnd, target_size, ctx.sprites);
            if lookup.tile_lookup == TileLookup::AlreadyPlaced {
                continue;
            }
            if lookup.tile.is_none() && matches!(lookup.tile_lookup, TileLookup::FeaturesListEmpty | TileLookup::NoFeature) {
                return Ok(TileReplacementRetValue::new(TileReplacement::Delete, None));
            }
            if lookup.tile.as_deref().is_some_and(is_pending_tile) {
                return Ok(TileReplacementRetValue::new(TileReplacement::Pending, None));
            }
            if let Some(tile) = lookup.tile {
                let Some(square) = chunks.get_mut(chunk).and_then(|ch| ch.get_grid_square_mut(x - chunk.x * 8, y - chunk.y * 8, z)) else {
                    return Err("NullPointerException: missing worldgen square".to_owned());
                };
                return self.apply_tile(&tile, square, ctx);
            }
        }
        Ok(TileReplacementRetValue::new(TileReplacement::Failure, None))
    }

    /// WorldGenTile.applyTile: IsoChunk.Fix2x followed by namedMap lookup, the carpentry fallback,
    /// then CellLoader.DoTileObjectCreation with the original (unfixed) tile name.
    pub fn apply_tile<R: JavaRandom>(
        &self,
        tile: &str,
        square: &mut IsoGridSquare,
        ctx: &mut LoadContext<R>,
    ) -> Result<TileReplacementRetValue, String> {
        let fixed = super::world_gen_tile_fix2x::fix2x(tile, ctx.rand);
        let sprite = ctx.sprites.named_map.get(fixed.as_str()).copied();
        let (sprite, retval) = match sprite {
            Some(sprite) => (sprite, TileReplacement::Success),
            None => (ctx.sprites.get_sprite("carpentry_02_58"), TileReplacement::Failure),
        };
        let ty = ctx.sprites.sprite(sprite).tile_type;
        cell_loader::do_tile_object_creation(ctx, sprite, ty, square, tile)?;
        Ok(TileReplacementRetValue::new(retval, Some(sprite)))
    }

    /// WorldGenTile.setTile(OreVein): one random tile-group pick and its first tile.
    pub fn set_ore_tile<R: JavaRandom>(
        &self,
        vein: &OreVein,
        square: &mut IsoGridSquare,
        ctx: &mut LoadContext<R>,
        rnd: &mut java::Random,
    ) -> Result<(), String> {
        let groups = vein.get_single_features();
        let index = rnd.next_int_bound(groups.len() as i32) as usize;
        let tile = groups.get(index).and_then(|g| g.tiles.first()).ok_or("missing ore tile group")?;
        self.apply_tile(tile, square, ctx).map(|_| ())
    }

    /// WorldGenTile.setTile(Road): one random tile-group pick and its first tile.
    pub fn set_road_tile<R: JavaRandom>(
        &self,
        road: &Road,
        square: &mut IsoGridSquare,
        ctx: &mut LoadContext<R>,
        rnd: &mut java::Random,
    ) -> Result<(), String> {
        let groups = road.get_single_features();
        let index = rnd.next_int_bound(groups.len() as i32) as usize;
        let tile = groups.get(index).and_then(|g| g.tiles.first()).ok_or("missing road tile group")?;
        self.apply_tile(tile, square, ctx).map(|_| ())
    }

    /// WorldGenTile.getGround: deliberately indexes tileGroups with nextInt(features.size()),
    /// not tileGroups.size(), just as the original Java does.
    pub fn get_ground<'a>(&self, biome: &'a Biome, rnd: &mut java::Random) -> Option<&'a TileGroup> {
        let features = biome.features.as_ref()?.get(FeatureType::Ground)?;
        let feature = features.get(rnd.next_int_bound(features.len() as i32) as usize)?;
        feature.tile_groups.get(rnd.next_int_bound(features.len() as i32) as usize)
    }

    /// WorldGenTile.getPlant: chooses an independent feature and group, with a nextFloat check.
    pub fn get_plant<'a>(&self, biome: &'a Biome, rnd: &mut java::Random) -> Option<&'a TileGroup> {
        let features = biome.features.as_ref()?.get(FeatureType::Plant)?;
        if features.is_empty() {
            return None;
        }
        let feature = features.get(rnd.next_int_bound(features.len() as i32) as usize)?;
        if rnd.next_float() > feature.probability.get_value() {
            None
        } else {
            feature.tile_groups.get(rnd.next_int_bound(feature.tile_groups.len() as i32) as usize)
        }
    }

    /// WorldGenTile.setGround: mark the sprite solidfloor and replace the square's existing floor.
    pub fn set_ground(&self, sprite: SpriteRef, square: &mut IsoGridSquare, sprites: &mut IsoSpriteManager) {
        sprites.sprite_mut(sprite).solidfloor = true;
        if let Some(index) = square.get_floor(sprites) {
            square.objects[index].attached_anim_sprite = None;
            square.objects[index].sprite = Some(sprite);
        }
    }

    /// WorldGenTile.deleteTiles(IsoGridSquare): remove every non-floor object.
    pub fn delete_tiles(&self, square: &mut IsoGridSquare, sprites: &IsoSpriteManager) {
        square.objects.retain(|o| o.has_property(sprites, IsoFlagType::solidfloor));
        square.properties_dirty = true;
    }

    /// WorldGenTile.deleteTiles(IsoGridSquare, List<String>): remove only the matching sprite names.
    pub fn delete_named_tiles(&self, square: &mut IsoGridSquare, sprites: &IsoSpriteManager, names: &[String]) {
        square.objects.retain(|o| {
            !o.sprite.and_then(|s| sprites.sprite(s).name.as_ref()).is_some_and(|name| names.contains(name))
        });
        square.properties_dirty = true;
    }
}

/// WorldGenTile.setTiles: reserved tokens returned to WorldGenChunk.doPending.
pub fn is_pending_tile(tile: &str) -> bool {
    matches!(tile, ANY | SUBBIOME | NO_TREE | NO_BUSH | NO_GRASS)
}

/// IsoObject.isBush: the tileset name or the Bush sprite property.
pub fn is_bush(object: &IsoObject, sprites: &IsoSpriteManager) -> bool {
    object.sprite.is_some_and(|spr| {
        let sprite = sprites.sprite(spr);
        sprite.tileset_name.as_deref() == Some("f_bushes_1") || sprite.is_bush
    })
}

/// IsoObject.isGrass: `e_newgrass_` or `blends_grassoverlays_` sprite name.
pub fn is_grass(object: &IsoObject, sprites: &IsoSpriteManager) -> bool {
    object.sprite.and_then(|spr| sprites.sprite(spr).name.as_deref()).is_some_and(|name| {
        name.starts_with("e_newgrass_") || name.starts_with("blends_grassoverlays_")
    })
}

/// IsoObject.isGrassLike: grass and the three additional families of natural ground cover.
pub fn is_grass_like(object: &IsoObject, sprites: &IsoSpriteManager) -> bool {
    object.sprite.and_then(|spr| sprites.sprite(spr).name.as_deref()).is_some_and(|name| {
        name.starts_with("e_newgrass_")
            || name.starts_with("blends_grassoverlays_")
            || name.starts_with("d_plants_")
            || name.starts_with("d_generic_1_")
            || name.starts_with("d_floorleaves_")
            || name.starts_with("vegetation_groundcover_")
    })
}

/// IsoObject.isOres: the two ore sprite-name families.
pub fn is_ore(object: &IsoObject, sprites: &IsoSpriteManager) -> bool {
    object.sprite.and_then(|spr| sprites.sprite(spr).name.as_deref()).is_some_and(|name| {
        name.starts_with("boulders_") || name.starts_with("crafting_ore_")
    })
}
