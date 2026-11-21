//! Native Project Zomboid 42.21 / world version 249 chunk writer.
//!
//! Mirrors IsoChunk.Save, IsoGridSquare.save and IsoObject.save. Unsupported
//! dynamic state is rejected rather than silently discarded on disk.
use std::fs;
use std::io::Write as _;
use std::path::Path;

use crate::java;
use crate::java_nio::{pack_float_unit_to_byte, write_string, BitHeaderWrite, ByteBufferWriter, HeaderSize};
use crate::zombie::world::dictionary_data::DictionaryData;
use super::chunk_save::{ChunkSave, ClassSave, ObjectSave, SquareSave, WORLD_VERSION};
use super::iso_chunk::IsoChunk;
use super::iso_grid_square::IsoGridSquare;
use super::iso_object::{IsoObject, ObjectKind};
use super::sprite::iso_sprite_manager::{IsoSpriteManager, SpriteRef};

fn byte(v: bool) -> i8 { i8::from(v) }
fn flags<const N: usize>(v: [bool; N]) -> i8 {
    v.into_iter().enumerate().fold(0u8, |acc, (i, on)| acc | (u8::from(on) << i)) as i8
}
fn sprite_id(sprite: Option<SpriteRef>, sprites: &IsoSpriteManager) -> i32 {
    sprite.map(|s| sprites.sprite(s).id).unwrap_or(-1)
}
fn optional_sprite(w: &mut ByteBufferWriter, sprite: Option<SpriteRef>, sprites: &IsoSpriteManager) {
    w.put(byte(sprite.is_some()));
    if sprite.is_some() { w.put_int(sprite_id(sprite, sprites)); }
}
fn as_short(value: usize, context: &str) -> Result<i16, String> {
    i16::try_from(value).map_err(|_| format!("{context}: {value} exceeds Java short"))
}

fn write_object(
    w: &mut ByteBufferWriter, obj: &IsoObject, sprites: &IsoSpriteManager,
    dictionary: Option<&DictionaryData>, room_id: i64,
) -> Result<(), String> {
    // IsoObject.factoryGetClassID; classes absent from the Rust load path are deliberately rejected.
    let id = match obj.kind {
        ObjectKind::IsoObject => 0,
        ObjectKind::IsoCurtain(_) => 8,
        ObjectKind::IsoRadio => 9,
        ObjectKind::IsoTelevision => 10,
        ObjectKind::IsoBarbecue { .. } => 12,
        ObjectKind::IsoClothingDryer => 13,
        ObjectKind::IsoClothingWasher => 14,
        ObjectKind::IsoFireplace => 15,
        ObjectKind::IsoStove => 16,
        ObjectKind::IsoDoor(_) => 17,
        ObjectKind::IsoBrokenGlass => 20,
        ObjectKind::IsoWindow(_) => 26,
        ObjectKind::IsoTree(_) => 28,
        ObjectKind::IsoLightSwitch => 29,
        ObjectKind::IsoCombinationWasherDryer => 34,
        ObjectKind::IsoWindowFrame { .. } => 41,
        _ => return Err(format!("cannot persist unsupported object {}", obj.kind.class_name())),
    };
    let saved = obj.save.as_deref();
    let empty = ObjectSave::default();
    let s = saved.unwrap_or(&empty);
    if !s.containers.is_empty() || s.mod_data.is_some() || !s.components.is_empty()
        || !s.wall_blood_splats.is_empty() || s.sprite_model_name.is_some()
    {
        return Err(format!("cannot persist unsupported dynamic state on {}", obj.kind.class_name()));
    }
    w.put(1); // IsoObject.Serialize
    w.put(id);
    w.put_int(sprite_id(obj.sprite, sprites));
    let mut header = BitHeaderWrite::alloc_write(HeaderSize::Byte, w);
    if let Some(attached) = &obj.attached_anim_sprite {
            // Java writes an attachment header even for a non-null empty list.
            let len = attached.len().min(255);
            header.add_flags(1);
            if len == 1 { header.add_flags(2); } else { w.put(len as i8); }
            if !s.attached.is_empty() && s.attached.len() != attached.len() {
                return Err("attached sprite/save metadata count mismatch".into());
            }
            for (i, spr) in attached.iter().take(len).enumerate() {
                let a = s.attached.get(i);
                w.put_int(sprites.sprite(*spr).id);
                let mut f = 0;
                let data = a.and_then(|a| a.data);
                if data.is_some() { f |= 2; }
                if a.is_some_and(|a| a.flip) { f |= 4; }
                if a.map_or(true, |a| a.copy_target_alpha) { f |= 8; }
                if a.is_some_and(|a| a.multiply_object_alpha) { f |= 16; }
                w.put(f);
                if let Some(data) = data {
                    for n in data.iter().take(3) { w.put_float(*n); }
                    for n in data.iter().skip(3) { w.put(pack_float_unit_to_byte(*n)); }
                }
                if f & 16 != 0 { w.put_float(a.and_then(|a| a.alpha).unwrap_or(1.0)); }
            }
    }
    let object_name = s.name.as_deref().or(obj.name.as_deref());
    if object_name.is_some() || s.sprite_name.is_some() {
        header.add_flags(4);
        let mut f = 0u8;
        let mut name_id = None;
        let mut sprite_name_id = None;
        if let Some(name) = object_name {
            if name == "Grass" { f |= 2; }
            else if let Some(id) = dictionary.and_then(|d| d.object_ids.get(name)) { f |= 4; name_id = Some(*id); }
            else { f |= 8; }
        }
        if let Some(name) = s.sprite_name.as_deref() {
            if let Some(id) = dictionary.and_then(|d| d.sprite_ids.get(name)) { f |= 16; sprite_name_id = Some(*id); }
            else { f |= 32; }
        }
        w.put(f as i8);
        if let Some(name) = object_name.filter(|n| *n != "Grass") {
            if let Some(id) = name_id { w.put(id); } else { write_string(w, Some(name)); }
        }
        if let Some(name) = s.sprite_name.as_deref() {
            if let Some(id) = sprite_name_id { w.put_int(id); } else { write_string(w, Some(name)); }
        }
    }
    if let Some(col) = s.custom_color {
        header.add_flags(8);
        for c in col { w.put(pack_float_unit_to_byte(c)); }
    }
    if s.do_not_sync { header.add_flags(16); }
    if obj.outline_on_mouseover { header.add_flags(32); }
    let mut extra = BitHeaderWrite::alloc_write(HeaderSize::Short, w);
    if s.special_tooltip { extra.add_flags(8); }
    if s.key_id != -1 { extra.add_flags(16); w.put_int(s.key_id); }
    if s.uses_external_water_source { extra.add_flags(32); }
    if let Some(h) = s.sheet_rope_health { extra.add_flags(64); w.put_float(h); }
    if obj.render_y_offset != 0.0 { extra.add_flags(128); w.put_float(obj.render_y_offset); }
    if let Some(overlay) = s.overlay_sprite {
        extra.add_flags(256);
        let name = sprites.sprite(overlay).name.as_deref().ok_or("overlay sprite has no name")?;
        if let Some(id) = dictionary.and_then(|d| d.sprite_ids.get(name)) {
            w.put_int(*id);
        } else {
            extra.add_flags(512);
            write_string(w, Some(name));
        }
        if let Some(color) = s.overlay_sprite_color {
            extra.add_flags(1024);
            for c in color { w.put(pack_float_unit_to_byte(c)); }
        }
    }
    if s.moved_thumpable { extra.add_flags(2048); }
    if !extra.equals(0) {
        header.add_flags(64);
        extra.write(w)?;
    } else {
        w.set_position(extra.start_position())?;
    }
    header.write(w)?;

    // IsoObject subclasses append their own fields after the base BitHeader.
    match &obj.kind {
        ObjectKind::IsoObject | ObjectKind::IsoBrokenGlass => {}
        ObjectKind::IsoTree(t) => { w.put(t.log_yield as i8); w.put((t.damage / 10) as i8); }
        ObjectKind::IsoWindowFrame { north } => w.put(byte(*north)),
        ObjectKind::IsoDoor(d) => {
            let (health, max_health, key_id, curtain) = match &s.class {
                ClassSave::Door { health, max_health, key_id, has_curtain, curtain_open, curtain_inside } =>
                    (*health, *max_health, *key_id, u8::from(*has_curtain) | (u8::from(*curtain_open) << 1) | (u8::from(*curtain_inside) << 2)),
                ClassSave::None => (500, 500, -1, 0),
                _ => return Err("door's saved class is incompatible".into()),
            };
            w.put(byte(d.open)); w.put(byte(d.locked)); w.put(byte(d.north));
            w.put_int(health); w.put_int(max_health);
            w.put_int(sprite_id(d.closed_sprite, sprites)); w.put_int(sprite_id(d.open_sprite, sprites));
            w.put_int(key_id); w.put(byte(d.locked_by_key)); w.put(curtain as i8);
        }
        ObjectKind::IsoWindow(win) => {
            let (health, max_health) = match &s.class {
                ClassSave::Window { health, max_health } => (*health, *max_health),
                ClassSave::None => (50, 50),
                _ => return Err("window's saved class is incompatible".into()),
            };
            w.put(byte(win.open)); w.put(byte(win.north)); w.put_int(health);
            w.put(byte(win.locked)); w.put(byte(win.perma_locked));
            w.put(byte(win.smashed)); w.put(byte(win.glass_removed));
            optional_sprite(w, win.open_sprite, sprites); optional_sprite(w, win.closed_sprite, sprites);
            optional_sprite(w, win.smashed_sprite, sprites); optional_sprite(w, win.glass_removed_sprite, sprites);
            w.put_int(max_health);
        }
        ObjectKind::IsoCurtain(c) => {
            let (health, barricade) = match &s.class {
                ClassSave::Curtain { health, barricade_strength } => (*health, *barricade_strength),
                ClassSave::None => (1000, 0),
                _ => return Err("curtain's saved class is incompatible".into()),
            };
            w.put(byte(c.open)); w.put(byte(c.north)); w.put_int(health); w.put_int(barricade);
            w.put_int(sprite_id(if c.open { c.closed_sprite } else { c.open_sprite }, sprites));
        }
        ObjectKind::IsoLightSwitch => {
            let ls = match &s.class {
                ClassSave::LightSwitch(ls) => ls.clone(),
                ClassSave::None => super::chunk_save::LightSwitchSave { room_id, ..Default::default() },
                _ => return Err("light switch's saved class is incompatible".into()),
            };
            w.put(byte(ls.light_room)); w.put_long(ls.room_id); w.put(byte(ls.activated));
            w.put(byte(ls.can_be_modified));
            if ls.can_be_modified {
                w.put(byte(ls.use_battery)); w.put(byte(ls.has_battery));
                w.put(byte(ls.bulb_item.is_some()));
                if ls.bulb_item.is_some() { write_string(w, ls.bulb_item.as_deref()); }
                w.put_float(ls.power); w.put_float(ls.delta);
                for c in ls.primary { w.put_float(c); }
            }
            w.put_long(ls.last_minute_stamp); w.put_int(ls.bulb_burn_minutes);
        }
        ObjectKind::IsoStove => {
            let (on, timer, temp, first, broken) = match &s.class {
                ClassSave::Stove { activated, seconds_timer, max_temperature, first_turn_on, broken } => (*activated, *seconds_timer, *max_temperature, *first_turn_on, *broken),
                ClassSave::None => (false, -1, 0.0, false, false),
                _ => return Err("stove saved class is incompatible".into()),
            };
            w.put(byte(on)); w.put_int(timer); w.put_float(temp); w.put(byte(first)); w.put(byte(broken));
        }
        ObjectKind::IsoFireplace => {
            let (fuel, lit, time, extinguished) = match &s.class {
                ClassSave::Fireplace { fuel_amount_minutes, lit, last_update_time, minutes_since_extinguished } => (*fuel_amount_minutes, *lit, *last_update_time, *minutes_since_extinguished),
                ClassSave::None => (0, false, 0.0, 0),
                _ => return Err("fireplace saved class is incompatible".into()),
            };
            w.put_int(fuel); w.put(byte(lit)); w.put_float(time); w.put_int(extinguished);
        }
        ObjectKind::IsoBarbecue { has_propane_tank, fuel_amount_minutes } => {
            let (lit, time, ext, normal, no_tank) = match &s.class {
                ClassSave::Barbecue { lit, last_update_time, minutes_since_extinguished, normal, no_tank } => (*lit, *last_update_time, *minutes_since_extinguished, *normal, *no_tank),
                ClassSave::None => (false, 0.0, 0, None, None),
                _ => return Err("barbecue saved class is incompatible".into()),
            };
            w.put(byte(*has_propane_tank)); w.put_int(*fuel_amount_minutes); w.put(byte(lit));
            w.put_float(time); w.put_int(ext); optional_sprite(w, normal, sprites); optional_sprite(w, no_tank, sprites);
        }
        ObjectKind::IsoClothingWasher => {
            let (on, time) = match &s.class { ClassSave::Washer { activated, last_update } => (*activated, *last_update), ClassSave::None => (false, 0.0), _ => return Err("washer saved class is incompatible".into()) };
            w.put(byte(on)); w.put_float(time);
        }
        ObjectKind::IsoClothingDryer => {
            let on = match &s.class { ClassSave::Dryer { activated } => *activated, ClassSave::None => false, _ => return Err("dryer saved class is incompatible".into()) };
            w.put(byte(on));
        }
        ObjectKind::IsoCombinationWasherDryer => {
            let (dryer, wa, wt, da) = match &s.class {
                ClassSave::WasherDryer { dryer, washer_activated, washer_last_update, dryer_activated } => (*dryer, *washer_activated, *washer_last_update, *dryer_activated),
                ClassSave::None => (false, false, 0.0, false),
                _ => return Err("combined washer/dryer saved class is incompatible".into()),
            };
            w.put(byte(dryer)); w.put(byte(wa)); w.put_float(wt); w.put(byte(da));
        }
        ObjectKind::IsoRadio | ObjectKind::IsoTelevision => {
            if !matches!(&s.class, ClassSave::None | ClassSave::WaveSignal(None)) { return Err("radio device data writer not ported".into()); }
            w.put(0);
        }
        _ => return Err(format!("unsupported save class {}", obj.kind.class_name())),
    }
    Ok(())
}

fn write_square(
    w: &mut ByteBufferWriter, sq: &IsoGridSquare, sprites: &IsoSpriteManager,
    dictionary: Option<&DictionaryData>,
) -> Result<(), String> {
    let empty = SquareSave::default();
    let s = sq.save.as_deref().unwrap_or(&empty);
    if !s.static_moving_objects.is_empty() || s.mod_data.is_some() || s.burnt_out || s.have_sheet_rope {
        return Err(format!("unsupported square extra state at {},{},{}", sq.x, sq.y, sq.z));
    }
    let mut erosion = Vec::new();
    if let Some(e) = &sq.erosion { e.save(&mut erosion); }
    else { s.erosion.save(&mut erosion); }
    w.put_bytes(&erosion);
    let mut head = BitHeaderWrite::alloc_write(HeaderSize::Byte, w);
    if !sq.objects.is_empty() {
        head.add_flags(1);
        match sq.objects.len() {
            2 => head.add_flags(2),
            3 => head.add_flags(4),
            n if n >= 4 => { head.add_flags(8); w.put_short(as_short(n, "square object count")?); }
            _ => {}
        }
        for (index, obj) in sq.objects.iter().enumerate() {
            let in_world = s.world_objects.contains(&index);
            w.put((u8::from(obj.special) * 2 | u8::from(in_world) * 4) as i8);
            write_object(w, obj, sprites, dictionary, sq.room_id)?;
        }
    }
    if s.overlay_done { head.add_flags(16); }
    if sq.have_roof { head.add_flags(32); }
    // IsoGridSquare.save always reserves a nested byte BitHeader and rewinds if empty.
    let extra = BitHeaderWrite::alloc_write(HeaderSize::Byte, w);
    w.set_position(extra.start_position())?;
    w.put(s.vis);
    head.write(w)?;
    Ok(())
}

impl IsoChunk {
    /// zombie.iso.IsoChunk.Save(ByteBuffer, CRC32, boolean): native v249 map chunk data.
    /// No in-memory state is mutated. Unsupported game data aborts the entire save.
    pub fn save_to_buffer(
        &self, sprites: &IsoSpriteManager, dictionary: Option<&DictionaryData>,
    ) -> Result<Vec<u8>, String> {
        let empty = ChunkSave::default();
        let s = self.save.as_deref().unwrap_or(&empty);
        if !s.floor_blood_splats.is_empty() { return Err("floor blood splat writer not ported".into()); }
        let mut w = ByteBufferWriter::new();
        w.put(0); w.put_int(WORLD_VERSION); w.put_int(0); w.put_long(-1);
        w.put(byte(s.blending_done_full));
        w.put(flags(s.blending_modified));
        w.put(byte(s.blending_done_partial));
        if s.blending_modified != [false; 4] && s.blending_done_partial {
            for d in s.blending_depth { w.put(d); }
        }
        w.put(byte(s.attachments_done_full));
        w.put(flags(s.attachments_state));
        if let Some(partial) = &s.attachments_partial {
            w.put_short(as_short(partial.len(), "attachmentsPartial")?);
            for (x, y, z) in partial { w.put_int(*x); w.put_int(*y); w.put_int(*z); }
        } else { w.put_short(0); }
        let levels = w.position();
        w.put_int(self.get_max_level()); w.put_int(self.get_min_level());
        w.put_int(0); // floor blood splats
        let mut min_written = i32::MAX;
        let mut max_written = i32::MIN;
        for x in 0..8 {
            for y in 0..8 {
                let at = w.position();
                w.put_long(0);
                let mut mask = 0u64;
                for z in self.get_min_level()..=self.get_max_level() {
                    if let Some(sq) = self.get_grid_square(x, y, z) {
                        if !sq.objects.is_empty() || z == 0 {
                            if !(-32..=31).contains(&z) { return Err(format!("square z={z} out of save range")); }
                            min_written = min_written.min(z);
                            max_written = max_written.max(z);
                            mask |= 1u64 << (z + 32);
                            write_square(&mut w, sq, sprites, dictionary)?;
                        }
                    }
                }
                let end = w.position();
                w.set_position(at)?; w.put_long(mask as i64); w.set_position(end)?;
            }
        }
        if min_written <= max_written {
            let end = w.position();
            w.set_position(levels)?;
            w.put_int(max_written); w.put_int(min_written);
            w.set_position(end)?;
        }
        let mut erosion = Vec::new();
        // Java always serializes IsoChunk.getErosionData(), not the stale
        // snapshot retained by the Rust save reader.
        self.erosion.save(&mut erosion);
        w.put_bytes(&erosion);
        if let Some(generators) = &s.generators_touching_this_chunk {
            w.put_short(as_short(generators.len(), "generatorsTouchingThisChunk")?);
            for (x,y,z) in generators { w.put_int(*x); w.put_int(*y); w.put(*z as i8); }
        } else { w.put_short(0); }
        w.put_short(0); // no saved vehicles in Rust model
        w.put_int(s.loot_respawn_hour);
        w.put_short(as_short(s.spawned_rooms.len().min(i16::MAX as usize), "spawnedRooms")?);
        for id in s.spawned_rooms.iter().take(i16::MAX as usize) { w.put_long(*id); }
        let end = w.position();
        let crc = java::crc32(w.bytes_from(17));
        w.set_position(5)?; w.put_int(i32::try_from(end).map_err(|_| "chunk save exceeds Java i32 length")?);
        w.put_long(i64::from(crc)); w.set_position(end)?;
        Ok(w.bytes().to_vec())
    }

    /// IsoChunk.SafeWrite: write a single native chunk under save/map/<wx>/<wy>.bin.
    /// A same-directory temporary file is flushed and atomically renamed after success.
    pub fn save_to_disk(
        &self, save_dir: &Path, sprites: &IsoSpriteManager,
        dictionary: Option<&DictionaryData>,
    ) -> Result<(), String> {
        let data = self.save_to_buffer(sprites, dictionary)?;
        Self::save_bytes_to_disk(save_dir, self.wx, self.wy, &data)
    }

    /// Serialize all chunks first; then atomically replace each individual chunk file.
    /// The caller owns recovery/coordination across multiple chunk files.
    pub fn save_bytes_to_disk(save_dir: &Path, wx: i32, wy: i32, data: &[u8]) -> Result<(), String> {
        let dir = save_dir.join("map").join(wx.to_string());
        fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let path = dir.join(format!("{wy}.bin"));
        // Refuse overwriting an existing player's chunk. The importer only calls this
        // for freshly generated coordinates, but concurrent writers must be excluded too.
        if path.exists() { return Err(format!("{} already exists; not overwriting", path.display())); }
        let temp = dir.join(format!("{wy}.bin.deadoxide-tmp-{}", std::process::id()));
        let written = (|| -> Result<(), String> {
            let mut file = fs::File::create_new(&temp).map_err(|e| format!("{}: {e}", temp.display()))?;
            file.write_all(&data).map_err(|e| format!("{}: {e}", temp.display()))?;
            file.sync_all().map_err(|e| format!("{}: {e}", temp.display()))?;
            // Do not clobber an existing file even if it appeared while serialization ran.
            fs::hard_link(&temp, &path).map_err(|e| format!("{} -> {}: {e}", temp.display(), path.display()))?;
            fs::remove_file(&temp).map_err(|e| format!("{}: {e}", temp.display()))
        })();
        if written.is_err() { let _ = fs::remove_file(&temp); }
        written
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::java_nio::ByteBuffer;
    use crate::zombie::iso::iso_grid_square::IsoGridSquare;
    use crate::zombie::iso::iso_object::{IsoObject, ObjectKind};

    #[test]
    fn native_world_version_and_crc() {
        let sprites = IsoSpriteManager::new();
        let mut chunk = IsoChunk::new(20, 30);
        let mut square = IsoGridSquare::new(160, 240, 0);
        square.objects.push(IsoObject::new(ObjectKind::IsoObject, None));
        chunk.set_square(0, 0, 0, Some(square));
        let bytes = chunk.save_to_buffer(&sprites, None).expect("native chunk");
        let mut b = ByteBuffer::wrap(&bytes);
        assert_eq!(b.get().expect("debug"), 0);
        assert_eq!(b.get_int().expect("world version"), WORLD_VERSION);
        assert_eq!(b.get_int().expect("length") as usize, bytes.len());
        assert_eq!(b.get_long().expect("CRC"), i64::from(java::crc32(&bytes[17..])));
    }

    #[test]
    fn unsupported_object_does_not_write_partial_file() {
        let sprites = IsoSpriteManager::new();
        let mut chunk = IsoChunk::new(1, 1);
        let mut sq = IsoGridSquare::new(8, 8, 0);
        sq.objects.push(IsoObject::new(ObjectKind::IsoMannequin, None));
        chunk.set_square(0, 0, 0, Some(sq));
        assert!(chunk.save_to_buffer(&sprites, None).is_err());
    }
}
