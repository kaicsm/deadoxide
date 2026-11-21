//! `zombie.iso.worldgen.attachments.AttachmentsHandler`.
//! Floor-edge overlays use the original eight-direction material rules and RNG draws.

use std::collections::HashMap;

use crate::java::{self, JavaHashMap};
use crate::zombie::core::tile_property_alias_map::TilePropertyAliasMap;
use crate::zombie::iso::iso_object::IsoObject;
use crate::zombie::iso::sprite::iso_sprite_manager::{IsoSpriteManager, SpriteRef};
use crate::zombie::iso::sprite_details::iso_flag_type::IsoFlagType;

use super::blending::{square_at, square_at_mut};
use super::chunks_cache::ChunksCache;
use super::utils::chunk_coord::ChunkCoord;
use super::utils::direction::Direction;
use super::world_gen_chunk::WorldGenChunk;

#[derive(Default)]
pub struct AttachmentsHandler {
    /// Java: Map<FloorMaterial, Map<Direction, List<sprite name>>>.
    attachments: HashMap<String, HashMap<Direction, Vec<SpriteRef>>>,
}

fn attachment_direction(sprite: SpriteRef, sprites: &IsoSpriteManager) -> Result<Direction, String> {
    let props = &sprites.sprite(sprite).properties;
    let location = (i32::from(props.has(IsoFlagType::FloorAttachmentN)) << 0)
        | (i32::from(props.has(IsoFlagType::FloorAttachmentS)) << 1)
        | (i32::from(props.has(IsoFlagType::FloorAttachmentE)) << 2)
        | (i32::from(props.has(IsoFlagType::FloorAttachmentW)) << 3);
    match location {
        1 => Ok(Direction::North), 2 => Ok(Direction::South), 4 => Ok(Direction::East),
        5 => Ok(Direction::NorthEast), 6 => Ok(Direction::SouthEast),
        8 => Ok(Direction::West), 9 => Ok(Direction::NorthWest),
        10 => Ok(Direction::SouthWest),
        _ => Err(format!("java.lang.IllegalArgumentException: invalid attachment direction mask {location}")),
    }
}

fn floor_at(chunks: &ChunksCache, sprites: &IsoSpriteManager, x: i32, y: i32) -> Option<SpriteRef> {
    let sq = square_at(chunks, x, y)?;
    sq.objects.get(sq.get_floor(sprites)?)?.sprite
}

fn floor_material(
    sprite: SpriteRef, sprites: &IsoSpriteManager, alias: &TilePropertyAliasMap,
) -> Option<String> {
    sprites.sprite(sprite).properties.get(alias, "FloorMaterial").map(str::to_owned)
}

impl AttachmentsHandler {
    pub fn new() -> Self { Self::default() }

    /// `loadAttachments`: iterate `IsoSpriteManager.namedMap` in JDK HashMap order.
    /// Sprite allocation handles preserve the original tile-loader insertion order.
    pub fn load_attachments(&mut self, sprites: &IsoSpriteManager, alias: &TilePropertyAliasMap) -> Result<(), String> {
        let mut in_order: Vec<_> = sprites.named_map.iter().collect();
        in_order.sort_by_key(|(_, sprite)| sprite.0);
        let mut names = JavaHashMap::default();
        for (name, sprite) in in_order {
            names.put(name.clone(), *sprite);
        }
        for (_, sprite) in names.iter() {
            let props = &sprites.sprite(*sprite).properties;
            if !props.has(IsoFlagType::IsFloorAttached) { continue; }
            let Some(material) = props.get(alias, "FloorMaterial") else { continue; };
            if material == "null" || material == "None" { continue; }
            let dir = attachment_direction(*sprite, sprites)?;
            self.attachments.entry(material.to_owned()).or_default()
                .entry(dir).or_default().push(*sprite);
        }
        Ok(())
    }

    /// A structural summary of the original sprite catalogue, for parity probes.
    pub fn catalogue_counts(&self) -> (usize, usize) {
        (self.attachments.len(), self.attachments.values().flat_map(|v| v.values()).map(Vec::len).sum())
    }

    /// Replace the catalogue with a single controlled overlay for differential fixtures.
    pub fn set_probe_choice(&mut self, material: &str, direction: Direction, tile: SpriteRef) {
        self.attachments.clear();
        self.attachments.entry(material.to_owned()).or_default().insert(direction, vec![tile]);
    }

    /// `resetAttachments`: does not clear the partial-square list, exactly like Java.
    pub fn reset_attachments(&self, generator: &mut WorldGenChunk, key: ChunkCoord) {
        let state = generator.chunk_states.entry((key.x, key.y)).or_default();
        state.attachments_done_full = false;
        state.attachments_state = [false; 5];
    }

    /// `applyAttachments`: do nothing until all 3x3 neighbour chunks are available.
    /// The full path visits all 64 squares, the partial path only recorded absolute squares.
    pub fn apply_attachments(
        &self,
        generator: &mut WorldGenChunk, chunks: &mut ChunksCache, key: ChunkCoord,
        sprites: &IsoSpriteManager, alias: &TilePropertyAliasMap,
    ) -> Result<(), String> {
        for wx in -1..=1 {
            for wy in -1..=1 {
                if !chunks.contains(ChunkCoord::new(key.x + wx, key.y + wy)) {
                    return Ok(());
                }
            }
        }
        let state = generator.chunk_states.get(&(key.x, key.y)).cloned().unwrap_or_default();
        if state.attachments_partial_initialized {
            for (x, y, _) in &state.attachments_partial {
                self.attach(generator, chunks, sprites, alias, *x, *y, false)?;
            }
        } else {
            for x in key.x * 8..key.x * 8 + 8 {
                for y in key.y * 8..key.y * 8 + 8 {
                    self.attach(generator, chunks, sprites, alias, x, y, true)?;
                }
            }
        }
        let state = generator.chunk_states.entry((key.x, key.y)).or_default();
        state.attachments_state = [true; 5];
        state.attachments_done_full = state.attachments_state.iter().all(|value| *value);
        Ok(())
    }

    fn attach(
        &self, generator: &WorldGenChunk, chunks: &mut ChunksCache,
        sprites: &IsoSpriteManager, alias: &TilePropertyAliasMap,
        x: i32, y: i32, do_priority: bool,
    ) -> Result<(), String> {
        let mut rnd = generator.params.get_random(x, y);
        let Some(floor) = floor_at(chunks, sprites, x, y) else { return Ok(()); };
        let floor_mat = floor_material(floor, sprites, alias);
        let mut materials: HashMap<Direction, Option<String>> = HashMap::new();
        for &dir in Direction::rose() {
            let remote = floor_at(chunks, sprites, x + dir.x(), y + dir.y());
            let value = remote.and_then(|spr| floor_material(spr, sprites, alias));
            if remote.is_some() && Direction::cardinals().contains(&dir) && !do_priority {
                self.delete_core_attachments(chunks, sprites, x, y, dir);
            }
            materials.insert(dir, value);
        }
        let mut found = Vec::new();
        for value in materials.values() {
            if !found.contains(value) { found.push(value.clone()); }
        }
        let mut not_null = Vec::new();
        for value in found.iter().flatten() {
            if !not_null.contains(value) { not_null.push(value.clone()); }
        }
        if not_null.is_empty() || (not_null.len() == 1 && Some(not_null[0].as_str()) == floor_mat.as_deref()) {
            return Ok(());
        }
        let mut valid: Vec<Direction> = Direction::rose().to_vec();
        if found.len() > 1 {
            for &dir in Direction::diagonals() {
                let a = materials.get(&dir).and_then(|v| v.as_deref());
                let b = materials.get(&dir.prev()).and_then(|v| v.as_deref());
                let c = materials.get(&dir.next()).and_then(|v| v.as_deref());
                if a.is_some() && a == b && b == c && a != floor_mat.as_deref() {
                    valid.retain(|v| *v != dir && *v != dir.prev() && *v != dir.next());
                    self.apply_attachment(generator, chunks, sprites, x, y, dir,
                        do_priority, true, &mut rnd, floor_mat.as_deref(), a);
                }
            }
        }
        for &dir in Direction::cardinals() {
            let Some(material) = materials.get(&dir).and_then(|v| v.as_deref()) else { continue; };
            if !valid.contains(&dir) { continue; }
            if floor_at(chunks, sprites, x + dir.x(), y + dir.y()).is_none() { continue; }
            valid.retain(|v| *v != dir);
            self.apply_attachment(generator, chunks, sprites, x + dir.x(), y + dir.y(),
                dir.opposite(), do_priority, false, &mut rnd, Some(material), floor_mat.as_deref());
        }
        Ok(())
    }

    /// Removes at most one matching overlay from each of the two adjacent floor objects.
    fn delete_core_attachments(
        &self, chunks: &mut ChunksCache, sprites: &IsoSpriteManager,
        x: i32, y: i32, dir: Direction,
    ) {
        self.remove_attached_sprite(chunks, sprites, x, y, dir);
        self.remove_attached_sprite(chunks, sprites, x + dir.x(), y + dir.y(), dir.opposite());
    }

    fn remove_attached_sprite(
        &self, chunks: &mut ChunksCache, sprites: &IsoSpriteManager,
        x: i32, y: i32, dir: Direction,
    ) {
        let Some(sq) = square_at_mut(chunks, x, y) else { return; };
        let Some(floor) = sq.get_floor(sprites) else { return; };
        let Some(attached) = sq.objects[floor].attached_anim_sprite.as_mut() else { return; };
        let flag = match dir {
            Direction::North => IsoFlagType::FloorAttachmentN,
            Direction::South => IsoFlagType::FloorAttachmentS,
            Direction::West => IsoFlagType::FloorAttachmentW,
            Direction::East => IsoFlagType::FloorAttachmentE,
            _ => return,
        };
        if let Some(index) = attached.iter().position(|spr| sprites.sprite(*spr).properties.has(flag)) {
            attached.remove(index);
        }
    }

    fn apply_attachment(
        &self, generator: &WorldGenChunk, chunks: &mut ChunksCache,
        _sprites: &IsoSpriteManager, x: i32, y: i32, dir: Direction,
        do_priority: bool, priority_order: bool, rnd: &mut java::Random,
        material: Option<&str>, remote_material: Option<&str>,
    ) {
        let priority = match (material, remote_material) {
            (Some(a), Some(b)) => if priority_order { generator.priority(a, b) } else { generator.priority(b, a) },
            _ => false,
        };
        if material == remote_material || (do_priority && !priority) { return; }
        let Some(remote) = remote_material else { return; };
        let Some(tiles) = self.attachments.get(remote).and_then(|dirs| dirs.get(&dir)) else { return; };
        if tiles.is_empty() { return; }
        let tile = tiles[rnd.next_int_bound(tiles.len() as i32) as usize];
        let Some(sq) = square_at_mut(chunks, x, y) else { return; };
        let Some(floor) = sq.get_floor(_sprites) else { return; };
        let obj: &mut IsoObject = &mut sq.objects[floor];
        obj.attach(tile);
    }
}
