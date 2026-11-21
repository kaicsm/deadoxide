//! Dumps engine state in the exact text format of the Java probes in `tests/original_probe`, for byte-for-byte diffs.
//!   pz-dump tiledefs <game root>                  (pairs with TileDefsProbe.java)
//!   pz-dump fonts <game root> <scratch>           (pairs with FontsProbe.java)
//!   pz-dump lotheaders <game root> <map folder>   (pairs with LotHeadersProbe.java)
//!   pz-dump lotpack <game root> <map folder> x1 y1 x2 y2   (pairs with LotPackProbe.java)
//!   pz-dump mapmeta <game root> <save dir>    (pairs with MapMetaProbe.java)
//!   pz-dump mapzone <game root> <save dir>    (pairs with MapZoneProbe.java)
//!   pz-dump animalzones <game root> <save dir>   (pairs with AnimalZonesProbe.java)
//!   pz-dump zoneoutlines <game root> <save dir> outlines   (MapZoneProbe.java ... outlines)
//!   pz-dump zonetriangles <game root> <save dir> triangles (MapZoneProbe.java ... triangles)
//!   pz-dump translator <game root> <scratch, unused> <command file>   (TranslatorProbe.java)
//!   pz-dump recipes <game root>               (pairs with RecipesProbe.java)
//!   pz-dump animsets <game root> <scratch dir>   (pairs with AnimSetsProbe.java)
//!   pz-dump sounds <game root> [<command file> <scratch dir>]   (pairs with SoundsProbe.java)
//!   pz-dump fmodbanks <game root> <scratch, unused> banks   (pairs with FmodBanksProbe.java)
#[path = "pz-dump-fmod/mod.rs"]
mod fmod_dump;
#[path = "pz-dump-inventory/mod.rs"]
mod inventory_dump;
#[path = "pz-dump-models/mod.rs"]
mod models;
#[path = "pz-dump-path/mod.rs"]
mod path_dump;

use std::fmt::Write as _;
use std::io::Write as _;
use std::path::Path;
use std::rc::Rc;

use zomboid_engine::java;
use zomboid_engine::zombie::core::random::rand_abstract::RandAbstract;
use zomboid_engine::zombie::core::tile_property_alias_map::TilePropertyAliasMap;
use zomboid_engine::zombie::iso::cell_loader::{self, LoadContext};
use zomboid_engine::zombie::iso::iso_chunk::IsoChunk;
use zomboid_engine::zombie::iso::iso_grid_square::{line_clear, GetSquare, IsoGridSquare};
use zomboid_engine::zombie::iso::iso_lot::IsoLot;
use zomboid_engine::zombie::iso::iso_meta_grid::{AlarmSettings, IsoMetaGrid};
use zomboid_engine::zombie::iso::iso_object::ObjectKind;
use zomboid_engine::zombie::iso::iso_world::{
    self, IsoWorld, TileDefinitions, VANILLA_TILE_DEFINITION_FILES,
};
use zomboid_engine::zombie::iso::sprite::iso_sprite_manager::IsoSpriteManager;
use zomboid_engine::zombie::iso::sprite_details::iso_flag_type::IsoFlagType;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let r = match a.get(1).map(String::as_str) {
        Some("tiledefs") if a.len() == 3 => tiledefs(Path::new(&a[2])),
        Some("fonts") if a.len() == 4 => fonts(Path::new(&a[2])),
        Some("translator") if a.len() == 5 => translator(Path::new(&a[2]), Path::new(&a[4])),
        Some("lotheaders") if a.len() == 4 => lotheaders(Path::new(&a[2]), &a[3]),
        Some("lotpack") if a.len() == 8 => lotpack(Path::new(&a[2]), &a[3], &a[4..8]),
        Some("metagrid") if a.len() == 8 => metagrid(Path::new(&a[2]), &a[3], &a[4..8]),
        Some("chunkobjects") if a.len() == 7 => chunkobjects(Path::new(&a[2]), &a[3..7]),
        Some("collide") if a.len() == 7 => collide(Path::new(&a[2]), &a[3..7]),
        Some("entitysprites") if a.len() == 3 => entitysprites(Path::new(&a[2])),
        Some("items") if a.len() == 4 => items(Path::new(&a[2]), Path::new(&a[3])),
        Some("recipes") if a.len() == 3 => recipes(Path::new(&a[2])),
        Some("vehicles") if a.len() == 3 => vehicles(Path::new(&a[2])),
        Some("xui") if a.len() == 3 => xui(Path::new(&a[2])),
        Some("mods") if a.len() == 5 => mods(Path::new(&a[2]), Path::new(&a[3]), Path::new(&a[4])),
        Some("sounds") if a.len() == 3 => sounds(Path::new(&a[2]), None),
        Some("sounds") if a.len() == 5 => {
            sounds(Path::new(&a[2]), Some((Path::new(&a[3]), Path::new(&a[4]))))
        }
        Some("gos") if a.len() == 5 => gos(Path::new(&a[2]), Path::new(&a[3]), Path::new(&a[4])),
        Some("savemeta") if a.len() == 5 => savemeta(Path::new(&a[3]), Path::new(&a[4])),
        Some("radio") if a.len() == 5 || a.len() == 6 => radio(
            Path::new(&a[2]),
            Path::new(&a[3]),
            Path::new(&a[4]),
            a.get(5).map(Path::new),
        ),
        Some("txtmodels") if a.len() >= 3 => models::txtmodels(Path::new(&a[2]), &a[3..]),
        Some(
            "aiscene" | "xscene" | "xmodelscene" | "animscene" | "fbxscene" | "fbxmodelscene"
            | "fbxanimscene" | "glbscene" | "glbmodelscene" | "glbanimscene",
        ) if a.len() >= 3 => models::aiscene(Path::new(&a[2]), &a[3..]),
        Some("fbxdoc") if a.len() >= 3 => models::fbxdoc(Path::new(&a[2]), &a[3..]),
        Some(
            "xmodels" | "animclips" | "fbxmodels" | "fbxanims" | "glbmodels" | "glbanims"
            | "processedscene",
        ) if a.len() >= 3 => models::processed(Path::new(&a[2]), &a[3..]),
        Some("mapver") if a.len() == 5 => mapver(Path::new(&a[3]), Path::new(&a[4])),
        Some("erosionini") if a.len() == 5 => erosionini(Path::new(&a[3]), Path::new(&a[4])),
        Some("isoregion") if a.len() == 5 => isoregion(Path::new(&a[3]), Path::new(&a[4])),
        Some("visited") if a.len() == 5 => {
            visited(Path::new(&a[2]), Path::new(&a[4]), Path::new(&a[3]))
        }
        Some("scriptparser") if a.len() == 4 => scriptparser(Path::new(&a[3])),
        Some("sandbox") if a.len() == 4 => sandbox(Path::new(&a[2]), Path::new(&a[3])),
        Some("specialcollide") if a.len() == 5 || a.len() == 6 => {
            specialcollide(Path::new(&a[2]), &a[3..], false)
        }
        Some("stairs") if a.len() == 5 => specialcollide_mode(Path::new(&a[2]), &a[3..], 2),
        Some("los") if a.len() == 5 || a.len() == 6 => {
            specialcollide(Path::new(&a[2]), &a[3..], true)
        }
        Some("basements") if a.len() == 7 => basements(Path::new(&a[2]), &a[3..7]),
        Some("worlddictionary") if a.len() == 4 => worlddictionary(Path::new(&a[3])),
        Some("chunksave") if a.len() == 4 => chunksave(Path::new(&a[2]), Path::new(&a[3])),
        // the item and container byte ranges of a save, as the cases of `invbytes` (tools/oracle.sh)
        Some("invcorpus") if a.len() == 4 => invcorpus(Path::new(&a[2]), Path::new(&a[3])),
        Some("mapmeta") if a.len() == 4 => mapmeta(Path::new(&a[2]), Path::new(&a[3])),
        Some("animalzones") if a.len() == 4 => animalzones(Path::new(&a[2]), Path::new(&a[3])),
        Some("gametime") if a.len() == 4 => gametime(Path::new(&a[3])),
        Some("climate") if a.len() >= 4 => climate(Path::new(&a[2]), &a[3..]),
        Some("erosion") if a.len() >= 4 => erosion(Path::new(&a[2]), &a[3..]),
        Some("zombie") if a.len() >= 4 => zombie(Path::new(&a[2]), &a[3..]),
        Some("actioncontext") if a.len() >= 4 => actioncontext(Path::new(&a[2]), &a[3..]),
        Some("body") if a.len() == 4 => body(Path::new(&a[2]), Path::new(&a[3])),
        Some("inventory") if a.len() == 4 => {
            inventory_dump::inventory(Path::new(&a[2]), Path::new(&a[3]))
        }
        // the inventory commands over the item and container bytes of the player's save (tools/oracle.sh invbytes)
        Some("invbytes") if a.len() == 4 => {
            inventory_dump::inventory(Path::new(&a[2]), Path::new(&a[3]))
        }
        Some("zombieai") | Some("zombiehit") if a.len() >= 4 => zombieai(Path::new(&a[2]), &a[3..]),
        Some("pathfind" | "pathgraph" | "pathsearch") if a.len() >= 4 => {
            path_dump::pathfind(Path::new(&a[2]), &a[3..])
        }
        Some("fmodbanks" | "fmodstrings" | "fmodevents" | "fmodsamples") if a.len() >= 5 => {
            fmod_dump::dump(Path::new(&a[2]), &a[4], &a[5..])
        }
        Some("fmodcandidates") if a.len() == 3 => fmod_dump::candidates(Path::new(&a[2])),
        Some("moddata") if a.len() == 4 => moddata(Path::new(&a[3])),
        Some("outfits") if a.len() == 4 => outfits(Path::new(&a[2]), Path::new(&a[3])),
        Some("clothingitems") if a.len() == 3 => clothingitems(Path::new(&a[2])),
        Some("outfitrandom") if a.len() == 3 => outfitrandom(Path::new(&a[2])),
        Some("animsets") if a.len() == 4 => animsets(Path::new(&a[2])),
        Some("luacompile") if a.len() == 4 => luacompile(Path::new(&a[2]), Path::new(&a[3])),
        Some("luavm") if a.len() == 4 => luavm(Path::new(&a[2]), Path::new(&a[3])),
        Some("luastring") if a.len() == 4 => luavm(Path::new(&a[2]), Path::new(&a[3])),
        Some("luaos") if a.len() == 4 => luavm(Path::new(&a[2]), Path::new(&a[3])),
        Some("javachars") if a.len() == 3 => javachars(),
        Some("fdlibm") if a.len() == 3 => fdlibm(),
        Some("wgsimplex") if a.len() == 3 => wgsimplex(),
        Some("wgbiomes") if a.len() == 4 => wgbiomes(Path::new(&a[2]), &a[3]),
        Some("wgutils") if a.len() == 3 => wgutils(),
        Some("wgconfigs") if a.len() == 4 => wgconfigs(Path::new(&a[2]), &a[3]),
        Some("wgvoronoi") if a.len() == 4 => wgvoronoi(Path::new(&a[2]), &a[3]),
        Some("wgveins") if a.len() == 4 => wgveins(Path::new(&a[2]), &a[3]),
        Some("wgroads") if a.len() == 3 => wgroads(Path::new(&a[2])),
        Some("luaload" | "luaserver" | "luaclient") if a.len() >= 4 => {
            luaload(Path::new(&a[2]), &a[3], &a[4..], false)
        }
        Some("luaboot") if a.len() >= 4 => luaload(Path::new(&a[2]), &a[3], &a[4..], true),
        Some("luagame") if a.len() >= 4 => luagame(Path::new(&a[2]), &a[3], &a[4..]),
        Some("loot") if a.len() >= 4 => loot(Path::new(&a[2]), &a[3], &a[4..]),
        Some("mapzone") if a.len() == 4 => {
            mapzone(Path::new(&a[2]), Path::new(&a[3]), false, false)
        }
        Some("zonetriangles") if a.len() == 5 && a[4] == "triangles" => {
            mapzone(Path::new(&a[2]), Path::new(&a[3]), true, true)
        }
        Some("zoneoutlines") if a.len() == 5 && a[4] == "outlines" => {
            mapzone(Path::new(&a[2]), Path::new(&a[3]), true, false)
        }
        _ => Err("usage: see the header of pz-dump.rs".into()),
    };
    match r {
        Ok(text) => {
            if let Err(e) = std::io::stdout().lock().write_all(text.as_bytes()) {
                eprintln!("{e}");
                std::process::exit(1);
            }
        }
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    }
}

/// The meta grid as a loaded save has it (pairs with MapMetaProbe.java): `IsoMetaGrid.Create`, the save's basements
/// from `map_basements.bin`, then `IsoMetaGrid.load` of its `map_meta.bin` as a single-player game reads it.
pub fn mapmeta(root: &Path, save: &Path) -> Result<String, String> {
    use zomboid_engine::java_nio::ByteBuffer;
    use zomboid_engine::zombie::basements::Basements;
    use zomboid_engine::zombie::iso::map_meta::MetaSide;
    let media = root.join("media");
    let mut g = IsoMetaGrid::default();
    let dirs = IsoMetaGrid::lot_directories(&media, "Muldraugh, KY");
    g.create_step1(&media, &dirs, (-250, -250, 250, 250))?;
    for e in g.load_all_cells(&mut |_| 1, &mut |_, _| Vec::new()) {
        eprintln!("{e}");
    }
    let alarm = AlarmSettings {
        last_stand: false,
        power_grid_exists: true,
        alarm: 1,
        alarm_decay: 1,
    };
    g.create_step2(&alarm, &mut |_| 1, &mut |a, _| a);
    let read = |n: &str| {
        let p = save.join(n);
        std::fs::read(&p).map_err(|e| format!("{}: {e}", p.display()))
    };
    let b = Basements::before_load_meta_grid_from_save(
        &media,
        &mut g,
        &dirs,
        &read("map_basements.bin")?,
    )?;
    for l in &b.log {
        eprintln!("{l}");
    }
    let mut sb = String::new();
    let mut h: u64 = 0xcbf29ce484222325;
    for p in &b.basement_placements {
        for v in [p.x, p.y, p.z, p.w, p.h] {
            mix(&mut h, v);
        }
        for u in p.name.encode_utf16() {
            mix(&mut h, i32::from(u));
        }
    }
    let _ = writeln!(sb, "BP {} {h:x}", b.basement_placements.len());
    let mut h: u64 = 0xcbf29ce484222325;
    for bd in &g.buildings {
        mix_long(&mut h, g.building(*bd).id);
        mix_long(&mut h, g.building(*bd).meta_id);
    }
    let _ = writeln!(sb, "B {} {h:x}", g.buildings.len());
    let data = read("map_meta.bin")?;
    let mut bb = ByteBuffer::wrap(&data);
    let int_at = |i: usize| {
        data.get(i..i + 4)
            .map_or(0, |s| i32::from_be_bytes([s[0], s[1], s[2], s[3]]))
    };
    let _ = writeln!(
        sb,
        "MM {} {} {} {} {}",
        int_at(4),
        int_at(8),
        int_at(12),
        int_at(16),
        int_at(20)
    );
    // As MapMetaProbe: the state Create drew from Rand goes back to the field defaults before the load.
    for r in &mut g.room_defs {
        (r.explored, r.lights_active, r.done_spawn, r.roof_fixed) = (false, false, false, false);
    }
    for d in &mut g.building_defs {
        (d.alarmed, d.key_id, d.seen, d.has_been_visited) = (false, -1, false, false);
        (d.loot_respawn_hour, d.alarm_decay) = (-1, 10000000);
    }
    let m = g.load(&mut bb, MetaSide::SinglePlayer)?;
    for l in &m.log {
        eprintln!("{l}");
    }
    let _ = writeln!(sb, "REST {}", bb.remaining());
    let mut t = [0i32; 9];
    for x in g.min_x..=g.max_x {
        for y in g.min_y..=g.max_y {
            let Some(cell) = g.get_cell_data(x, y) else {
                continue;
            };
            let mut detail = String::new();
            let mut h: u64 = 0xcbf29ce484222325;
            for rr in &cell.room_list {
                let r = g.room(*rr);
                let f = i32::from(r.explored)
                    | i32::from(r.lights_active) << 1
                    | i32::from(r.done_spawn) << 2
                    | i32::from(r.roof_fixed) << 3;
                mix_long(&mut h, r.meta_id);
                mix(&mut h, f);
                t[0] += 1;
                for i in 0..4 {
                    if f & 1 << i != 0 {
                        t[2 + i] += 1;
                    }
                }
                if f & !2 != 0 {
                    let _ = writeln!(detail, "MR {} {f}", r.meta_id);
                }
            }
            for bd in &cell.buildings {
                let d = g.building(*bd);
                mix_long(&mut h, d.meta_id);
                for v in [
                    i32::from(d.alarmed),
                    d.key_id,
                    i32::from(d.seen),
                    i32::from(d.has_been_visited),
                    d.loot_respawn_hour,
                    d.alarm_decay,
                ] {
                    mix(&mut h, v);
                }
                t[1] += 1;
                t[6] += i32::from(d.alarmed);
                t[7] += i32::from(d.seen);
                t[8] += i32::from(d.has_been_visited);
                if d.alarmed || d.seen || d.has_been_visited {
                    let _ = writeln!(
                        detail,
                        "MD {} {} {} {} {} {} {}",
                        d.meta_id,
                        u8::from(d.alarmed),
                        d.key_id,
                        u8::from(d.seen),
                        u8::from(d.has_been_visited),
                        d.loot_respawn_hour,
                        d.alarm_decay
                    );
                }
            }
            let _ = writeln!(
                sb,
                "MC {x} {y} {} {} {h:x}",
                cell.room_list.len(),
                cell.buildings.len()
            );
            sb.push_str(&detail);
        }
    }
    let _ = writeln!(
        sb,
        "MT {}",
        t.iter()
            .map(|v| v.to_string())
            .collect::<Vec<_>>()
            .join(" ")
    );
    let _ = writeln!(sb, "SH {}", m.safehouses.len());
    for s in &m.safehouses {
        let _ = writeln!(
            sb,
            "S {} {} {} {} {} {} [{}] {} {} {} {} [{}]",
            s.x,
            s.y,
            s.w,
            s.h,
            s.owner,
            s.hit_points,
            s.players.join(", "),
            s.last_visited,
            s.title,
            s.datetime_created,
            s.location,
            s.players_respawn.join(", ")
        );
    }
    let _ = writeln!(sb, "NP {}", m.non_pvp_zones.len());
    for z in &m.non_pvp_zones {
        let _ = writeln!(
            sb,
            "N {} {} {} {} {} {}",
            z.x, z.y, z.x2, z.y2, z.size, z.title
        );
    }
    let _ = writeln!(sb, "FA {}", m.factions.len());
    for f in &m.factions {
        let _ = writeln!(
            sb,
            "F {} {} {} {} {} {} [{}]",
            f.name,
            f.owner,
            f.tag.as_deref().unwrap_or("null"),
            f.tag_color[0].to_bits() as i32,
            f.tag_color[1].to_bits() as i32,
            f.tag_color[2].to_bits() as i32,
            f.players.join(", ")
        );
    }
    let _ = writeln!(sb, "DZ {}", m.designation_zones.len());
    let st = m.stash_system.unwrap_or_default();
    let _ = writeln!(
        sb,
        "ST {} {} {}",
        st.possible_stashes.len(),
        st.buildings_to_do.len(),
        st.already_read_map.len()
    );
    for s in &st.possible_stashes {
        let _ = writeln!(sb, "SP {} {} {}", s.stash_name, s.building_x, s.building_y);
    }
    for s in &st.buildings_to_do {
        let _ = writeln!(sb, "SB {} {} {}", s.stash_name, s.building_x, s.building_y);
    }
    for s in &st.already_read_map {
        let _ = writeln!(sb, "SR {s}");
    }
    let _ = writeln!(sb, "RDS {}", m.unique_rds_spawned.len());
    for s in &m.unique_rds_spawned {
        let _ = writeln!(sb, "R {s}");
    }
    Ok(sb)
}

/// The loading sequence of IsoWorld.init without mods and without entity scripts, then one line per sprite.
pub fn tiledefs(root: &Path) -> Result<String, String> {
    let mut files = Vec::new();
    for (f, n) in VANILLA_TILE_DEFINITION_FILES {
        let p = root.join("media").join(f);
        files.push((
            f,
            n,
            std::fs::read(&p).map_err(|e| format!("{}: {e}", p.display()))?,
        ));
    }
    let mut defs = TileDefinitions::default();
    let mut alias = TilePropertyAliasMap::default();
    let mut sm = IsoSpriteManager::new();
    for (f, n, data) in &files {
        if let Err(e) = defs.load_tile_definitions_property_strings(data, f, *n) {
            eprintln!("PROBE strings {f}: {e}");
        }
    }
    defs.set_custom_property_values()?;
    defs.generate_tile_property_lookup_tables(&mut alias, &[]);
    for (f, n, data) in &files {
        if let Err(e) = iso_world::load_tile_definitions(&mut sm, &alias, data, f, *n) {
            eprintln!("PROBE load {f}: {e}");
        }
    }
    iso_world::register_fake_jumbo_tree(&mut sm, &alias, 7);

    let mut names: Vec<&String> = sm.named_map.keys().collect();
    names.sort_unstable();
    let mut sb = String::new();
    let null = |o: Option<&str>| o.unwrap_or("null").to_string();
    for n in &names {
        let r = sm.named_map[*n];
        let s = sm.sprite(r);
        let p = &s.properties;
        let flags: Vec<&str> = p.get_flags_list().iter().map(|f| f.name()).collect();
        let _ = write!(
            sb,
            "{n}\t{}\t{}\t{}\t",
            s.id,
            s.tile_type.name(),
            flags.join(",")
        );
        for k in p.get_property_names(&alias) {
            let _ = write!(sb, "{k}={};", null(p.get(&alias, k)));
        }
        sb.push('\t');
        for x in [
            s.force_ambient,
            s.solidfloor,
            s.can_be_removed,
            s.attached_floor,
            s.cut_w,
            s.cut_n,
            s.solid,
            s.solid_trans,
            s.invisible,
            s.always_draw,
            s.force_render,
            s.move_with_wind,
            s.is_bush,
            s.treat_as_wall_order,
        ] {
            sb.push(if x { '1' } else { '0' });
        }
        let _ = write!(
            sb,
            "\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            s.firerequirement,
            null(s.burnt_tile.as_deref()),
            s.wind_type,
            s.render_layer,
            null(s.tileset_name.as_deref()),
            s.tile_sheet_index,
            null(s.parent_object_name.as_deref()),
            s.fascia_edge.map_or("null", |f| f.name())
        );
        match s.sprite_grid {
            None => sb.push_str("\t-"),
            Some(g) => {
                let g = sm.grid(g);
                let anchor = g
                    .get_anchor_sprite()
                    .map(|a| null(sm.sprite(a).name.as_deref()))
                    .unwrap_or("null".into());
                let _ = write!(
                    sb,
                    "\t{}x{}x{}@{}:{}",
                    g.get_width(),
                    g.get_height(),
                    g.get_levels(),
                    g.get_sprite_index(r).map_or(-1, |i| i as i32),
                    anchor
                );
            }
        }
        let u = p.init_surface(&alias);
        let b = |x: bool| if x { 1 } else { 0 };
        let _ = writeln!(
            sb,
            "\t{},{}{}{},{},{},{},{},{}",
            u.surface,
            b(u.is_surface_offset),
            b(u.is_table),
            b(u.is_table_top),
            u.stack_replace_tile_offset,
            u.item_height,
            u.sloped_surface_direction.map_or("null", |d| d.name()),
            u.sloped_surface_height_min,
            u.sloped_surface_height_max
        );
    }
    eprintln!("PROBE sprites={} intMap={}", names.len(), sm.int_map.len());
    Ok(sb)
}

/// `pz-dump fonts <root> <scratch>` (pairs with FontsProbe.java): every `.fnt` under `media/fonts` and in `media/`
/// through the ported `AngelCodeFont`, then the `TextManager` tables for every language, font size option and dyslexic
/// flag. The scratch argument is the probe's cache folder, unused here.
pub fn fonts(root: &Path) -> Result<String, String> {
    use std::rc::Rc;
    use zomboid_engine::zombie::core::core::get_option_font_size_real;
    use zomboid_engine::zombie::core::fonts::angel_code_font::AngelCodeFont;
    use zomboid_engine::zombie::ui::fonts_file;
    use zomboid_engine::zombie::ui::text_manager::{FontRef, TextManager};
    use zomboid_engine::zombie::ui::ui_font::UIFont;
    use zomboid_engine::zombie::zomboid_file_system::ZomboidFileSystem;

    const LONG: &str = "This is a long sentence that should wrap across several lines when the width is small enough, with averyveryverylongwordthatcannotfitonasingleline inside.";
    const SAMPLES: [&str; 19] = [
        "",
        "Hello, World!",
        "The quick brown fox jumps over the lazy dog. 0123456789",
        "Line one\nLine two\n\nfourth line",
        "\n",
        "AV AW Ta To Wa Yo ff fi LT",
        "  two  spaces  ",
        "Tab\there\u{0}nul",
        "a\r\nb\rc",
        "\u{dc}ber na\u{ef}ve caf\u{e9} \u{e0}\u{e9}\u{ee}\u{f5}\u{fc} \u{df} \u{a9}\u{ae}\u{20ac}\u{2122}",
        "\u{41f}\u{440}\u{438}\u{432}\u{435}\u{442}, \u{43c}\u{438}\u{440}! \u{401}\u{451}",
        "\u{4f60}\u{597d}\u{4e16}\u{754c}\u{ff0c}\u{6d4b}\u{8bd5}\u{3002}",
        "\u{3053}\u{3093}\u{306b}\u{3061}\u{306f}\u{3001}\u{30ab}\u{30bf}\u{30ab}\u{30ca}\u{6f22}\u{5b57}",
        "\u{c548}\u{b155}\u{d558}\u{c138}\u{c694} \u{d55c}\u{ad6d}\u{c5b4}",
        "\u{e2a}\u{e27}\u{e31}\u{e2a}\u{e14}\u{e35}\u{e04}\u{e23}\u{e31}\u{e1a} \u{e01}\u{e48}\u{e33}",
        "\u{1}\u{7f}\u{fffd}\u{ffff}\u{1f600}",
        "<RGB:1,0,0> Red <LINE> <SIZE:large> [img=music] <IMAGE:media/ui/x.png>",
        "Za\u{17c}\u{f3}\u{142}\u{107} g\u{119}\u{15b}l\u{105} ja\u{17a}\u{144}",
        LONG,
    ];
    const INIT_SAMPLES: [usize; 6] = [0, 1, 3, 10, 11, 15];
    const CJK4: &str = "\u{4f60}\u{597d}\u{4e16}\u{754c}\u{4f60}\u{597d}\u{4e16}\u{754c}\u{4f60}\u{597d}\u{4e16}\u{754c}\u{4f60}\u{597d}\u{4e16}\u{754c}";
    const WRAPS: [(&str, i32, i32, &str); 16] = [
        (LONG, 200, -1, ""),
        (LONG, 50, -1, ""),
        (LONG, 50, 2, "..."),
        (LONG, 120, 3, "\u{2026}"),
        (
            "first line\r\nsecond line is longer than the first\nthird",
            80,
            -1,
            "",
        ),
        ("", 100, -1, ""),
        ("   ", 0, -1, ""),
        ("short", 1000, 1, "..."),
        (CJK4, 60, -1, ""),
        ("word  double  spaces here", 40, -1, ""),
        ("\n\nlead", 100, -1, ""),
        ("trail\n\n", 100, -1, ""),
        (" x", -1, -1, ""),
        (LONG, 90, 2, "a much longer suffix than the room"),
        (
            "Supercalifragilisticexpialidociousandsomemore end",
            60,
            -1,
            "",
        ),
        ("Antidisestablishmentarianism\r\n\r\nx", 25, 4, "~"),
    ];
    const WRAP_FONTS: [UIFont; 5] = [
        UIFont::Small,
        UIFont::Medium,
        UIFont::Large,
        UIFont::Code,
        UIFont::Dialogue,
    ];
    const HEIGHTS: [i32; 29] = [
        0, 720, 1079, 1080, 1211, 1212, 1343, 1344, 1475, 1476, 1607, 1608, 1739, 1740, 1871, 1872,
        2003, 2004, 2135, 2136, 2267, 2268, 2399, 2400, 2531, 2532, 3000, 4320, -5000,
    ];
    let u16s = |s: &str| -> Vec<u16> { s.encode_utf16().collect() };
    let esc16 = |s: &[u16]| -> String {
        let mut b = String::new();
        for &c in s {
            if (0x20..0x7f).contains(&c) && c != u16::from(b'\\') {
                b.push(c as u8 as char);
            } else {
                let _ = write!(b, "\\u{c:04x}");
            }
        }
        b
    };
    let esc = |s: Option<&str>| -> String {
        match s {
            None => "null".into(),
            Some(s) => esc16(&u16s(s)),
        }
    };
    // FontsProbe.exc: the simple name of the exception's class.
    let exc = |e: &str| -> String {
        let class = e.split(':').next().unwrap_or(e);
        format!("E:{}", class.rsplit('.').next().unwrap_or(class))
    };
    let walk = |dir: &Path| -> Vec<std::path::PathBuf> {
        let mut out = Vec::new();
        let mut stack = vec![dir.to_path_buf()];
        while let Some(d) = stack.pop() {
            out.push(d.clone());
            if d.is_dir() {
                if let Ok(rd) = std::fs::read_dir(&d) {
                    for e in rd.flatten() {
                        stack.push(e.path());
                    }
                }
            }
        }
        out
    };
    // Files.walk(Paths.get("media/...")) as the probe sees it from the game folder: paths relative to the root.
    let rel_to_root = |p: &Path| -> String {
        p.strip_prefix(root)
            .unwrap_or(p)
            .to_string_lossy()
            .replace('\\', "/")
    };

    let fs = ZomboidFileSystem::init(root)?;
    let base = format!("{}/", fs.base().to_string_lossy());
    let rel = |abs: &str| -> String {
        match abs.strip_prefix(&base) {
            Some(r) => format!("$/{r}"),
            None => abs.to_string(),
        }
    };
    let mut sb = String::new();

    // Part A: every font file.
    let mut files: Vec<String> = Vec::new();
    for p in walk(&root.join("media/fonts")) {
        let s = rel_to_root(&p);
        if s.ends_with(".fnt") && p.is_file() {
            files.push(s);
        }
    }
    if let Ok(rd) = std::fs::read_dir(root.join("media")) {
        for e in rd.flatten() {
            let s = rel_to_root(&e.path());
            if s.ends_with(".fnt") && e.path().is_file() {
                files.push(s);
            }
        }
    }
    files.sort_by(|a, b| java::string_compare_to(a, b));
    let _ = writeln!(sb, "FONTS {}", files.len());
    for path in &files {
        let _ = writeln!(sb, "FONT {path}");
        let f = match AngelCodeFont::new(&fs, path, None) {
            Ok(f) => f,
            Err(e) => {
                let _ = writeln!(sb, "FAIL {}", exc(&e));
                continue;
            }
        };
        let defined = f.chars.iter().flatten().count();
        // pages: the loaded page textures; none without a renderer.
        let _ = writeln!(
            sb,
            "FILE {} sdf={} lh={} chars={} defined={defined} pages=0",
            rel(f.fnt_file.as_deref().unwrap_or("null")),
            u8::from(f.is_sdf()),
            f.get_line_height(),
            f.chars.len()
        );
        for t in f.texture_requests() {
            let _ = writeln!(sb, "TEX {}", esc(Some(&rel(t))));
        }
        for d in f.chars.iter().flatten() {
            let _ = write!(
                sb,
                "C {} {} {} {} {} {} {} {} {}",
                d.id, d.x, d.y, d.width, d.height, d.xoffset, d.yoffset, d.xadvance, d.page
            );
            if let (Some(second), Some(amount)) = (&d.kerning_second, &d.kerning_amount) {
                sb.push_str(" K");
                for (s, a) in second.iter().zip(amount) {
                    let _ = write!(sb, " {s}:{a}");
                }
            }
            sb.push('\n');
        }
        for (s, sample) in SAMPLES.iter().enumerate() {
            let t = u16s(sample);
            let _ = write!(
                sb,
                "M {s} {} {} {} {} {} ",
                f.get_width(&t),
                f.get_width_xadvance(&t, true),
                f.get_height(&t),
                f.get_height_ext(&t, true, false),
                f.get_height_ext(&t, false, true)
            );
            match f.get_y_offset(&t) {
                Ok(y) => {
                    let _ = write!(sb, "{y}");
                }
                Err(e) => sb.push_str(&exc(&e)),
            }
            if t.len() >= 3 {
                let end = t.len() as i32 - 2;
                let _ = write!(
                    sb,
                    " {} {}",
                    f.get_width_range(&t, 1, end, false)?,
                    f.get_width_range(&t, 1, end, true)?
                );
            }
            sb.push('\n');
        }
    }

    // Part B: the TextManager tables.
    for o in 1..=6 {
        for h in HEIGHTS {
            let _ = writeln!(sb, "FSR {o} {h} {}", get_option_font_size_real(o, h));
        }
    }
    // Languages.init: EN, then every folder of media/lua/shared/Translate with a language.json (named after it).
    let mut langs: Vec<String> = vec!["EN".into()];
    if let Ok(rd) = std::fs::read_dir(root.join("media/lua/shared/Translate")) {
        for e in rd.flatten() {
            if e.path().is_dir() && e.path().join("language.json").exists() {
                langs.push(e.file_name().to_string_lossy().into_owned());
            }
        }
    }
    langs.sort_by(|a, b| java::string_compare_to(a, b));
    langs.dedup();
    let _ = writeln!(sb, "LANG {} {}", langs.len(), langs.join(" "));

    let mut names: Vec<String> = Vec::new();
    let mut fonts_files: Vec<String> = Vec::new();
    for p in walk(&root.join("media/fonts")) {
        let name = p
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        if name.len() >= 9 && name.starts_with("fonts") && name.ends_with(".txt") {
            fonts_files.push(rel_to_root(&p));
        }
    }
    fonts_files.sort_by(|a, b| java::string_compare_to(a, b));
    let mut log = Vec::new();
    for ff in &fonts_files {
        let mut m = std::collections::HashMap::new();
        let ok = fonts_file::read(&fs, fs.get_string_or(ff), &mut m, &mut log);
        let _ = writeln!(sb, "FONTSFILE {ff} {ok} {}", m.len());
        let mut keys: Vec<&String> = m.keys().collect();
        keys.sort_by(|a, b| java::string_compare_to(a, b));
        for k in keys {
            let v = &m[k];
            let _ = writeln!(
                sb,
                "FF {} id={} fnt={} img={} type={}",
                esc(Some(k)),
                esc(Some(&v.id)),
                esc(Some(&v.fnt)),
                esc(v.img.as_deref()),
                esc(v.r#type.as_deref())
            );
            names.push(v.fnt.clone());
            if let Some(img) = &v.img {
                names.push(img.clone());
            }
        }
    }
    names.sort_by(|a, b| java::string_compare_to(a, b));
    names.dedup();
    let size_dirs = [None, Some("1x"), Some("2x"), Some("3x"), Some("4x")];
    for l in &langs {
        for sd in size_dirs {
            for n in &names {
                let _ = writeln!(
                    sb,
                    "PATH {l} {} {n} {}",
                    sd.unwrap_or("null"),
                    TextManager::get_font_file_path(&fs, l, sd, n)
                );
            }
        }
    }

    let fonts = UIFont::values();
    let font_line = |f: &FontRef| -> String {
        let f = f.borrow();
        format!(
            "{} sdf={} lh={} chars={}",
            rel(f.fnt_file.as_deref().unwrap_or("null")),
            u8::from(f.is_sdf()),
            f.get_line_height(),
            f.chars.len()
        )
    };
    for l in &langs {
        for o in 1..=5 {
            for dys in 0..=1 {
                let mut tm = TextManager::new();
                let _ = write!(sb, "INIT {l} size={o} dyslexic={dys}");
                match tm.init(&fs, l, dys == 1, get_option_font_size_real(o, 720)) {
                    Ok(()) => sb.push_str(" ok\n"),
                    Err(e) => {
                        let _ = writeln!(sb, " {}", exc(&e));
                    }
                }
                let mut distinct: Vec<FontRef> = Vec::new();
                let mut id = |f: Option<&FontRef>| -> String {
                    let Some(f) = f else {
                        return "null".into();
                    };
                    if let Some(i) = distinct.iter().position(|d| Rc::ptr_eq(d, f)) {
                        return format!("#{i}");
                    }
                    distinct.push(f.clone());
                    format!("#{} {}", distinct.len() - 1, font_line(f))
                };
                for u in fonts {
                    let _ = writeln!(
                        sb,
                        "U {} {}",
                        u.name(),
                        id(tm.enum_to_font[u.ordinal()].as_ref())
                    );
                }
                for n in 0..tm.normal.len() as i32 {
                    let f = tm.get_normal_from_font_size(n + 11)?;
                    let _ = writeln!(sb, "N {} {}", n + 11, id(f.as_ref()));
                }
                let fields = [
                    &tm.font,
                    &tm.font2,
                    &tm.font3,
                    &tm.font4,
                    &tm.main1,
                    &tm.main2,
                    &tm.zombiefontcredits1,
                    &tm.zombiefontcredits2,
                    &tm.zombienew1,
                    &tm.zombienew2,
                    &tm.zombienew3,
                    &tm.zomboid_dialogue,
                    &tm.codetext,
                    &tm.code_small,
                    &tm.code_medium,
                    &tm.code_large,
                    &tm.debug_console,
                    &tm.intro,
                    &tm.handwritten,
                ];
                sb.push_str("FIELDS");
                for f in fields {
                    let _ = write!(sb, " {}", id(f.as_ref()));
                }
                sb.push('\n');
                let _ = writeln!(
                    sb,
                    "NONEN {} code={}",
                    TextManager::is_using_non_english_fonts(&fs, l),
                    tm.get_current_code_font().name()
                );
                let with_null: Vec<Option<UIFont>> =
                    fonts.iter().map(|f| Some(*f)).chain([None]).collect();
                for u in with_null {
                    let name = u.map_or("null", |f| f.name());
                    let _ = write!(sb, "H {name}");
                    for r in [tm.get_font_height(u), tm.measure_font(u)] {
                        match r {
                            Ok(v) => {
                                let _ = write!(sb, " {v}");
                            }
                            Err(e) => {
                                let _ = write!(sb, " {}", exc(&e));
                            }
                        }
                    }
                    match tm.is_sdf(u) {
                        Ok(v) => {
                            let _ = write!(sb, " {v}");
                        }
                        Err(e) => {
                            let _ = write!(sb, " {}", exc(&e));
                        }
                    }
                    sb.push('\n');
                    for s in INIT_SAMPLES {
                        let t = u16s(SAMPLES[s]);
                        let t = Some(t.as_slice());
                        let _ = writeln!(
                            sb,
                            "MS {name} {s} {} {} {} {} {}",
                            tm.measure_string_x(u, t)?,
                            tm.measure_string_y(u, t)?,
                            tm.measure_string_y_real(u, t)?,
                            tm.measure_string_y_offset(u, t)?,
                            tm.centre_string_y_offset(u, t)?
                        );
                    }
                }
                for u in WRAP_FONTS {
                    for (w, (text, width, lines, suffix)) in WRAPS.iter().enumerate() {
                        let _ = write!(sb, "WRAP {} {w} ", u.name());
                        match tm.wrap_text_ext(Some(u), &u16s(text), *width, *lines, &u16s(suffix))
                        {
                            Ok(r) => sb.push_str(&esc16(&r)),
                            Err(e) => sb.push_str(&exc(&e)),
                        }
                        sb.push('\n');
                    }
                    let r = tm.wrap_text(Some(u), &u16s(LONG), 150)?;
                    let _ = writeln!(sb, "WRAP2 {} {}", u.name(), esc16(&r));
                }
            }
        }
    }
    sb.push_str("ALLFONTS");
    for u in TextManager::new().get_all_fonts() {
        let _ = write!(sb, " {}", u.name());
    }
    sb.push('\n');
    for s in ["Small", "small", "SdfCaveat", "", "Nope", "DebugConsole"] {
        let _ = writeln!(
            sb,
            "FROMSTRING {} {}",
            esc(Some(s)),
            UIFont::from_string(s).map_or("null", |f| f.name())
        );
    }
    Ok(sb)
}

const NO_WORLD_GEN: (i32, i32, i32, i32) = (10000000, 10000000, -10000000, -10000000);

fn mix(h: &mut u64, v: i32) {
    for b in v.to_le_bytes() {
        *h ^= b as u64;
        *h = h.wrapping_mul(0x100000001b3);
    }
}
fn mix_long(h: &mut u64, v: i64) {
    mix(h, v as i32);
    mix(h, (v >> 32) as i32);
}

/// Every lotheader of one map folder through the port of MetaGridLoaderThread.loadCell, cells in x-then-y order.
/// `lightsActive` is random in the original and not printed.
pub fn lotheaders(root: &Path, dir: &str) -> Result<String, String> {
    let media = root.join("media");
    let mut g = IsoMetaGrid::default();
    g.create_step1(&media, &[dir.to_string()], NO_WORLD_GEN)?;
    let mut sb = String::new();
    {
        let mf = &g.map_files[0];
        let _ = writeln!(
            sb,
            "M {} {} {} {} {} {} {} {}",
            mf.min_x,
            mf.min_y,
            mf.max_x,
            mf.max_y,
            mf.min_cell300_x,
            mf.min_cell300_y,
            mf.max_cell300_x,
            mf.max_cell300_y
        );
        for y in mf.min_y..=mf.max_y {
            for x in mf.min_x..=mf.max_x {
                sb.push(if mf.has_cell(x, y) { '1' } else { '0' });
            }
            sb.push('\n');
        }
        for y in mf.min_cell300_y..=mf.max_cell300_y {
            for x in mf.min_cell300_x..=mf.max_cell300_x {
                sb.push(if mf.has_cell300(x, y) { '1' } else { '0' });
            }
            sb.push('\n');
        }
    }
    let (x1, y1, x2, y2) = (
        g.map_files[0].min_x,
        g.map_files[0].min_y,
        g.map_files[0].max_x,
        g.map_files[0].max_y,
    );
    for x in x1..=x2 {
        for y in y1..=y2 {
            if !g.map_files[0].has_cell(x, y) {
                continue;
            }
            let path = g.map_files[0]
                .get_lot_header(x, y)
                .ok_or("header missing")?
                .absolute_file_path
                .clone();
            let data = std::fs::read(&path).map_err(|e| e.to_string())?;
            if let Err(e) = g.load_cell(0, &data, x, y, &mut |_| 1, &[]) {
                eprintln!("ERROR loading {}: {e}", path.display());
            }
            let h = g.map_files[0]
                .get_lot_header(x, y)
                .ok_or("header missing")?;
            let c = g.get_cell_data(x, y).ok_or("cell missing")?;
            let _ = writeln!(
                sb,
                "C {x} {y} {} {} {} {} {} {} {} {} {}",
                h.version,
                h.width,
                h.height,
                h.min_level,
                h.max_level,
                h.fixed2x,
                h.tiles_used.len(),
                c.room_list.len(),
                c.buildings.len()
            );
            for t in &h.tiles_used {
                let _ = writeln!(sb, "T {t}");
            }
            for r in &c.room_list {
                let r = g.room(*r);
                let _ = write!(
                    sb,
                    "R {} {} {} {} {} {} {} {} {} [{}]",
                    r.id,
                    r.meta_id,
                    r.level,
                    r.x,
                    r.y,
                    r.x2,
                    r.y2,
                    r.area,
                    r.building.map_or(-1, |b| g.building(b).id),
                    r.name
                );
                for q in &r.rects {
                    let _ = write!(sb, " {},{},{},{}", q.x, q.y, q.w, q.h);
                }
                sb.push_str(" |");
                for o in &r.objects {
                    let _ = write!(sb, " {},{},{}", o.r#type, o.x, o.y);
                }
                sb.push('\n');
            }
            for b in &c.buildings {
                let b = g.building(*b);
                let _ = write!(
                    sb,
                    "B {} {} {} {} {} {} r",
                    b.id, b.meta_id, b.x, b.y, b.x2, b.y2
                );
                for r in &b.rooms {
                    let _ = write!(sb, " {}", g.room(*r).id);
                }
                sb.push_str(" e");
                for r in &b.emptyoutside {
                    let _ = write!(sb, " {}", g.room(*r).id);
                }
                sb.push_str(" c");
                for v in &b.overlapped_chunks {
                    let _ = write!(sb, " {v}");
                }
                sb.push('\n');
            }
            sb.push_str("Z ");
            for i in 0..1024 {
                let _ = write!(sb, "{:02x}", h.get_zombie_intensity(i));
            }
            sb.push('\n');
        }
    }
    Ok(sb)
}

/// Every chunk of a cell range through the port of IsoLot.load; pairs with LotPackProbe.java.
pub fn lotpack(root: &Path, dir: &str, range: &[String]) -> Result<String, String> {
    let r: Vec<i32> = range
        .iter()
        .map(|s| s.parse::<i32>().map_err(|e| e.to_string()))
        .collect::<Result<_, _>>()?;
    let media = root.join("media");
    let mut g = IsoMetaGrid::default();
    g.create_step1(&media, &[dir.to_string()], NO_WORLD_GEN)?;
    let mut sb = String::new();
    let mut lot = IsoLot::default();
    for cx in r[0]..=r[2] {
        for cy in r[1]..=r[3] {
            if !g.map_files[0].has_cell(cx, cy) {
                continue;
            }
            let path = g.map_files[0]
                .get_lot_header(cx, cy)
                .ok_or("header missing")?
                .absolute_file_path
                .clone();
            let data = std::fs::read(&path).map_err(|e| e.to_string())?;
            g.load_cell(0, &data, cx, cy, &mut |_| 1, &[])?;
            let info = g.map_files[0]
                .get_lot_header(cx, cy)
                .ok_or("header missing")?;
            let pack_path = g.map_files[0]
                .info_file_names
                .get(&format!("world_{cx}_{cy}.lotpack"))
                .ok_or("no lotpack")?;
            let pack = std::fs::read(pack_path).map_err(|e| e.to_string())?;
            let mut h: u64 = 0xcbf29ce484222325;
            let n = ((info.max_level.min(31) - info.min_level.max(-32) + 1) * 64) as usize;
            let base = ((info.min_level.max(-32) - info.min_level) * 64) as usize;
            for lx in 0..32 {
                for ly in 0..32 {
                    if let Some(e) = lot.load(info, &pack, cx, cy, cx * 32 + lx, cy * 32 + ly) {
                        eprintln!("ERROR {cx},{cy} chunk {lx},{ly}: {e}");
                    }
                    mix(&mut h, lot.min_level);
                    mix(&mut h, lot.max_level);
                    mix(&mut h, lot.data.len() as i32);
                    for v in &lot.offset_in_data[base..base + n] {
                        mix(&mut h, *v);
                    }
                    for v in &lot.data {
                        mix(&mut h, *v);
                    }
                    if lx == 16 && ly == 16 {
                        let _ = write!(sb, "D {cx} {cy} {} {}", lot.min_level, lot.max_level);
                        for v in &lot.offset_in_data[base..base + n] {
                            let _ = write!(sb, " {v}");
                        }
                        sb.push_str(" |");
                        for v in &lot.data {
                            let _ = write!(sb, " {v}");
                        }
                        sb.push('\n');
                    }
                }
            }
            let _ = writeln!(sb, "L {cx} {cy} {h:x}");
        }
    }
    Ok(sb)
}

/// The whole meta grid as IsoMetaGrid.Create builds it for `Core.gameMap`, then rooms per chunk and room lookups per
/// square for a cell range; pairs with MetaGridProbe.java (run inside the booted original).
pub fn metagrid(root: &Path, game_map: &str, range: &[String]) -> Result<String, String> {
    let r: Vec<i32> = range
        .iter()
        .map(|s| s.parse::<i32>().map_err(|e| e.to_string()))
        .collect::<Result<_, _>>()?;
    let media = root.join("media");
    let mut g = IsoMetaGrid::default();
    let dirs = IsoMetaGrid::lot_directories(&media, game_map);
    g.create_step1(&media, &dirs, (-250, -250, 250, 250))?;
    for e in g.load_all_cells(&mut |_| 1, &mut |_, _| Vec::new()) {
        eprintln!("{e}");
    }
    let alarm = AlarmSettings {
        last_stand: false,
        power_grid_exists: true,
        alarm: 1,
        alarm_decay: 1,
    };
    g.create_step2(&alarm, &mut |_| 1, &mut |a, _| a);
    let mut sb = String::new();
    let _ = writeln!(
        sb,
        "G {} {} {} {} dirs={}",
        g.min_x,
        g.min_y,
        g.max_x,
        g.max_y,
        dirs.join(";")
    );
    let mut h: u64 = 0xcbf29ce484222325;
    for b in &g.buildings {
        mix_long(&mut h, g.building(*b).id);
    }
    let _ = writeln!(sb, "B {} {h:x}", g.buildings.len());
    for cx in r[0]..=r[2] {
        for cy in r[1]..=r[3] {
            let Some(info) = g.get_header(cx, cy) else {
                continue;
            };
            let adj: String = info
                .adjacent_cells
                .iter()
                .map(|b| if *b { '1' } else { '0' })
                .collect();
            let _ = writeln!(sb, "A {cx} {cy} {adj}");
            let cell = g.get_cell_data(cx, cy).ok_or("cell missing")?;
            for ly in 0..32 {
                for lx in 0..32 {
                    if let Some(ch) = cell.chunk((ly * 32 + lx) as usize) {
                        if ch.get_rooms_size() > 0 {
                            let _ = write!(sb, "K {cx} {cy} {lx} {ly}");
                            for rr in ch.rooms() {
                                let _ = write!(sb, " {}", g.room(*rr).id);
                            }
                            sb.push('\n');
                        }
                    }
                }
            }
            let mut h: u64 = 0xcbf29ce484222325;
            for z in info.min_level..=info.max_level {
                for y in cy * 256..cy * 256 + 256 {
                    for x in cx * 256..cx * 256 + 256 {
                        mix_long(
                            &mut h,
                            g.get_room_at(x, y, z).map_or(-1, |rr| g.room(rr).id),
                        );
                        mix_long(
                            &mut h,
                            g.get_empty_outside_at(x, y, z)
                                .map_or(-1, |rr| g.room(rr).id),
                        );
                    }
                }
            }
            let _ = writeln!(sb, "H {cx} {cy} {h:x}");
        }
    }
    Ok(sb)
}

/// Tile definitions as IsoWorld.init loads them (vanilla files, no mods, no entity scripts).
fn load_tile_definitions(
    root: &Path,
) -> Result<(IsoSpriteManager, TilePropertyAliasMap, TileDefinitions), String> {
    let mut files = Vec::new();
    for (f, n) in VANILLA_TILE_DEFINITION_FILES {
        let p = root.join("media").join(f);
        files.push((
            f,
            n,
            std::fs::read(&p).map_err(|e| format!("{}: {e}", p.display()))?,
        ));
    }
    let mut defs = TileDefinitions::default();
    let mut alias = TilePropertyAliasMap::default();
    let mut sm = IsoSpriteManager::new();
    for (f, n, data) in &files {
        if let Err(e) = defs.load_tile_definitions_property_strings(data, f, *n) {
            eprintln!("PROBE strings {f}: {e}");
        }
    }
    defs.set_custom_property_values()?;
    defs.generate_tile_property_lookup_tables(&mut alias, &[]);
    for (f, n, data) in &files {
        if let Err(e) = iso_world::load_tile_definitions(&mut sm, &alias, data, f, *n) {
            eprintln!("PROBE load {f}: {e}");
        }
    }
    iso_world::register_fake_jumbo_tree(&mut sm, &alias, 7);
    Ok((sm, alias, defs))
}

/// Squares and objects of a chunk range through the port of CellLoader.LoadCellBinaryChunk; pairs with
/// ChunkObjectsProbe.java (run inside the booted original).
pub fn chunkobjects(root: &Path, range: &[String]) -> Result<String, String> {
    let r: Vec<i32> = range
        .iter()
        .map(|s| s.parse::<i32>().map_err(|e| e.to_string()))
        .collect::<Result<_, _>>()?;
    let media = root.join("media");
    let (mut sm, alias, mut defs) = load_tile_definitions(root)?;
    sm.add_sprite("media/ui/missing-tile.png");
    defs.loaded_tile_definitions(&mut sm, &alias, &|_| false)?;
    let mut g = IsoMetaGrid::default();
    let dirs = IsoMetaGrid::lot_directories(&media, "Muldraugh, KY");
    g.create_step1(&media, &dirs, (-250, -250, 250, 250))?;
    for e in g.load_all_cells(&mut |_| 1, &mut |_, _| Vec::new()) {
        eprintln!("{e}");
    }
    let alarm = AlarmSettings {
        last_stand: false,
        power_grid_exists: true,
        alarm: 1,
        alarm_decay: 1,
    };
    g.create_step2(&alarm, &mut |_| 1, &mut |a, _| a);
    let mut rand = RandAbstract::new(java::Random::new(12345));
    // LockedHouses is 6 in the dedicated server's default sandbox preset, which the probe runs under.
    let mut ctx = LoadContext {
        sprites: &mut sm,
        alias: &alias,
        glass_removed_window_sprite_map: &defs.glass_removed_window_sprite_map,
        smashed_window_sprite_map: &defs.smashed_window_sprite_map,
        meta_grid: &mut g,
        rand: &mut rand,
        locked_houses: 6,
        missing_tiles: Vec::new(),
        lotpack_cache: None,
    };
    let mut sb = String::new();
    let _ = writeln!(sb, "O lockedHouses={}", ctx.locked_houses);
    let b = |v: bool| if v { 1 } else { 0 };
    for wx in r[0]..=r[2] {
        for wy in r[1]..=r[3] {
            *ctx.rand = RandAbstract::new(java::Random::new(12345));
            let mut ch = IsoChunk::new(wx, wy);
            let mut errors = Vec::new();
            let ok = cell_loader::load_cell_binary_chunk(&mut ctx, wx, wy, &mut ch, &mut errors);
            for e in errors {
                eprintln!("chunk {wx},{wy}: {e}");
            }
            let mut taint = false;
            for z in ch.get_min_level()..=ch.get_max_level() {
                for i in 0..64 {
                    if let Some(sq) = ch.get_grid_square(i % 8, i / 8, z) {
                        taint |= sq.objects.iter().any(|o| {
                            matches!(o.kind, ObjectKind::IsoRadio | ObjectKind::IsoTelevision)
                        });
                    }
                }
            }
            let _ = writeln!(
                sb,
                "C {wx} {wy} {ok} {} {}",
                ch.get_min_level(),
                ch.get_max_level()
            );
            for z in ch.get_min_level()..=ch.get_max_level() {
                for y in 0..8 {
                    for x in 0..8 {
                        let Some(sq) = ch.get_grid_square(x, y, z) else {
                            continue;
                        };
                        let _ = write!(sb, "S {x} {y} {z} {}", sq.room_id);
                        let name = |s: Option<_>| {
                            s.and_then(|s| ctx.sprites.sprite(s).name.clone())
                                .unwrap_or_else(|| "null".to_string())
                        };
                        for o in &sq.objects {
                            let _ = write!(sb, " {}:{}", o.kind.class_name(), name(o.sprite));
                            for at in o.attached_anim_sprite.iter().flatten() {
                                let _ = write!(sb, "+{}", name(Some(*at)));
                            }
                            if o.special {
                                sb.push('*');
                            }
                            if o.render_y_offset != 0.0 {
                                let _ = write!(sb, "@{:x}", o.render_y_offset.to_bits());
                            }
                            match &o.kind {
                                ObjectKind::IsoDoor(d) if taint => {
                                    let _ = write!(sb, "{{o{}n{}?}}", b(d.open), b(d.north));
                                }
                                ObjectKind::IsoDoor(d) => {
                                    let _ = write!(
                                        sb,
                                        "{{o{}n{}l{}k{}}}",
                                        b(d.open),
                                        b(d.north),
                                        b(d.locked),
                                        b(d.locked_by_key)
                                    );
                                }
                                ObjectKind::IsoWindow(w) if taint => {
                                    let _ = write!(
                                        sb,
                                        "{{n{}?p{}s{}}}",
                                        b(w.north),
                                        b(w.perma_locked),
                                        b(w.smashed)
                                    );
                                }
                                ObjectKind::IsoWindow(w) => {
                                    let _ = write!(
                                        sb,
                                        "{{n{}l{}p{}s{}}}",
                                        b(w.north),
                                        b(w.locked),
                                        b(w.perma_locked),
                                        b(w.smashed)
                                    );
                                }
                                ObjectKind::IsoTree(t) => {
                                    let _ = write!(sb, "{{s{}}}", t.size);
                                }
                                _ => {}
                            }
                            if o.sprite.is_some_and(|s| {
                                ctx.sprites
                                    .sprite(s)
                                    .properties
                                    .has(IsoFlagType::vegitation)
                            }) {
                                if taint {
                                    sb.push_str("{t?}");
                                } else {
                                    let _ = write!(
                                        sb,
                                        "{{t{:x},{:x},{:x}}}",
                                        o.tintr.to_bits(),
                                        o.tintg.to_bits(),
                                        o.tintb.to_bits()
                                    );
                                }
                            }
                        }
                        sb.push('\n');
                    }
                }
            }
            if !taint {
                let _ = writeln!(sb, "X {}", ctx.rand.next(1000000));
            }
        }
    }
    for t in &ctx.missing_tiles {
        eprintln!("Missing tile definition: {t}");
    }
    Ok(sb)
}

struct ChunkGrid<'a>(std::collections::HashMap<(i32, i32, i32), &'a IsoGridSquare>);
impl GetSquare for ChunkGrid<'_> {
    fn get_grid_square(&self, x: i32, y: i32, z: i32) -> Option<&IsoGridSquare> {
        self.0.get(&(x, y, z)).copied()
    }
}

/// Square properties and collide/path/vision matrices of a chunk range; pairs with CollideProbe.java.
pub fn collide(root: &Path, range: &[String]) -> Result<String, String> {
    let r: Vec<i32> = range
        .iter()
        .map(|s| s.parse::<i32>().map_err(|e| e.to_string()))
        .collect::<Result<_, _>>()?;
    let mut rand = RandAbstract::new(java::Random::new(12345));
    let alarm = AlarmSettings {
        last_stand: false,
        power_grid_exists: true,
        alarm: 1,
        alarm_decay: 1,
    };
    let mut world = IsoWorld::init(root, "Muldraugh, KY", &|_| false, &alarm, None, &mut rand)?;
    let mut chunks = Vec::new();
    {
        let IsoWorld {
            sprites,
            alias,
            tile_definitions,
            meta_grid,
            ..
        } = &mut world;
        let mut ctx = LoadContext {
            sprites,
            alias,
            glass_removed_window_sprite_map: &tile_definitions.glass_removed_window_sprite_map,
            smashed_window_sprite_map: &tile_definitions.smashed_window_sprite_map,
            meta_grid,
            rand: &mut rand,
            locked_houses: 6,
            missing_tiles: Vec::new(),
            lotpack_cache: None,
        };
        for wx in r[0]..=r[2] {
            for wy in r[1]..=r[3] {
                *ctx.rand = RandAbstract::new(java::Random::new(12345));
                let mut ch = IsoChunk::new(wx, wy);
                cell_loader::load_cell_binary_chunk(&mut ctx, wx, wy, &mut ch, &mut Vec::new());
                chunks.push(ch);
            }
        }
    }
    for ch in &mut chunks {
        for z in ch.get_min_level()..=ch.get_max_level() {
            for i in 0..64 {
                if let Some(sq) = ch.get_grid_square_mut(i % 8, i / 8, z) {
                    sq.recalc_properties(&world.sprites);
                }
            }
        }
    }
    let mut all = Vec::new();
    for ch in &chunks {
        for z in ch.get_min_level()..=ch.get_max_level() {
            for y in 0..8 {
                for x in 0..8 {
                    all.extend(ch.get_grid_square(x, y, z));
                }
            }
        }
    }
    let grid = ChunkGrid(all.iter().map(|s| ((s.x, s.y, s.z), *s)).collect());
    let mut sb = String::new();
    for sq in &all {
        let flags: Vec<&str> = sq
            .properties
            .get_flags_list()
            .iter()
            .map(|f| f.name())
            .collect();
        let _ = write!(
            sb,
            "Q {} {} {} {} {} ",
            sq.x,
            sq.y,
            sq.z,
            if sq.have_roof { 1 } else { 0 },
            flags.join(",")
        );
        for k in sq.properties.get_property_names(&world.alias) {
            let _ = write!(
                sb,
                "{k}={};",
                sq.properties.get(&world.alias, k).unwrap_or("null")
            );
        }
        let (c, p, v) = sq.calculate_matrices(&world.alias, &grid);
        let _ = writeln!(sb, " {:x} {c:x} {p:x} {v:x}", sq.has_types);
    }
    Ok(sb)
}

/// Entity scripts in ScriptManager order and the sprites they mark; pairs with EntitySpritesProbe.java.
pub fn entitysprites(root: &Path) -> Result<String, String> {
    let mut rand = RandAbstract::new(java::Random::new(12345));
    let alarm = AlarmSettings {
        last_stand: false,
        power_grid_exists: true,
        alarm: 1,
        alarm_decay: 1,
    };
    let world = IsoWorld::init(root, "Muldraugh, KY", &|_| false, &alarm, None, &mut rand)?;
    for l in &world.log {
        eprintln!("{l}");
    }
    let mut sb = String::new();
    for e in &world.scripts.entities {
        let _ = writeln!(sb, "E {}", e.name);
    }
    let mut names: Vec<&String> = world.sprites.named_map.keys().collect();
    names.sort_unstable();
    for n in names {
        let p = &world.sprites.sprite(world.sprites.named_map[n]).properties;
        let name = p.get(&world.alias, "EntityScriptName");
        if p.has(IsoFlagType::EntityScript) || name.is_some() {
            let _ = writeln!(
                sb,
                "S {n} {} {}",
                if p.has(IsoFlagType::EntityScript) {
                    1
                } else {
                    0
                },
                name.unwrap_or("null")
            );
        }
    }
    Ok(sb)
}

/// Collide matrices and door/window collision objects of one 64-square server cell; pairs with SpecialCollideProbe.java.
pub fn specialcollide(root: &Path, cell: &[String], los: bool) -> Result<String, String> {
    specialcollide_mode(root, cell, if los { 1 } else { 0 })
}

/// mode 0: collision objects, 1: line of sight (LosProbe.java), 2: stair heights (StairsProbe.java).
fn specialcollide_mode(root: &Path, cell: &[String], mode: u8) -> Result<String, String> {
    let los = mode == 1;
    let toggle = cell.len() == 3;
    let c: Vec<i32> = cell[..2]
        .iter()
        .map(|s| s.parse::<i32>().map_err(|e| e.to_string()))
        .collect::<Result<_, _>>()?;
    let mut rand = RandAbstract::new(java::Random::new(12345));
    let alarm = AlarmSettings {
        last_stand: false,
        power_grid_exists: true,
        alarm: 1,
        alarm_decay: 1,
    };
    let mut world = IsoWorld::init(root, "Muldraugh, KY", &|_| false, &alarm, None, &mut rand)?;
    let mut chunks = Vec::new();
    {
        let IsoWorld {
            sprites,
            alias,
            tile_definitions,
            meta_grid,
            ..
        } = &mut world;
        let mut ctx = LoadContext {
            sprites,
            alias,
            glass_removed_window_sprite_map: &tile_definitions.glass_removed_window_sprite_map,
            smashed_window_sprite_map: &tile_definitions.smashed_window_sprite_map,
            meta_grid,
            rand: &mut rand,
            locked_houses: 6,
            missing_tiles: Vec::new(),
            lotpack_cache: None,
        };
        for cx in 0..8 {
            for cy in 0..8 {
                *ctx.rand = RandAbstract::new(java::Random::new(12345));
                let (wx, wy) = (c[0] * 8 + cx, c[1] * 8 + cy);
                let mut ch = IsoChunk::new(wx, wy);
                cell_loader::load_cell_binary_chunk(&mut ctx, wx, wy, &mut ch, &mut Vec::new());
                chunks.push(ch);
            }
        }
    }
    for ch in &mut chunks {
        for z in ch.get_min_level()..=ch.get_max_level() {
            for i in 0..64 {
                if let Some(sq) = ch.get_grid_square_mut(i % 8, i / 8, z) {
                    if toggle {
                        for o in &mut sq.objects {
                            o.toggle_door_silent();
                        }
                    }
                    sq.recalc_properties(&world.sprites);
                }
            }
        }
    }
    if los {
        // Store the vision matrices (RecalcAllWithNeighbours), which testVisionAdjacent reads.
        let mut vm = Vec::new();
        {
            let mut all = Vec::new();
            for ch in &chunks {
                for z in ch.get_min_level()..=ch.get_max_level() {
                    for i in 0..64 {
                        all.extend(ch.get_grid_square(i % 8, i / 8, z));
                    }
                }
            }
            let grid = ChunkGrid(all.iter().map(|s| ((s.x, s.y, s.z), *s)).collect());
            for sq in &all {
                vm.push(sq.calculate_matrices(&world.alias, &grid).2);
            }
        }
        let mut it = vm.into_iter();
        for ch in &mut chunks {
            for z in ch.get_min_level()..=ch.get_max_level() {
                for i in 0..64 {
                    if let Some(sq) = ch.get_grid_square_mut(i % 8, i / 8, z) {
                        sq.vision_matrix = it.next().unwrap_or(0);
                    }
                }
            }
        }
    }
    let mut all = Vec::new();
    for ch in &chunks {
        for z in ch.get_min_level()..=ch.get_max_level() {
            for y in 0..8 {
                for x in 0..8 {
                    all.extend(ch.get_grid_square(x, y, z));
                }
            }
        }
    }
    let grid = ChunkGrid(all.iter().map(|s| ((s.x, s.y, s.z), *s)).collect());
    if mode == 2 {
        let mut sb = String::from("F true\n");
        for sq in &all {
            if !sq.has_stairs() && sq.get_sloped_surface_direction(&world.alias).is_none() {
                continue;
            }
            let (cm, _, _) = sq.calculate_matrices(&world.alias, &grid);
            let _ = write!(
                sb,
                "Z {} {} {} {} {cm:x}",
                sq.x,
                sq.y,
                sq.z,
                sq.get_stairs().name()
            );
            for py in [0.0f32, 0.25, 0.5, 0.75, 1.0] {
                for px in [0.0f32, 0.25, 0.5, 0.75, 1.0] {
                    let _ = write!(
                        sb,
                        " {:x}",
                        sq.get_apparent_z(&world.alias, px, py, true).to_bits()
                    );
                }
            }
            sb.push('\n');
        }
        return Ok(sb);
    }
    if los {
        let mut sb = String::new();
        for sq in &all {
            if sq.z != 0 || sq.x % 8 != 3 || sq.y % 8 != 3 {
                continue;
            }
            let _ = write!(sb, "L {} {} ", sq.x, sq.y);
            for z in 0..=1 {
                for dy in -10..=10 {
                    for dx in -10..=10 {
                        let r = line_clear(
                            &world.sprites,
                            &world.alias,
                            &grid,
                            sq.x,
                            sq.y,
                            0,
                            sq.x + dx,
                            sq.y + dy,
                            z,
                            false,
                            10000,
                            true,
                        );
                        let _ = write!(sb, "{}", r as u8);
                    }
                }
            }
            sb.push('\n');
        }
        return Ok(sb);
    }
    let mut sb = String::new();
    for sq in &all {
        let (cm, _, _) = sq.calculate_matrices(&world.alias, &grid);
        let _ = write!(sb, "P {} {} {} {cm:x}", sq.x, sq.y, sq.z);
        for (dx, dy) in [
            (0, -1),
            (-1, -1),
            (-1, 0),
            (-1, 1),
            (0, 1),
            (1, 1),
            (1, 0),
            (1, -1),
        ] {
            let next = grid.get_grid_square(sq.x + dx, sq.y + dy, sq.z);
            match sq.test_collide_special_objects(&world.alias, next, &grid) {
                None => sb.push_str(" -"),
                Some((x, y, z, i)) => {
                    let o = &grid
                        .get_grid_square(x, y, z)
                        .ok_or("object square")?
                        .objects[i];
                    let name = o
                        .sprite
                        .and_then(|s| world.sprites.sprite(s).name.clone())
                        .unwrap_or_else(|| "null".into());
                    let _ = write!(sb, " {}:{name}@{x},{y}", o.kind.class_name());
                }
            }
        }
        sb.push('\n');
    }
    Ok(sb)
}

/// The meta grid with procedural basements merged in for the oracle seed; pairs with BasementsProbe.java.
pub fn basements(root: &Path, range: &[String]) -> Result<String, String> {
    use zomboid_engine::zombie::basements::Basements;
    let r: Vec<i32> = range
        .iter()
        .map(|s| s.parse::<i32>().map_err(|e| e.to_string()))
        .collect::<Result<_, _>>()?;
    let media = root.join("media");
    let mut g = IsoMetaGrid::default();
    let dirs = IsoMetaGrid::lot_directories(&media, "Muldraugh, KY");
    g.create_step1(&media, &dirs, (-250, -250, 250, 250))?;
    for e in g.load_all_cells(&mut |_| 1, &mut |_, _| Vec::new()) {
        eprintln!("{e}");
    }
    let alarm = AlarmSettings {
        last_stand: false,
        power_grid_exists: true,
        alarm: 1,
        alarm_decay: 1,
    };
    g.create_step2(&alarm, &mut |_| 1, &mut |a, _| a);
    // WorldGenParams.setSeedString: the seed is the Java hash of the seed text.
    let seed = java::string_hash_code("DeadOxideOracle") as i64;
    let b = Basements::before_load_meta_grid(&media, &mut g, &dirs, seed)?;
    for l in &b.log {
        eprintln!("{l}");
    }
    let mut sb = String::new();
    let _ = writeln!(sb, "SEED {seed}");
    for p in &b.basement_placements {
        let _ = writeln!(sb, "P {} {} {} {} {} {}", p.name, p.x, p.y, p.z, p.w, p.h);
    }
    let mut h: u64 = 0xcbf29ce484222325;
    for bd in &g.buildings {
        mix_long(&mut h, g.building(*bd).id);
    }
    let _ = writeln!(sb, "B {} {h:x}", g.buildings.len());
    for cx in r[0]..=r[2] {
        for cy in r[1]..=r[3] {
            if g.get_header(cx, cy).is_none() {
                continue;
            }
            let Some(cell) = g.get_cell_data(cx, cy) else {
                continue;
            };
            let _ = writeln!(
                sb,
                "C {cx} {cy} {} {}",
                cell.room_list.len(),
                cell.buildings.len()
            );
            for rr in &cell.room_list {
                let room = g.room(*rr);
                let _ = writeln!(
                    sb,
                    "R {} {} {} {} {} {} {} {} [{}]",
                    room.id,
                    room.meta_id,
                    room.level,
                    room.x,
                    room.y,
                    room.x2,
                    room.y2,
                    room.building.map_or(-1, |b| g.building(b).id),
                    room.name
                );
            }
            for bd in &cell.buildings {
                let d = g.building(*bd);
                let _ = writeln!(
                    sb,
                    "D {} {} {} {} {} {} {} {}",
                    d.id,
                    d.meta_id,
                    d.x,
                    d.y,
                    d.x2,
                    d.y2,
                    d.rooms.len(),
                    d.emptyoutside.len()
                );
            }
            for ly in 0..32 {
                for lx in 0..32 {
                    let Some(ch) = cell.chunk((ly * 32 + lx) as usize) else {
                        continue;
                    };
                    if ch.get_rooms_size() == 0 {
                        continue;
                    }
                    let mut ids: Vec<i64> = ch.rooms().iter().map(|rr| g.room(*rr).id).collect();
                    ids.sort_unstable();
                    let _ = write!(sb, "K {cx} {cy} {lx} {ly}");
                    for id in ids {
                        let _ = write!(sb, " {id}");
                    }
                    sb.push('\n');
                }
            }
            let mut h: u64 = 0xcbf29ce484222325;
            for z in -32..=8 {
                for y in cy * 256..cy * 256 + 256 {
                    for x in cx * 256..cx * 256 + 256 {
                        mix_long(
                            &mut h,
                            g.get_room_at(x, y, z).map_or(-1, |rr| g.room(rr).id),
                        );
                        mix_long(
                            &mut h,
                            g.get_empty_outside_at(x, y, z)
                                .map_or(-1, |rr| g.room(rr).id),
                        );
                    }
                }
            }
            let _ = writeln!(sb, "H {cx} {cy} {h:x}");
        }
    }
    Ok(sb)
}

/// Sandbox options at their defaults and after loading a save's map_sand.bin; pairs with SandboxProbe.java.
pub fn sandbox(root: &Path, file: &Path) -> Result<String, String> {
    use zomboid_engine::zombie::sandbox_options::{Kind, SandboxOptions};
    let mut s = SandboxOptions::new();
    let preset = std::fs::read_to_string(root.join("media/lua/shared/Sandbox/Apocalypse.lua"))
        .map_err(|e| e.to_string())?;
    s.load_game_file(&preset);
    let mut sb = String::new();
    let dump = |sb: &mut String, tag: &str, s: &SandboxOptions| {
        for (i, d) in s.defs().iter().enumerate() {
            let ty = match d.kind {
                Kind::Boolean(_) => "boolean",
                Kind::Double { .. } => "double",
                Kind::Integer { .. } => "integer",
                Kind::Enum { .. } => "enum",
                Kind::Str { .. } => "string",
            };
            let _ = writeln!(sb, "{tag}\t{}\t{ty}\t{}", d.name, s.value_as_string(i));
        }
    };
    dump(&mut sb, "D", &s);
    s.load(&std::fs::read(file).map_err(|e| e.to_string())?)?;
    for l in &s.log {
        eprintln!("{l}");
    }
    dump(&mut sb, "L", &s);
    let _ = writeln!(sb, "P\t{}", s.preset);
    Ok(sb)
}

/// Item scripts, pairs with ItemsProbe.java: every loaded item sorted by full name, with the fields named in `fields`
/// (one per line, from tools/gen_item_params.py); floats as raw bits, null as "null".
pub fn items(root: &Path, fields: &Path) -> Result<String, String> {
    use zomboid_engine::zombie::scripting::item::Value;
    use zomboid_engine::zombie::scripting::script_manager::ScriptManager;
    let names =
        std::fs::read_to_string(fields).map_err(|e| format!("{}: {e}", fields.display()))?;
    let names: Vec<&str> = names.lines().filter(|l| !l.is_empty()).collect();
    let m = ScriptManager::load(root)?;
    for (n, e) in &m.item_errors {
        eprintln!("dropped {n}: {e}");
    }
    let mut items: Vec<_> = m.items.iter().collect();
    // java.lang.String.compareTo: UTF-16 order; item names are ASCII.
    items.sort_by_key(|i| i.full_name());
    let mut sb = String::new();
    for it in items {
        sb.push_str(&it.full_name());
        for n in &names {
            let v = it.get(n).ok_or_else(|| format!("no field {n}"))?;
            sb.push('\t');
            match v {
                Value::F32(f) => {
                    let _ = write!(sb, "{:x}", f.to_bits());
                }
                Value::I32(i) => {
                    let _ = write!(sb, "{i}");
                }
                Value::Bool(b) => {
                    let _ = write!(sb, "{b}");
                }
                Value::Str(x) => sb.push_str(x.as_deref().unwrap_or("null")),
            }
        }
        sb.push('\n');
    }
    Ok(sb)
}

/// Craft recipes (`craftRecipe` scripts), pairs with RecipesProbe.java: every loaded recipe sorted by full type, with
/// the fields Load, InitLoadPP, OnScriptsLoaded and OnPostWorldDictionaryInit set; io lines by their index in
/// `ioLines`.
pub fn recipes(root: &Path) -> Result<String, String> {
    use zomboid_engine::zombie::scripting::entity::components::crafting::craft_recipe::{
        IoScript, RequiredSkill, LUA_CALLS,
    };
    use zomboid_engine::zombie::scripting::entity::components::crafting::output_script::MapperRef;
    use zomboid_engine::zombie::scripting::script_manager::ScriptManager;
    /// RecipesProbe.esc: '\\', tab, LF, CR and every UTF-16 unit outside 0x20..0x7e escaped.
    fn esc(s: Option<&str>) -> String {
        let Some(s) = s else {
            return "null".into();
        };
        let mut sb = String::new();
        for u in s.encode_utf16() {
            match u {
                0x5c => sb.push_str("\\\\"),
                0x09 => sb.push_str("\\t"),
                0x0a => sb.push_str("\\n"),
                0x0d => sb.push_str("\\r"),
                0x20..=0x7e => sb.push(char::from(u as u8)),
                _ => {
                    let _ = write!(sb, "\\u{u:04x}");
                }
            }
        }
        sb
    }
    /// RecipesProbe.val of a Collection: `[a, b]`.
    fn list<T>(v: &[T], f: impl Fn(&T) -> String) -> String {
        let items: Vec<String> = v.iter().map(f).collect();
        format!("[{}]", items.join(", "))
    }
    let strs = |v: &[String]| list(v, |s| esc(Some(s)));
    let floats = |v: &[f32]| list(v, |f| java::float_to_string(*f));
    let skills = |v: &Option<Vec<RequiredSkill>>| match v {
        None => "null".to_string(),
        Some(v) => list(v, |s| format!("{}:{}", esc(Some(s.perk)), s.level)),
    };
    let opt = |v: &Option<usize>| v.map_or("null".to_string(), |i| i.to_string());
    let idx = |v: &[usize]| list(v, |i| i.to_string());

    let m = ScriptManager::load(root)?;
    for (n, e) in &m.craft_recipe_errors {
        eprintln!("dropped {n}: {e}");
    }
    for e in [
        &m.craft_recipe_scripts_loaded_error,
        &m.craft_recipe_post_init_error,
    ]
    .into_iter()
    .flatten()
    {
        eprintln!("aborted: {e}");
    }
    let mut recipes: Vec<_> = m.craft_recipes.iter().collect();
    recipes.sort_by(|a, b| {
        java::string_compare_to(
            &a.get_script_object_full_type(),
            &b.get_script_object_full_type(),
        )
    });
    let mut sb = String::new();
    for r in recipes {
        let ft = esc(Some(&r.get_script_object_full_type()));
        let mut lines: Vec<String> = Vec::new();
        lines.push(format!("name\t{}", esc(r.name.as_deref())));
        lines.push(format!(
            "translationName\t{}",
            esc(r.translation_name.as_deref())
        ));
        lines.push(format!(
            "enabled\t{}\tdebugOnly\t{}",
            r.enabled, r.debug_only
        ));
        lines.push(format!(
            "modId\t{}\texistsAsVanilla\t{}",
            esc(r.mod_id.as_deref()),
            r.exists_as_vanilla
        ));
        lines.push(format!("scriptVersion\t{:x}", r.script_version as u64));
        lines.push(format!("iconName\t{}", esc(r.icon_name.as_deref())));
        lines.push(format!("recipeGroup\t{}", esc(r.recipe_group)));
        lines.push(format!("needToBeLearn\t{}", r.need_to_be_learn));
        lines.push(format!("skillRequired\t{}", skills(&r.skill_required)));
        lines.push(format!("autoLearnAny\t{}", skills(&r.auto_learn_any)));
        lines.push(format!("autoLearnAll\t{}", skills(&r.auto_learn_all)));
        lines.push(format!(
            "xpAward\t{}",
            match &r.xp_award {
                None => "null".to_string(),
                Some(v) => list(v, |x| format!("{}:{}", esc(Some(x.perk)), x.amount)),
            }
        ));
        lines.push(format!("metaRecipe\t{}", esc(r.meta_recipe.as_deref())));
        lines.push(format!("time\t{}", r.time));
        lines.push(format!(
            "loadedTimedActionScript\t{}",
            esc(r.loaded_timed_action_script.as_deref())
        ));
        lines.push(format!(
            "timedActionScript\t{}",
            esc(r.timed_action_script.as_deref())
        ));
        let mut lc = String::from("luaCalls");
        for (i, c) in LUA_CALLS.iter().enumerate() {
            if let Some(v) = &r.lua_calls[i] {
                let _ = write!(lc, "\t{c}={}", esc(Some(v)));
            }
        }
        lines.push(lc);
        lines.push(format!("onAddToMenu\t{}", esc(r.on_add_to_menu.as_deref())));
        lines.push(format!("category\t{}", esc(r.category.as_deref())));
        lines.push(format!("categoryTags\t{}", strs(&r.category_tags)));
        lines.push(format!("animation\t{}", esc(r.animation.as_deref())));
        lines.push(format!("tooltip\t{}", esc(r.tooltip.as_deref())));
        lines.push(format!(
            "allowBatchCraft\t{}\tcanWalk\t{}",
            r.allow_batch_craft, r.can_walk
        ));
        lines.push(format!("researchSkillLevel\t{}", r.research_skill_level));
        lines.push(format!(
            "researchAll\t{}\tresearchAny\t{}",
            list(&r.research_all, |p| esc(Some(p))),
            list(&r.research_any, |p| esc(Some(p)))
        ));
        lines.push(format!(
            "inputs\t{}\toutputs\t{}",
            idx(&r.inputs),
            idx(&r.outputs)
        ));
        for (i, s) in r.io_lines.iter().enumerate() {
            let mut l = format!("io\t{i}");
            match s {
                IoScript::Input(s) => {
                    let mut tags: Vec<String> = s
                        .item_tags
                        .iter()
                        .map(|t| t.as_ref().map_or("null".to_string(), |t| t.to_string()))
                        .collect();
                    tags.sort_by(|a, b| java::string_compare_to(a, b));
                    let _ = write!(
                        l,
                        "\tin\t{}\tline\t{}\tamount\t{}\t{}\tamounts\t{}\t{}\tloadedItems\t{}\tloadedFluids\t{}\tloadedEnergies\t{}\titemTags\t{}\tcategories\t{}\tany\t{}\t{}\t{}\tapplyMode\t{}\tfluidMode\t{}\tapplyOnTick\t{}\tshapedIndex\t{}\tflags\t{}\tcreateTo\t{}\tconsumeFrom\t{}\tparent\t{}",
                        s.r#type.name(),
                        esc(Some(&s.original_line)),
                        java::float_to_string(s.amount),
                        java::float_to_string(s.maxamount),
                        floats(&s.amounts),
                        floats(&s.maxamounts),
                        strs(&s.loaded_items),
                        strs(&s.loaded_fluids),
                        strs(&s.loaded_energies),
                        strs(&tags),
                        strs(&s.categories),
                        s.accepts_any_item,
                        s.accepts_any_fluid,
                        s.accepts_any_energy,
                        s.item_apply_mode.name(),
                        s.fluid_match_mode.name(),
                        s.apply_on_tick,
                        s.shaped_index,
                        list(&s.flags.iter().collect::<Vec<_>>(), |f| f.name().to_string()),
                        opt(&s.create_to_item_script),
                        opt(&s.consume_from_item_script),
                        opt(&s.parent_script),
                    );
                }
                // the null of a bare `+` line: the probe would fail on it (none in the game's scripts)
                IoScript::Null => l.push_str("\tnull"),
                IoScript::Output(o) => {
                    let mapper = match &o.output_mapper {
                        Some(MapperRef::Recipe(n)) => r.get_output_mapper(n),
                        Some(MapperRef::Own(m)) => Some(m),
                        None => None,
                    };
                    let _ = write!(
                        l,
                        "\tout\t{}\tline\t{}\tamount\t{}\t{}\tchance\t{}\tloadedFluid\t{}\tloadedEnergy\t{}\tapplyMode\t{}\tfluidMode\t{}\tapplyOnTick\t{}\tshapedIndex\t{}\tflags\t{}\tcreateTo\t{}\tmapper\t{}\tresults\t{}",
                        o.r#type.name(),
                        esc(Some(&o.original_line)),
                        java::float_to_string(o.amount),
                        java::float_to_string(o.maxamount),
                        java::float_to_string(o.chance),
                        esc(o.loaded_fluid.as_deref()),
                        esc(o.loaded_energy.as_deref()),
                        o.item_apply_mode.name(),
                        o.fluid_match_mode.name(),
                        o.apply_on_tick,
                        o.shaped_index,
                        list(&o.flags.iter().collect::<Vec<_>>(), |f| f.name().to_string()),
                        opt(&o.create_to_item_script),
                        esc(o.mapper_name()),
                        mapper.map_or("null".to_string(), |m| strs(&m.result_items)),
                    );
                }
            }
            lines.push(l);
        }
        lines.push(format!(
            "prop1\t{}\tprop2\t{}",
            opt(&r.prop1),
            opt(&r.prop2)
        ));
        lines.push(format!(
            "toolLeft\t{}\ttoolRight\t{}\tusesTools\t{}",
            opt(&r.tool_left),
            opt(&r.tool_right),
            r.uses_tools
        ));
        lines.push(format!(
            "hasOnTick\t{}\t{}",
            r.has_on_tick_inputs, r.has_on_tick_outputs
        ));
        let mut mappers: Vec<_> = r.output_mappers.iter().collect();
        mappers.sort_by(|a, b| java::string_compare_to(&a.0, &b.0));
        for (n, m) in mappers {
            let mut l = format!("outputMapper\t{}\tdefault\t", esc(Some(n)));
            match &m.default_output_entree {
                None => l.push_str("null"),
                Some(d) => {
                    let _ = write!(
                        l,
                        "{}={}",
                        esc(Some(&d.full_type)),
                        esc(d.result.as_deref())
                    );
                }
            }
            let entrees = list(&m.entrees, |e| {
                format!(
                    "{}={} <- {}={}",
                    esc(Some(&e.full_type)),
                    esc(e.result.as_deref()),
                    strs(&e.pattern_full_types),
                    strs(&e.pattern)
                )
            });
            let mut map: Vec<String> = m
                .entree_map
                .iter()
                .map(|(k, i)| format!("{}={i}", esc(Some(k))))
                .collect();
            map.sort_by(|a, b| java::string_compare_to(a, b));
            let _ = write!(
                l,
                "\tentrees\t{entrees}\tentreeMap\t{}\tresults\t{}\tinputs\t{}",
                strs(&map),
                strs(&m.result_items),
                idx(&m.input_scripts)
            );
            lines.push(l);
        }
        let om = &r.overlay_mapper;
        let mut styles: Vec<String> = om
            .style_map
            .iter()
            .map(|(item, style)| format!("{item}={style}"))
            .collect();
        styles.sort_by(|a, b| java::string_compare_to(a, b));
        lines.push(format!(
            "overlayMapper\tdefault\t{}\tstyles\t{}\tinput\t{}",
            strs(&om.default_styles),
            strs(&styles),
            opt(&om.input_script)
        ));
        for line in lines {
            let _ = writeln!(sb, "{ft}\t{line}");
        }
    }
    Ok(sb)
}

/// zombie.scripting.objects.VehicleScript / VehicleTemplate as the server loads them, pairs with VehiclesProbe.java:
/// every vehicle script sorted by full name, then every vehicle template sorted by full type with the script its
/// getScript() built (if any); every field in declaration order, one line each.
pub fn vehicles(root: &Path) -> Result<String, String> {
    use zomboid_engine::org::joml::vector2f::Vector2f;
    use zomboid_engine::org::joml::vector3f::Vector3f;
    use zomboid_engine::se::krka::kahlua::j2se::kahlua_table_impl::{KahluaTable, LuaValue};
    use zomboid_engine::zombie::core::immutable_color::ImmutableColor;
    use zomboid_engine::zombie::scripting::objects::vehicle_script::{
        Anim, Area, Model, Part, Passenger, PhysicsShape, Position, Skin, SwitchSeat,
        VehicleScript, Wheel,
    };
    use zomboid_engine::zombie::scripting::script_manager::ScriptManager;
    /// VehiclesProbe.esc: '\\', tab, LF, CR and every UTF-16 unit outside 0x20..0x7e escaped.
    fn esc(s: Option<&str>) -> String {
        let Some(s) = s else {
            return "null".into();
        };
        let mut sb = String::new();
        for u in s.encode_utf16() {
            match u {
                0x5c => sb.push_str("\\\\"),
                0x09 => sb.push_str("\\t"),
                0x0a => sb.push_str("\\n"),
                0x0d => sb.push_str("\\r"),
                0x20..=0x7e => sb.push(char::from(u as u8)),
                _ => {
                    let _ = write!(sb, "\\u{u:04x}");
                }
            }
        }
        sb
    }
    fn o(s: &Option<String>) -> String {
        esc(s.as_deref())
    }
    fn f(x: f32) -> String {
        java::float_to_string(x)
    }
    fn v3(v: &Vector3f) -> String {
        format!("({}, {}, {})", f(v.x), f(v.y), f(v.z))
    }
    fn v2(v: &Vector2f) -> String {
        format!("({}, {})", f(v.x), f(v.y))
    }
    fn color(c: &Option<ImmutableColor>) -> String {
        match c {
            None => "null".into(),
            Some(c) => format!("rgba({}, {}, {}, {})", f(c.r), f(c.g), f(c.b), f(c.a)),
        }
    }
    fn floats(v: &[f32]) -> String {
        let l: Vec<String> = v.iter().map(|x| f(*x)).collect();
        format!("[{}]", l.join(", "))
    }
    fn strs(v: &[String]) -> String {
        let l: Vec<String> = v.iter().map(|x| esc(Some(x))).collect();
        format!("[{}]", l.join(", "))
    }
    fn ostrs(v: &Option<Vec<String>>) -> String {
        v.as_ref().map_or("null".into(), |v| strs(v))
    }
    /// VehiclesProbe.val of a Set: the escaped elements, sorted.
    fn set(v: &[String]) -> String {
        let mut l: Vec<String> = v.iter().map(|x| esc(Some(x))).collect();
        l.sort();
        format!("[{}]", l.join(", "))
    }
    /// VehiclesProbe.val of a Map: "k=v" entries, sorted.
    fn map(v: &[(String, String)]) -> String {
        let mut l: Vec<String> = v
            .iter()
            .map(|(k, x)| format!("{}={}", esc(Some(k)), esc(Some(x))))
            .collect();
        l.sort();
        format!("{{{}}}", l.join(", "))
    }
    /// VehiclesProbe.table: "n:" + Double.toString or "s:" + the escaped key, "=", the value; sorted.
    fn table(t: &KahluaTable) -> String {
        let mut l: Vec<String> = t
            .entries
            .iter()
            .map(|(k, v)| {
                let ks = match k {
                    LuaValue::Number(d) => format!("n:{}", java::double_to_string(*d)),
                    LuaValue::String(s) => format!("s:{}", esc(Some(s))),
                    LuaValue::Boolean(b) => format!("s:{b}"),
                    LuaValue::Table(_) => "s:table".into(),
                };
                let vs = match v {
                    LuaValue::String(s) => esc(Some(s)),
                    LuaValue::Number(d) => java::double_to_string(*d),
                    LuaValue::Boolean(b) => b.to_string(),
                    LuaValue::Table(t) => table(t),
                };
                format!("{ks}={vs}")
            })
            .collect();
        l.sort();
        format!("{{{}}}", l.join(", "))
    }
    fn line(sb: &mut String, path: &str, field: &str, value: &str) {
        let _ = writeln!(sb, "{path}\t{field}\t{value}");
    }
    /// VehiclesProbe.dump of a list of objects: "[]" when empty, else "size N" and one block per element.
    fn objs<T>(
        sb: &mut String,
        path: &str,
        field: &str,
        list: &[T],
        each: fn(&mut String, &str, &T),
    ) {
        if list.is_empty() {
            line(sb, path, field, "[]");
            return;
        }
        line(sb, path, field, &format!("size {}", list.len()));
        for (i, x) in list.iter().enumerate() {
            each(sb, &format!("{path}\t{field}[{i}]"), x);
        }
    }
    fn oobjs<T>(
        sb: &mut String,
        path: &str,
        field: &str,
        list: &Option<Vec<T>>,
        each: fn(&mut String, &str, &T),
    ) {
        match list {
            None => line(sb, path, field, "null"),
            Some(l) => objs(sb, path, field, l, each),
        }
    }
    fn model(sb: &mut String, p: &str, m: &Model) {
        line(sb, p, "id", &o(&m.id));
        line(sb, p, "file", &o(&m.file));
        line(sb, p, "scale", &f(m.scale));
        line(sb, p, "offset", &v3(&m.offset));
        line(sb, p, "rotate", &v3(&m.rotate));
        line(sb, p, "attachmentNameParent", &o(&m.attachment_name_parent));
        line(sb, p, "attachmentNameSelf", &o(&m.attachment_name_self));
        line(
            sb,
            p,
            "ignoreVehicleScale",
            &m.ignore_vehicle_scale.to_string(),
        );
    }
    fn anim(sb: &mut String, p: &str, a: &Anim) {
        line(sb, p, "id", &o(&a.id));
        line(sb, p, "anim", &o(&a.anim));
        line(sb, p, "rate", &f(a.rate));
        line(sb, p, "animate", &a.animate.to_string());
        line(sb, p, "loop", &a.r#loop.to_string());
        line(sb, p, "reverse", &a.reverse.to_string());
        line(sb, p, "offset", &v3(&a.offset));
        line(sb, p, "angle", &v3(&a.angle));
        line(sb, p, "sound", &o(&a.sound));
    }
    fn switch_seat(sb: &mut String, p: &str, s: &SwitchSeat) {
        line(sb, p, "id", &o(&s.id));
        line(sb, p, "seat", &s.seat.to_string());
        line(sb, p, "anim", &o(&s.anim));
        line(sb, p, "rate", &f(s.rate));
        line(sb, p, "sound", &o(&s.sound));
    }
    fn position(sb: &mut String, p: &str, x: &Position) {
        line(sb, p, "id", &o(&x.id));
        line(sb, p, "offset", &v3(&x.offset));
        line(sb, p, "rotate", &v3(&x.rotate));
        line(sb, p, "area", &o(&x.area));
    }
    fn passenger(sb: &mut String, p: &str, x: &Passenger) {
        line(sb, p, "id", &o(&x.id));
        objs(sb, p, "anims", &x.anims, anim);
        objs(sb, p, "switchSeats", &x.switch_seats, switch_seat);
        line(sb, p, "hasRoof", &x.has_roof.to_string());
        line(sb, p, "showPassenger", &x.show_passenger.to_string());
        line(sb, p, "door", &o(&x.door));
        line(sb, p, "door2", &o(&x.door2));
        line(sb, p, "area", &o(&x.area));
        objs(sb, p, "positions", &x.positions, position);
    }
    fn physics(sb: &mut String, p: &str, x: &PhysicsShape) {
        line(sb, p, "type", &x.r#type.to_string());
        line(sb, p, "offset", &v3(&x.offset));
        line(sb, p, "rotate", &v3(&x.rotate));
        line(sb, p, "extents", &v3(&x.extents));
        line(sb, p, "radius", &f(x.radius));
        line(sb, p, "physicsShapeScript", &o(&x.physics_shape_script));
    }
    fn wheel(sb: &mut String, p: &str, x: &Wheel) {
        line(sb, p, "id", &o(&x.id));
        line(sb, p, "model", &o(&x.model));
        line(sb, p, "front", &x.front.to_string());
        line(sb, p, "offset", &v3(&x.offset));
        line(sb, p, "radius", &f(x.radius));
        line(sb, p, "width", &f(x.width));
    }
    fn skin(sb: &mut String, p: &str, x: &Skin) {
        line(sb, p, "texture", &o(&x.texture));
        line(sb, p, "textureRust", &o(&x.texture_rust));
        line(sb, p, "textureMask", &o(&x.texture_mask));
        line(sb, p, "textureLights", &o(&x.texture_lights));
        line(
            sb,
            p,
            "textureDamage1Overlay",
            &o(&x.texture_damage1_overlay),
        );
        line(sb, p, "textureDamage1Shell", &o(&x.texture_damage1_shell));
        line(
            sb,
            p,
            "textureDamage2Overlay",
            &o(&x.texture_damage2_overlay),
        );
        line(sb, p, "textureDamage2Shell", &o(&x.texture_damage2_shell));
        line(sb, p, "textureShadow", &o(&x.texture_shadow));
        // The Texture objects, never loaded on a server.
        for t in [
            "textureData",
            "textureDataRust",
            "textureDataMask",
            "textureDataLights",
            "textureDataDamage1Overlay",
            "textureDataDamage1Shell",
            "textureDataDamage2Overlay",
            "textureDataDamage2Shell",
            "textureDataShadow",
        ] {
            line(sb, p, t, "null");
        }
    }
    fn area(sb: &mut String, p: &str, x: &Area) {
        line(sb, p, "id", &o(&x.id));
        line(sb, p, "x", &f(x.x));
        line(sb, p, "y", &f(x.y));
        line(sb, p, "w", &f(x.w));
        line(sb, p, "h", &f(x.h));
    }
    fn part(sb: &mut String, p: &str, x: &Part) {
        line(sb, p, "id", &o(&x.id));
        line(sb, p, "parent", &o(&x.parent));
        line(sb, p, "itemType", &ostrs(&x.item_type));
        match &x.container {
            None => line(sb, p, "container", "null"),
            Some(c) => {
                line(sb, p, "container", "object");
                let q = format!("{p}\tcontainer");
                line(sb, &q, "capacity", &c.capacity.to_string());
                line(sb, &q, "seat", &c.seat.to_string());
                line(sb, &q, "seatId", &o(&c.seat_id));
                line(sb, &q, "luaTest", &o(&c.lua_test));
                line(sb, &q, "contentType", &o(&c.content_type));
                line(
                    sb,
                    &q,
                    "conditionAffectsCapacity",
                    &c.condition_affects_capacity.to_string(),
                );
                line(sb, &q, "soundMap", &map(&c.sound_map));
            }
        }
        line(sb, p, "area", &o(&x.area));
        line(sb, p, "mechanicArea", &o(&x.mechanic_area));
        line(sb, p, "wheel", &o(&x.wheel));
        match &x.tables {
            None => line(sb, p, "tables", "null"),
            Some(t) if t.is_empty() => line(sb, p, "tables", "{}"),
            Some(t) => {
                line(sb, p, "tables", &format!("size {}", t.len()));
                let mut keys: Vec<&(Option<String>, KahluaTable)> = t.iter().collect();
                keys.sort_by(|a, b| {
                    java::string_compare_to(
                        a.0.as_deref().unwrap_or(""),
                        b.0.as_deref().unwrap_or(""),
                    )
                });
                for (k, v) in keys {
                    let _ = writeln!(sb, "{p}\ttables[{}]\t{}", o(k), table(v));
                }
            }
        }
        line(
            sb,
            p,
            "luaFunctions",
            &x.lua_functions.as_ref().map_or("null".into(), |m| map(m)),
        );
        oobjs(sb, p, "models", &x.models, model);
        line(
            sb,
            p,
            "setAllModelsVisible",
            &x.set_all_models_visible.to_string(),
        );
        line(
            sb,
            p,
            "door",
            if x.door.is_some() { "object" } else { "null" },
        );
        match &x.window {
            None => line(sb, p, "window", "null"),
            Some(w) => {
                line(sb, p, "window", "object");
                line(
                    sb,
                    &format!("{p}\twindow"),
                    "openable",
                    &w.openable.to_string(),
                );
            }
        }
        oobjs(sb, p, "anims", &x.anims, anim);
        line(sb, p, "category", &o(&x.category));
        line(sb, p, "specificItem", &x.specific_item.to_string());
        line(
            sb,
            p,
            "mechanicRequireKey",
            &x.mechanic_require_key.to_string(),
        );
        line(sb, p, "repairMechanic", &x.repair_mechanic.to_string());
        line(sb, p, "hasLightsRear", &x.has_lights_rear.to_string());
        line(sb, p, "durability", &f(x.durability));
    }
    fn base(sb: &mut String, p: &str, s: &VehicleScript) {
        let _ = writeln!(
            sb,
            "{p}\tmodule\t{}\tenabled\t{}\tdebugOnly\t{}\tscriptVersion\t{:x}\tbodies\t{}",
            esc(Some(&s.module)),
            s.enabled,
            s.debug_only,
            s.script_version as u64,
            s.loaded_script_bodies.len() * 2
        );
    }
    fn dump(sb: &mut String, p: &str, s: &VehicleScript) {
        line(sb, p, "fileName", &o(&s.file_name));
        line(sb, p, "name", &o(&s.name));
        objs(sb, p, "models", &s.models, model);
        if s.attachments.is_empty() {
            line(sb, p, "attachments", "[]");
        } else {
            line(
                sb,
                p,
                "attachments",
                &format!("size {}", s.attachments.len()),
            );
            for (i, a) in s.attachments.iter().enumerate() {
                let q = format!("{p}\tattachments[{i}]");
                line(sb, &q, "owner", if a.owned { "self" } else { "null" });
                line(sb, &q, "id", &esc(Some(&a.id)));
                line(sb, &q, "offset", &v3(&a.offset));
                line(sb, &q, "rotate", &v3(&a.rotate));
                line(sb, &q, "scale", &f(a.scale));
                line(sb, &q, "bone", &o(&a.bone));
                line(sb, &q, "canAttach", &ostrs(&a.can_attach));
                line(sb, &q, "zoffset", &f(a.zoffset));
                line(sb, &q, "updateConstraint", &a.update_constraint.to_string());
            }
        }
        line(sb, p, "mass", &f(s.mass));
        line(sb, p, "centerOfMassOffset", &v3(&s.center_of_mass_offset));
        for (k, v) in [
            ("engineForce", s.engine_force),
            ("engineIdleSpeed", s.engine_idle_speed),
            ("steeringIncrement", s.steering_increment),
            ("steeringClamp", s.steering_clamp),
            ("steeringClampMax", s.steering_clamp_max),
            ("wheelFriction", s.wheel_friction),
            ("stoppingMovementForce", s.stopping_movement_force),
            ("animalTrailerSize", s.animal_trailer_size),
            ("suspensionStiffness", s.suspension_stiffness),
            ("suspensionDamping", s.suspension_damping),
            ("suspensionCompression", s.suspension_compression),
            ("suspensionRestLength", s.suspension_rest_length),
            ("maxSuspensionTravelCm", s.max_suspension_travel_cm),
            ("rollInfluence", s.roll_influence),
        ] {
            line(sb, p, k, &f(v));
        }
        line(sb, p, "extents", &v3(&s.extents));
        line(sb, p, "shadowExtents", &v2(&s.shadow_extents));
        line(sb, p, "shadowOffset", &v2(&s.shadow_offset));
        line(
            sb,
            p,
            "hadShadowOExtents",
            &s.had_shadow_o_extents.to_string(),
        );
        line(sb, p, "hadShadowOffset", &s.had_shadow_offset.to_string());
        line(sb, p, "extentsOffset", &v2(&s.extents_offset));
        line(sb, p, "physicsChassisShape", &v3(&s.physics_chassis_shape));
        objs(sb, p, "physicsShapes", &s.physics_shapes, physics);
        objs(sb, p, "wheels", &s.wheels, wheel);
        objs(sb, p, "crawlThroughWheels", &s.crawl_through_wheels, wheel);
        objs(sb, p, "passengers", &s.passengers, passenger);
        line(sb, p, "maxSpeed", &f(s.max_speed));
        line(
            sb,
            p,
            "useChassisPhysicsCollision",
            &s.use_chassis_physics_collision.to_string(),
        );
        line(sb, p, "maxSpeedReverse", &f(s.max_speed_reverse));
        line(sb, p, "isSmallVehicle", &s.is_small_vehicle.to_string());
        for (k, v) in [
            ("frontEndHealth", s.front_end_health),
            ("rearEndHealth", s.rear_end_health),
            ("storageCapacity", s.storage_capacity),
            ("engineLoudness", s.engine_loudness),
            ("engineQuality", s.engine_quality),
            ("seats", s.seats),
            ("mechanicType", s.mechanic_type),
            ("engineRepairLevel", s.engine_repair_level),
        ] {
            line(sb, p, k, &v.to_string());
        }
        for (k, v) in [
            ("playerDamageProtection", s.player_damage_protection),
            ("forcedHue", s.forced_hue),
            ("forcedSat", s.forced_sat),
            ("forcedVal", s.forced_val),
        ] {
            line(sb, p, k, &f(v));
        }
        line(sb, p, "leftSirenCol", &color(&s.left_siren_col));
        line(sb, p, "rightSirenCol", &color(&s.right_siren_col));
        line(sb, p, "engineRpmType", &esc(Some(&s.engine_rpm_type)));
        line(sb, p, "offroadEfficiency", &f(s.offroad_efficiency));
        line(sb, p, "crawlOffsets", &floats(&s.crawl_offsets));
        line(sb, p, "zombieType", &ostrs(&s.zombie_type));
        line(sb, p, "specialKeyRing", &ostrs(&s.special_key_ring));
        line(sb, p, "notKillCrops", &s.not_kill_crops.to_string());
        line(sb, p, "hasLighter", &s.has_lighter.to_string());
        line(sb, p, "carMechanicsOverlay", &o(&s.car_mechanics_overlay));
        line(sb, p, "carModelName", &o(&s.car_model_name));
        line(
            sb,
            p,
            "specialLootChance",
            &s.special_loot_chance.to_string(),
        );
        line(
            sb,
            p,
            "specialKeyRingChance",
            &s.special_key_ring_chance.to_string(),
        );
        line(sb, p, "neverSpawnKey", &s.never_spawn_key.to_string());
        line(sb, p, "gearRatioCount", &s.gear_ratio_count.to_string());
        line(sb, p, "gearRatio", &floats(&s.gear_ratio));
        line(sb, p, "textures", "object");
        skin(sb, &format!("{p}\ttextures"), &s.textures);
        objs(sb, p, "skins", &s.skins, skin);
        objs(sb, p, "areas", &s.areas, area);
        objs(sb, p, "parts", &s.parts, part);
        line(sb, p, "hasSiren", &s.has_siren.to_string());
        line(sb, p, "lightbar", "object");
        let q = format!("{p}\tlightbar");
        line(sb, &q, "enable", &s.lightbar.enable.to_string());
        line(sb, &q, "soundSiren0", &esc(Some(&s.lightbar.sound_siren0)));
        line(sb, &q, "soundSiren1", &esc(Some(&s.lightbar.sound_siren1)));
        line(sb, &q, "soundSiren2", &esc(Some(&s.lightbar.sound_siren2)));
        line(sb, p, "sound", "object");
        let q = format!("{p}\tsound");
        let so = &s.sound;
        line(sb, &q, "alarmEnable", &so.alarm_enable.to_string());
        line(sb, &q, "alarm", &strs(&so.alarm));
        line(sb, &q, "alarmLoop", &strs(&so.alarm_loop));
        line(sb, &q, "hornEnable", &so.horn_enable.to_string());
        line(sb, &q, "horn", &o(&so.horn));
        line(
            sb,
            &q,
            "backSignalEnable",
            &so.back_signal_enable.to_string(),
        );
        line(sb, &q, "backSignal", &o(&so.back_signal));
        line(sb, &q, "engine", &o(&so.engine));
        line(sb, &q, "engineStart", &o(&so.engine_start));
        line(sb, &q, "engineTurnOff", &o(&so.engine_turn_off));
        line(sb, &q, "handBrake", &o(&so.hand_brake));
        line(sb, &q, "ignitionFail", &o(&so.ignition_fail));
        line(
            sb,
            &q,
            "ignitionFailNoPower",
            &o(&so.ignition_fail_no_power),
        );
        line(sb, &q, "specified", &set(&so.specified));
        line(
            sb,
            p,
            "textureMaskEnable",
            &s.texture_mask_enable.to_string(),
        );
    }

    let m = ScriptManager::load(root)?;
    for (n, e) in &m.vehicle_errors {
        eprintln!("dropped {n}: {e}");
    }
    for l in &m.vehicle_log {
        eprintln!("log: {l}");
    }
    let mut sb = String::new();
    let mut vehicles: Vec<&VehicleScript> = m.vehicles.iter().collect();
    vehicles.sort_by(|a, b| java::string_compare_to(&a.get_full_name(), &b.get_full_name()));
    for v in vehicles {
        let p = esc(Some(&v.get_full_name()));
        base(&mut sb, &p, v);
        dump(&mut sb, &p, v);
    }
    let mut templates: Vec<_> = m.vehicle_templates.iter().collect();
    templates.sort_by(|a, b| {
        java::string_compare_to(
            &a.get_script_object_full_type(),
            &b.get_script_object_full_type(),
        )
    });
    for t in templates {
        let p = format!("template:{}", esc(Some(&t.get_script_object_full_type())));
        let _ = writeln!(
            sb,
            "{p}\tname\t{}\tbody\t{}\t{}\tscript\t{}",
            esc(Some(&t.name)),
            t.body.encode_utf16().count(),
            java::string_hash_code(&t.body),
            if t.script.is_some() { "built" } else { "null" }
        );
        if let Some(s) = &t.script {
            base(&mut sb, &p, s);
            dump(&mut sb, &p, s);
        }
    }
    Ok(sb)
}

/// The XUI scripts and XuiManager.ParseScripts' state, pairs with XuiProbe.java: the six XUI script lists sorted by
/// full type, XuiLuaStyle's statics (the var registry), XuiManager's statics with every skin, and the colours of
/// zombie.core.Colors by name.
pub fn xui(root: &Path) -> Result<String, String> {
    use zomboid_engine::zombie::core::color::Color;
    use zomboid_engine::zombie::scripting::objects::base_script_object::BaseScriptObject;
    use zomboid_engine::zombie::scripting::objects::xui_colors_script::XuiColorsScript;
    use zomboid_engine::zombie::scripting::objects::xui_config_script::XuiConfigScript;
    use zomboid_engine::zombie::scripting::objects::xui_layout_script::XuiLayoutScript;
    use zomboid_engine::zombie::scripting::objects::xui_skin_script::{
        ComponentUiScript, EntityUiScript, StyleInfoScript, XuiSkinScript,
    };
    use zomboid_engine::zombie::scripting::script_manager::ScriptManager;
    use zomboid_engine::zombie::scripting::ui::xui_lua_style::{
        XuiLuaStyle, XuiValue, XuiVar, ALLOWED_VAR_TYPES,
    };
    use zomboid_engine::zombie::scripting::ui::xui_manager::{DEFAULT_SKIN_NAME, XUI_SCRIPT_TYPES};
    use zomboid_engine::zombie::scripting::ui::xui_skin::{
        ComponentUiStyle, EntityUiStyle, StyleInfo, XuiSkin,
    };
    use zomboid_engine::zombie::scripting::ui::JHashMap;
    /// XuiProbe.esc: '\\', tab, LF, CR and every UTF-16 unit outside 0x20..0x7e escaped.
    fn esc(s: Option<&str>) -> String {
        let Some(s) = s else {
            return "null".into();
        };
        let mut sb = String::new();
        for u in s.encode_utf16() {
            match u {
                0x5c => sb.push_str("\\\\"),
                0x09 => sb.push_str("\\t"),
                0x0a => sb.push_str("\\n"),
                0x0d => sb.push_str("\\r"),
                0x20..=0x7e => sb.push(char::from(u as u8)),
                _ => {
                    let _ = write!(sb, "\\u{u:04x}");
                }
            }
        }
        sb
    }
    fn e(s: &str) -> String {
        esc(Some(s))
    }
    fn o(s: &Option<String>) -> String {
        esc(s.as_deref())
    }
    fn rgba(c: &Color) -> String {
        format!(
            "rgba({}, {}, {}, {})",
            java::float_to_string(c.r),
            java::float_to_string(c.g),
            java::float_to_string(c.b),
            java::float_to_string(c.a)
        )
    }
    fn line(sb: &mut String, p: &str, f: &str, v: &str) {
        let _ = writeln!(sb, "{p}\t{f}\t{v}");
    }
    fn head(sb: &mut String, p: &str, n: usize) {
        let _ = writeln!(sb, "{p}\tsize {n}");
    }
    /// XuiProbe.val of a List.
    fn list(v: &[String]) -> String {
        let l: Vec<String> = v.iter().map(|s| e(s)).collect();
        format!("[{}]", l.join(", "))
    }
    /// XuiProbe.val of a Set: sorted.
    fn set(mut v: Vec<String>) -> String {
        v.sort_by(|a, b| java::string_compare_to(a, b));
        format!("[{}]", v.join(", "))
    }
    fn texture(t: &Option<String>) -> String {
        t.as_ref()
            .map_or("null".into(), |n| format!("texture:{}", e(n)))
    }
    fn base(sb: &mut String, p: &str, b: &BaseScriptObject) {
        let _ = writeln!(
            sb,
            "{p}\tmodule\t{}\tenabled\t{}\tdebugOnly\t{}\tscriptVersion\t{:x}\tbodies\t{}",
            o(&b.module),
            b.enabled,
            b.debug_only,
            b.script_version as u64,
            b.get_loaded_script_body_count()
        );
    }
    fn value(v: Option<&XuiValue>) -> String {
        match v {
            None => "null".into(),
            Some(XuiValue::Boolean(b)) => b.to_string(),
            Some(XuiValue::Double(d)) => java::double_to_string(*d),
            Some(XuiValue::Font(f)) => f.name().to_string(),
            Some(XuiValue::Str(s)) => e(s),
            Some(XuiValue::StrList(l)) => list(l),
            Some(XuiValue::Color(c)) => rgba(c),
        }
    }
    fn str_map(sb: &mut String, p: &str, m: &JHashMap<Option<String>>) {
        head(sb, p, m.len());
        for (k, v) in m.sorted() {
            let _ = writeln!(sb, "{p}[{}]\t=\t{}", e(k), o(v));
        }
    }
    fn color_map(sb: &mut String, p: &str, m: &JHashMap<Color>) {
        head(sb, p, m.len());
        for (k, v) in m.sorted() {
            let _ = writeln!(sb, "{p}[{}]\t=\t{}", e(k), rgba(v));
        }
    }
    fn var(sb: &mut String, p: &str, v: &XuiVar) {
        line(sb, p, "class", v.class.simple_name());
        line(sb, p, "uiOrder", &v.ui_order.to_string());
        line(sb, p, "type", v.r#type.name());
        line(sb, p, "parent", if v.has_parent { "owner" } else { "null" });
        line(sb, p, "valueSet", &v.value_set.to_string());
        line(sb, p, "autoApply", v.auto_apply.name());
        line(sb, p, "defaultValue", &value(v.default_value.as_ref()));
        line(sb, p, "value", &value(v.value.as_ref()));
        line(sb, p, "luaTableKey", &e(&v.lua_table_key));
    }
    fn style(sb: &mut String, p: &str, s: &XuiLuaStyle) {
        line(sb, p, "xuiLuaClass", &e(&s.xui_lua_class));
        line(sb, p, "xuiStyleName", &e(&s.xui_style_name));
        let skin = s
            .xui_skin
            .as_ref()
            .map_or("null".into(), |n| format!("skin:{}", e(n)));
        line(sb, p, "xuiSkin", &skin);
        let pm = format!("{p}\tvarsMap");
        head(sb, &pm, s.vars_map.len());
        for (k, i) in s.vars_map.sorted() {
            let _ = writeln!(sb, "{pm}[{}]\t=\t#{i}", e(k));
        }
        if s.vars.is_empty() {
            line(sb, p, "vars", "[]");
        } else {
            line(sb, p, "vars", &format!("size {}", s.vars.len()));
            for (i, v) in s.vars.iter().enumerate() {
                var(sb, &format!("{p}\tvars[{i}]"), v);
            }
        }
    }
    fn component_script(sb: &mut String, p: &str, c: &ComponentUiScript) {
        line(sb, p, "luaPanelClass", &o(&c.lua_panel_class));
        line(sb, p, "xuiStyle", &o(&c.xui_style));
        line(sb, p, "displayName", &o(&c.display_name));
        line(sb, p, "iconPath", &o(&c.icon_path));
        line(sb, p, "listOrderZset", &c.list_order_zset.to_string());
        line(sb, p, "listOrderZ", &c.list_order_z.to_string());
        line(sb, p, "enabledSet", &c.enabled_set.to_string());
        line(sb, p, "enabled", &c.enabled.to_string());
    }
    fn entity_script(sb: &mut String, p: &str, u: &EntityUiScript) {
        line(sb, p, "luaWindowClass", &o(&u.lua_window_class));
        line(sb, p, "xuiStyle", &o(&u.xui_style));
        line(sb, p, "luaCanOpenWindow", &o(&u.lua_can_open_window));
        line(sb, p, "luaOpenWindow", &o(&u.lua_open_window));
        line(sb, p, "displayName", &o(&u.display_name));
        line(sb, p, "description", &o(&u.description));
        line(sb, p, "buildDescription", &o(&u.build_description));
        line(sb, p, "iconPath", &o(&u.icon_path));
        line(sb, p, "clearComponents", &u.clear_components.to_string());
        let pm = format!("{p}\tcomponentUiScriptMap");
        head(sb, &pm, u.component_ui_script_map.len());
        let mut m: Vec<_> = u.component_ui_script_map.iter().collect();
        m.sort_by(|a, b| java::string_compare_to(a.0.name(), b.0.name()));
        for (t, c) in m {
            component_script(sb, &format!("{pm}[{}]", t.name()), c);
        }
    }
    fn style_info_script(sb: &mut String, p: &str, s: &StyleInfoScript) {
        str_map(
            sb,
            &format!("{p}\tdefaultStyleBlock"),
            &s.default_style_block,
        );
        let pm = format!("{p}\tstyleBlocks");
        head(sb, &pm, s.style_blocks.len());
        for (k, m) in s.style_blocks.sorted() {
            str_map(sb, &format!("{pm}[{}]", e(k)), m);
        }
    }
    fn colors_script(sb: &mut String, p: &str, c: &XuiColorsScript) {
        base(sb, p, &c.base);
        line(sb, p, "name", &o(&c.name));
        color_map(sb, &format!("{p}\tcolorMap"), &c.color_map);
    }
    fn skin_script(sb: &mut String, p: &str, s: &XuiSkinScript) {
        base(sb, p, &s.base);
        line(sb, p, "imports", &list(&s.imports));
        line(sb, p, "defaultEntityUiScript", "object");
        entity_script(
            sb,
            &format!("{p}\tdefaultEntityUiScript"),
            &s.default_entity_ui_script,
        );
        let pm = format!("{p}\tentityUiScriptMap");
        head(sb, &pm, s.entity_ui_script_map.len());
        for (k, u) in s.entity_ui_script_map.sorted() {
            entity_script(sb, &format!("{pm}[{}]", e(k)), u);
        }
        let pm = format!("{p}\tstyleInfoMap");
        head(sb, &pm, s.style_info_map.len());
        for (k, i) in s.style_info_map.sorted() {
            style_info_script(sb, &format!("{pm}[{}]", e(k)), i);
        }
        line(sb, p, "colorsScript", "object");
        colors_script(sb, &format!("{p}\tcolorsScript"), &s.colors_script);
    }
    fn config(sb: &mut String, p: &str, c: &XuiConfigScript) {
        base(sb, p, &c.base);
        let pm = format!("{p}\tvarConfigs");
        head(sb, &pm, c.var_configs.len());
        let mut m: Vec<_> = c.var_configs.iter().collect();
        m.sort_by(|a, b| java::string_compare_to(a.0.name(), b.0.name()));
        for (t, keys) in m {
            let _ = writeln!(sb, "{pm}[{}]\t=\t{}", t.name(), list(keys));
        }
    }
    fn layout(sb: &mut String, p: &str, l: &XuiLayoutScript) {
        base(sb, p, &l.base);
        let x = l
            .xui_script
            .as_ref()
            .map_or("null".into(), |n| format!("xuiscript:{}", e(n)));
        line(sb, p, "xuiScript", &x);
        line(sb, p, "name", &o(&l.name));
        let total = l.total_file.as_ref().map_or("null".into(), |t| {
            format!(
                "{}\t{}",
                t.encode_utf16().count(),
                java::string_hash_code(t)
            )
        });
        line(sb, p, "totalFile", &total);
        line(sb, p, "hasParsed", &l.has_parsed.to_string());
        line(sb, p, "scriptType", l.script_type.name());
        line(sb, p, "block", if l.block { "set" } else { "null" });
    }
    fn component_style(sb: &mut String, p: &str, c: &ComponentUiStyle) {
        line(sb, p, "luaPanelClass", &o(&c.lua_panel_class));
        line(sb, p, "xuiStyle", &o(&c.xui_style));
        line(sb, p, "displayName", &o(&c.display_name));
        line(sb, p, "icon", &texture(&c.icon));
        line(sb, p, "listOrderZ", &c.list_order_z.to_string());
        line(sb, p, "enabled", &c.enabled.to_string());
    }
    fn entity_style(sb: &mut String, p: &str, u: &EntityUiStyle) {
        line(sb, p, "luaWindowClass", &o(&u.lua_window_class));
        line(sb, p, "xuiStyle", &o(&u.xui_style));
        line(sb, p, "luaCanOpenWindow", &o(&u.lua_can_open_window));
        line(sb, p, "luaOpenWindow", &o(&u.lua_open_window));
        let pm = format!("{p}\tcomponentUiStyleMap");
        head(sb, &pm, u.component_ui_style_map.len());
        let mut m: Vec<_> = u.component_ui_style_map.iter().collect();
        m.sort_by(|a, b| java::string_compare_to(a.0.name(), b.0.name()));
        for (t, c) in m {
            component_style(sb, &format!("{pm}[{}]", t.name()), c);
        }
        line(sb, p, "displayName", &o(&u.display_name));
        line(sb, p, "description", &o(&u.description));
        line(sb, p, "buildDescription", &o(&u.build_description));
        line(sb, p, "icon", &texture(&u.icon));
    }
    fn style_info(sb: &mut String, p: &str, s: &StyleInfo) {
        match &s.default_style {
            None => line(sb, p, "defaultStyle", "null"),
            Some(d) => {
                line(sb, p, "defaultStyle", "object");
                style(sb, &format!("{p}\tdefaultStyle"), d);
            }
        }
        let pm = format!("{p}\tstyles");
        head(sb, &pm, s.styles.len());
        for (k, st) in s.styles.sorted() {
            style(sb, &format!("{pm}[{}]", e(k)), st);
        }
    }
    fn skin(sb: &mut String, p: &str, s: &XuiSkin, scripts: &[XuiSkinScript]) {
        let imports: Vec<String> = s.imports.iter().map(|n| format!("skin:{}", e(n))).collect();
        line(sb, p, "imports", &format!("[{}]", imports.join(", ")));
        color_map(sb, &format!("{p}\tcolorMap"), &s.color_map);
        line(sb, p, "defaultEntityUiStyle", "object");
        entity_style(
            sb,
            &format!("{p}\tdefaultEntityUiStyle"),
            &s.default_entity_ui_style,
        );
        let pm = format!("{p}\tentityUiStyleMap");
        head(sb, &pm, s.entity_ui_style_map.len());
        for (k, u) in s.entity_ui_style_map.sorted() {
            entity_style(sb, &format!("{pm}[{}]", e(k)), u);
        }
        let pm = format!("{p}\tstyles");
        head(sb, &pm, s.styles.len());
        for (k, i) in s.styles.sorted() {
            style_info(sb, &format!("{pm}[{}]", e(k)), i);
        }
        line(sb, p, "name", &e(&s.name));
        let script = scripts.get(s.script).map_or("null".into(), |x| {
            format!("script:{}", e(&x.base.get_script_object_full_type()))
        });
        line(sb, p, "script", &script);
        line(sb, p, "hasLoaded", &s.has_loaded.to_string());
        line(sb, p, "invalidated", &s.invalidated.to_string());
    }
    fn sorted_by_full<T>(v: &[T], base: impl Fn(&T) -> &BaseScriptObject) -> Vec<(String, &T)> {
        let mut l: Vec<(String, &T)> = v
            .iter()
            .map(|x| (base(x).get_script_object_full_type(), x))
            .collect();
        l.sort_by(|a, b| java::string_compare_to(&a.0, &b.0));
        l
    }
    let m = ScriptManager::load(root)?;
    let mut sb = String::new();
    let l = sorted_by_full(&m.xui_configs, |x| &x.base);
    head(&mut sb, "xuiConfig", l.len());
    for (full, x) in l {
        config(&mut sb, &format!("xuiConfig:{}", e(&full)), x);
    }
    for (tag, list) in [
        ("xuiLayout", &m.xui_layouts),
        ("xuiStyle", &m.xui_styles),
        ("xuiDefaultStyle", &m.xui_default_styles),
    ] {
        let l = sorted_by_full(list, |x| &x.base);
        head(&mut sb, tag, l.len());
        for (full, x) in l {
            layout(&mut sb, &format!("{tag}:{}", e(&full)), x);
        }
    }
    let l = sorted_by_full(&m.xui_colors, |x| &x.base);
    head(&mut sb, "xuiGlobalColors", l.len());
    for (full, x) in l {
        colors_script(&mut sb, &format!("xuiGlobalColors:{}", e(&full)), x);
    }
    let l = sorted_by_full(&m.xui_skin_scripts, |x| &x.base);
    head(&mut sb, "xuiSkin", l.len());
    for (full, x) in l {
        skin_script(&mut sb, &format!("xuiSkin:{}", e(&full)), x);
    }
    // XuiLuaStyle's static fields
    let x = &m.xui_manager;
    line(
        &mut sb,
        "XuiLuaStyle",
        "ALLOWED_VAR_TYPES",
        &set(ALLOWED_VAR_TYPES
            .iter()
            .map(|t| t.name().to_string())
            .collect()),
    );
    head(
        &mut sb,
        "XuiLuaStyle\tvarRegisteryMap",
        x.var_registery_map.len(),
    );
    for (k, v) in x.var_registery_map.sorted() {
        var(
            &mut sb,
            &format!("XuiLuaStyle\tvarRegisteryMap[{}]", e(k)),
            v,
        );
    }
    // XuiManager's static fields
    let p = "XuiManager";
    line(&mut sb, p, "DEFAULT_SKIN_NAME", DEFAULT_SKIN_NAME);
    line(
        &mut sb,
        p,
        "XUI_SCRIPT_TYPES",
        &set(XUI_SCRIPT_TYPES.iter().map(|t| t.to_string()).collect()),
    );
    for (f, map, list) in [
        ("layoutScriptsMap", &x.layout_scripts_map, &m.xui_layouts),
        ("stylesScriptsMap", &x.styles_scripts_map, &m.xui_styles),
        (
            "defaultStylesScriptsMap",
            &x.default_styles_scripts_map,
            &m.xui_default_styles,
        ),
    ] {
        let pm = format!("{p}\t{f}");
        head(&mut sb, &pm, map.len());
        for (k, i) in map.sorted() {
            let full = list.get(*i).map_or("null".into(), |s| {
                format!("script:{}", e(&s.base.get_script_object_full_type()))
            });
            let _ = writeln!(sb, "{pm}[{}]\t=\t{full}", e(k));
        }
    }
    for (f, names) in [
        ("combinedList", &x.combined_list),
        ("xuiLayoutsList", &x.xui_layouts_list),
        ("xuiStylesList", &x.xui_styles_list),
        ("xuiDefaultStylesList", &x.xui_default_styles_list),
    ] {
        let l: Vec<String> = names
            .iter()
            .map(|n| format!("xuiscript:{}", e(n)))
            .collect();
        line(&mut sb, p, f, &format!("[{}]", l.join(", ")));
    }
    for (f, map) in [
        ("xuiLayouts", &x.xui_layouts),
        ("xuiStyles", &x.xui_styles),
        ("xuiDefaultStyles", &x.xui_default_styles),
    ] {
        let pm = format!("{p}\t{f}");
        head(&mut sb, &pm, map.len());
        for (k, n) in map.sorted() {
            let _ = writeln!(sb, "{pm}[{}]\t=\txuiscript:{}", e(k), e(n));
        }
    }
    head(&mut sb, &format!("{p}\txuiSkins"), x.xui_skins.len());
    for (k, s) in x.xui_skins.sorted() {
        skin(&mut sb, &format!("skin:{}", e(k)), s, &m.xui_skin_scripts);
    }
    let def = x
        .xui_default_skin
        .as_ref()
        .map_or("null".into(), |n| format!("skin:{}", e(n)));
    line(&mut sb, p, "xuiDefaultSkin", &def);
    line(&mut sb, p, "parseOnce", &x.parse_once.to_string());
    line(&mut sb, p, "hasParsedOnce", &x.has_parsed_once.to_string());
    let d = x.get_default_skin();
    let _ = writeln!(
        sb,
        "{p}\tGetDefaultSkin\t{}\tsame\t{}",
        d.map_or("null".into(), |s| format!("skin:{}", e(&s.name))),
        d.is_some_and(|s| x.get_skin(Some(&s.name)).is_some_and(|t| t.name == s.name))
    );
    let names = m.colors.get_color_names();
    let _ = writeln!(
        sb,
        "Colors\tsize {}\t{}",
        names.len(),
        m.colors.get_colors_count()
    );
    for (i, n) in names.iter().enumerate() {
        let c = m
            .colors
            .get_color_by_name(n)
            .map_or("null".into(), |c| rgba(&c));
        let set = m
            .colors
            .get_color_info(n)
            .map_or("null", |i| i.color_set.name());
        let _ = writeln!(sb, "Colors[{i}]\t{}\t{c}\t{set}", e(n));
    }
    let cb = m.colors.cb_get_color_names();
    let _ = writeln!(sb, "CB\tsize {}", cb.len());
    for (i, n) in cb.iter().enumerate() {
        let c = m
            .colors
            .cb_get_color_by_name(n)
            .map_or("null".into(), |c| rgba(&c));
        let _ = writeln!(sb, "CB[{i}]\t{}\t{c}", e(n));
    }
    Ok(sb)
}

/// Mod loading over the crafted mods of tests/mods_cases.txt, pairs with ModsProbe.java: the commands run against
/// ChooseGameInfo, ZomboidFileSystem's mod side and ActiveMods, with `<scratch>/rust` as the cache dir (the probe's is
/// `<scratch>/java`; this one is pz-dump's own and is made afresh).
pub fn mods(root: &Path, scratch: &Path, commands: &Path) -> Result<String, String> {
    use zomboid_engine::zombie::core::game_version::{core_game_version, GameVersion};
    use zomboid_engine::zombie::core::translator::Translator;
    use zomboid_engine::zombie::game_states::choose_game_info::{
        get_min_required_version, ChooseGameInfo, Mod, TEXTURE_ID_USE_COMPRESSION_OPTION,
    };
    use zomboid_engine::zombie::modding::active_mods::{
        ActiveMods, ActiveModsError, ActiveModsState,
    };
    use zomboid_engine::zombie::zomboid_file_system::{
        canonical_path, PzFolder, PzModFolder, ZomboidFileSystem, CORE_OPTION_MODS_ENABLED,
    };

    struct Out {
        sb: String,
        cache: String,
        cache_canon: String,
        base: String,
    }
    impl Out {
        fn line(&mut self, s: &str) {
            self.sb.push_str(s);
            self.sb.push('\n');
        }
        fn p(&self, s: Option<&str>) -> String {
            let Some(s) = s else {
                return "null".into();
            };
            let mut s = s.replace(&self.cache, "$S");
            if self.cache_canon != self.cache {
                s = s.replace(&self.cache_canon, "$S");
            }
            let s = s.replace(&self.base, "$G");
            let mut b = String::from("\"");
            for c in s.encode_utf16() {
                if (0x20..0x7f).contains(&c) && c != 0x22 && c != 0x5c {
                    b.push(c as u8 as char);
                } else {
                    let _ = write!(b, "\\u{c:04x}");
                }
            }
            b.push('"');
            b
        }
        fn list<S: AsRef<str>>(&self, l: Option<&[S]>) -> String {
            let Some(l) = l else {
                return "null".into();
            };
            let items: Vec<String> = l.iter().map(|s| self.p(Some(s.as_ref()))).collect();
            format!("[{}]", items.join(", "))
        }
        fn sorted<'a>(&self, l: impl Iterator<Item = &'a String>) -> String {
            let mut v: Vec<&String> = l.collect();
            v.sort_by(|a, b| java::string_compare_to(a, b));
            self.list(Some(&v))
        }
        fn version(&self, v: Option<&GameVersion>) -> String {
            v.map_or("null".into(), |v| {
                format!("{} {}", self.p(Some(&v.to_string())), v.get_int())
            })
        }
        fn folder(&self, f: &PzFolder) -> String {
            format!(
                "{} {}",
                self.p(f.absolute_file.as_deref()),
                self.p(f.canonical_file.as_deref())
            )
        }
        fn mod_folder(&mut self, label: &str, f: &PzModFolder) {
            let l = format!(
                " {label} common {} version {}",
                self.folder(&f.common),
                self.folder(&f.version)
            );
            self.line(&l);
        }
        fn dump_mod(&mut self, m: &Mod) {
            let lines = vec![
                format!("MOD {}", self.p(Some(&m.dir))),
                format!(" commonDir {}", self.p(Some(&m.common_dir))),
                format!(" versionDir {}", self.p(Some(&m.version_dir))),
            ];
            for l in lines {
                self.line(&l);
            }
            self.mod_folder("baseFile", &m.base_file);
            self.mod_folder("mediaFile", &m.media_file);
            self.mod_folder("actionGroupsFile", &m.action_groups_file);
            self.mod_folder("animSetsFile", &m.anim_sets_file);
            self.mod_folder("animsXFile", &m.anims_x_file);
            let packs: Vec<String> = m
                .packs
                .iter()
                .map(|pf| format!("{} {}", self.p(Some(&pf.name)), pf.flags))
                .collect();
            let defs: Vec<String> = m
                .tile_defs
                .iter()
                .map(|td| format!("{} {}", self.p(Some(&td.name)), td.file_number))
                .collect();
            let lines = vec![
                format!(" posters {}", self.list(Some(&m.posters))),
                format!(" require {}", self.list(m.require.as_deref())),
                format!(" incompatible {}", self.list(m.incompatible.as_deref())),
                format!(" loadAfter {}", self.list(m.load_after.as_deref())),
                format!(" loadBefore {}", self.list(m.load_before.as_deref())),
                format!(" name {}", self.p(Some(&m.name))),
                format!(" desc {}", self.p(Some(&m.desc))),
                format!(" id {}", self.p(Some(&m.id))),
                format!(" url {}", self.p(m.url.as_deref())),
                format!(" author {}", self.p(m.author.as_deref())),
                format!(" modVersion {}", self.p(m.mod_version.as_deref())),
                format!(" icon {}", self.p(m.icon.as_deref())),
                format!(" category {}", self.p(m.category.as_deref())),
                format!(" workshopId {}", self.p(Some(&m.workshop_id))),
                format!(" source {}", self.p(Some(&m.source))),
                format!(" availableDone {}", m.available_done),
                format!(" available {}", m.available),
                format!(" versionMin {}", self.version(m.version_min.as_ref())),
                format!(" versionMax {}", self.version(m.version_max.as_ref())),
                format!(" packs [{}]", packs.join(", ")),
                format!(" tileDefs [{}]", defs.join(", ")),
                format!(" read {}", m.read),
                format!(" valid {}", m.valid),
            ];
            for l in lines {
                self.line(&l);
            }
        }
        fn active_text(&self, a: &ActiveMods) -> String {
            format!(
                "{} mods {} maps {}",
                self.p(Some(a.id())),
                self.list(Some(a.get_mods())),
                self.list(Some(a.get_map_order()))
            )
        }
    }
    // the command file's escapes, as bytes (\xHH a raw byte, the rest UTF-8; \u a UTF-16 unit in 3-byte form)
    fn unescape(s: &str) -> Vec<u8> {
        let mut out = Vec::new();
        let mut it = s.chars();
        while let Some(c) = it.next() {
            if c != '\\' {
                let mut buf = [0u8; 4];
                out.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
                continue;
            }
            match it.next() {
                Some('n') => out.push(b'\n'),
                Some('t') => out.push(b'\t'),
                Some('r') => out.push(b'\r'),
                Some('\\') => out.push(b'\\'),
                Some('x') => {
                    let h: String = it.by_ref().take(2).collect();
                    out.push(u8::from_str_radix(&h, 16).unwrap_or(0));
                }
                Some('u') => {
                    let h: String = it.by_ref().take(4).collect();
                    let u = u32::from_str_radix(&h, 16).unwrap_or(0);
                    if u < 0x80 {
                        out.push(u as u8);
                    } else if u < 0x800 {
                        out.extend_from_slice(&[0xc0 | (u >> 6) as u8, 0x80 | (u & 0x3f) as u8]);
                    } else {
                        out.extend_from_slice(&[
                            0xe0 | (u >> 12) as u8,
                            0x80 | ((u >> 6) & 0x3f) as u8,
                            0x80 | (u & 0x3f) as u8,
                        ]);
                    }
                }
                Some(e) => {
                    out.push(b'\\');
                    let mut buf = [0u8; 4];
                    out.extend_from_slice(e.encode_utf8(&mut buf).as_bytes());
                }
                None => out.push(b'\\'),
            }
        }
        out
    }
    type Exc = (&'static str, String);
    fn am(e: ActiveModsError) -> Exc {
        (e.class, e.message)
    }

    let cache_path = scratch.join("rust");
    if cache_path.exists() {
        std::fs::remove_dir_all(&cache_path)
            .map_err(|e| format!("{}: {e}", cache_path.display()))?;
    }
    std::fs::create_dir_all(&cache_path).map_err(|e| format!("{}: {e}", cache_path.display()))?;
    let mut fs = ZomboidFileSystem::init(root)?;
    fs.set_cache_dir(&cache_path.to_string_lossy());
    let mut tr = Translator::new(fs.base());
    let base_path = fs.base().to_path_buf();
    tr.languages.init(&base_path);
    let mut cgi = ChooseGameInfo::default();
    let mut active = ActiveModsState::default();
    let cache = fs.get_cache_dir().unwrap_or_default().to_string();
    let mut o = Out {
        sb: String::new(),
        cache_canon: canonical_path(&cache).unwrap_or_else(|| cache.clone()),
        cache,
        base: base_path.to_string_lossy().into_owned(),
    };
    let header = format!(
        "GAME version {} min {} compression {} modsEnabled {}",
        o.version(Some(&core_game_version())),
        o.version(Some(&get_min_required_version())),
        TEXTURE_ID_USE_COMPRESSION_OPTION,
        CORE_OPTION_MODS_ENABLED
    );
    o.line(&header);
    let text =
        std::fs::read_to_string(commands).map_err(|e| format!("{}: {e}", commands.display()))?;
    let cache = o.cache.clone();
    for l in java::buffered_reader_lines(&text) {
        if l.is_empty() || l.starts_with('#') {
            continue;
        }
        let f: Vec<&str> = l.split('\t').collect();
        let arg = |field: Option<&&str>| {
            String::from_utf8_lossy(&unescape(field.copied().unwrap_or(""))).replace("$S", &cache)
        };
        let (a1, a2) = (arg(f.get(1)), arg(f.get(2)));
        let r: Result<(), Exc> = (|| {
            match f[0] {
                "lang" => {
                    let l = tr.languages.get_by_name(Some(&a1));
                    tr.set_language(l);
                    let name = tr.get_language().name.clone();
                    let s = format!("LANG {} -> {}", o.p(Some(&a1)), o.p(Some(&name)));
                    o.line(&s);
                }
                "gv" => {
                    let v = GameVersion::parse(&a1).map_err(|e| ("IllegalArgumentException", e))?;
                    let s = format!("GV {} -> {}", o.p(Some(&a1)), o.version(Some(&v)));
                    o.line(&s);
                }
                "vint" => {
                    let s = format!(
                        "VINT {} -> {}",
                        o.p(Some(&a1)),
                        fs.get_game_version_int_from_name(&a1)
                    );
                    o.line(&s);
                }
                "file" => {
                    let path = cache_path.join(f[1]);
                    if let Some(parent) = path.parent() {
                        std::fs::create_dir_all(parent)
                            .map_err(|e| ("IOException", e.to_string()))?;
                    }
                    std::fs::write(&path, unescape(f.get(2).copied().unwrap_or("")))
                        .map_err(|e| ("IOException", e.to_string()))?;
                }
                "dir" => {
                    std::fs::create_dir_all(cache_path.join(f[1]))
                        .map_err(|e| ("IOException", e.to_string()))?;
                }
                "order" => {
                    fs.set_mod_folders_order(&a1);
                    fs.reset_mod_folders();
                    let s = format!("ORDER {}", o.p(Some(&a1)));
                    o.line(&s);
                }
                "folders" => {
                    let out = fs.get_all_mod_folders();
                    let s = format!("FOLDERS {}", out.len());
                    o.line(&s);
                    for d in &out {
                        let s = format!(" {}", o.p(Some(d)));
                        o.line(&s);
                    }
                }
                "details" | "available" | "read" => {
                    let m = match f[0] {
                        "details" => cgi.get_mod_details(&mut fs, &mut tr, &a1),
                        "available" => cgi.get_available_mod_details(&mut fs, &mut tr, &a1),
                        _ => cgi.read_mod_info(&mut fs, &mut tr, Some(&a1)),
                    };
                    let dir = m.map(|m| fs.mod_info(m).dir.clone());
                    let s = format!(
                        "{} {} -> {}",
                        f[0].to_uppercase(),
                        o.p(Some(&a1)),
                        o.p(dir.as_deref())
                    );
                    o.line(&s);
                }
                "verdir" => {
                    let d = fs.get_mod_version_dir_name(&java::file_normalize(&a1));
                    let s = format!("VERDIR {} -> {}", o.p(Some(&a1)), o.p(Some(&d)));
                    o.line(&s);
                }
                "active" => {
                    let i = active.get_by_id(&a1).map_err(am)?;
                    let b = java::equals_ignore_case(&arg(f.get(3)), "true");
                    active.get_mut(i).set_mod_active(&a2, b);
                    let s = format!(
                        "ACTIVE {} {} {} -> {}",
                        o.p(Some(&a1)),
                        o.p(Some(&a2)),
                        f.get(3).copied().unwrap_or(""),
                        o.active_text(active.get(i))
                    );
                    o.line(&s);
                }
                "isactive" => {
                    let i = active.get_by_id(&a1).map_err(am)?;
                    let s = format!(
                        "ISACTIVE {} {} -> {}",
                        o.p(Some(&a1)),
                        o.p(Some(&a2)),
                        active.get(i).is_mod_active(&a2)
                    );
                    o.line(&s);
                }
                "remove" => {
                    let i = active.get_by_id(&a1).map_err(am)?;
                    active.get_mut(i).remove_mod(&a2);
                    let s = format!(
                        "REMOVE {} {} -> {}",
                        o.p(Some(&a1)),
                        o.p(Some(&a2)),
                        o.active_text(active.get(i))
                    );
                    o.line(&s);
                }
                "checkmissing" => {
                    let i = active.get_by_id(&a1).map_err(am)?;
                    active.check_missing_mods(i, &mut cgi, &mut fs, &mut tr);
                    let s = format!(
                        "CHECKMISSING {} -> {}",
                        o.p(Some(&a1)),
                        o.active_text(active.get(i))
                    );
                    o.line(&s);
                }
                "lists" => {
                    let s = format!("LISTS {}", active.count());
                    o.line(&s);
                    for a in active.all() {
                        let s = format!(" {}", o.active_text(a));
                        o.line(&s);
                    }
                    let s = format!(" loaded {}", o.active_text(active.loaded()));
                    o.line(&s);
                }
                "load" => {
                    let i = active.get_by_id(&a1).map_err(am)?;
                    let list = active.get(i).get_mods().to_vec();
                    fs.load_mods(&mut cgi, &mut tr, &list);
                    let s = format!(
                        "LOAD {} -> {}",
                        o.p(Some(&a1)),
                        o.list(Some(fs.get_mod_ids()))
                    );
                    o.line(&s);
                }
                "loadstr" => {
                    fs.load_mods_by_id(&mut cgi, &mut tr, &mut active, &a1)
                        .map_err(am)?;
                    let s = format!(
                        "LOADSTR {} -> {}",
                        o.p(Some(&a1)),
                        o.list(Some(fs.get_mod_ids()))
                    );
                    o.line(&s);
                }
                "get" => {
                    let s = match fs.get_string(&a1) {
                        None => format!("GET {} unchanged", o.p(Some(&a1))),
                        Some(r) => format!("GET {} -> {}", o.p(Some(&a1)), o.p(Some(r))),
                    };
                    o.line(&s);
                }
                "dirget" => {
                    let s = format!(
                        "DIRGET {} -> {}",
                        o.p(Some(&a1)),
                        o.p(Some(&fs.get_directory_string(&a1)))
                    );
                    o.line(&s);
                }
                "known" => {
                    let s = format!("KNOWN {} {}", o.p(Some(&a1)), fs.is_known_file(&a1));
                    o.line(&s);
                }
                "abs" => {
                    let s = format!(
                        "ABS {} -> {}",
                        o.p(Some(&a1)),
                        o.p(fs.get_absolute_path(&a1))
                    );
                    o.line(&s);
                }
                "map" => {
                    let s = format!(
                        "MAP size {} abs {}",
                        fs.active_file_map.len(),
                        fs.all_absolute_paths_len()
                    );
                    o.line(&s);
                    let mut entries: Vec<(&String, &String)> = fs
                        .active_file_map
                        .iter()
                        .filter(|(_, v)| v.starts_with(&o.cache))
                        .collect();
                    entries.sort_by(|a, b| java::string_compare_to(a.0, b.0));
                    for (k, v) in entries {
                        let s = format!(" {} -> {}", o.p(Some(k)), o.p(Some(v)));
                        o.line(&s);
                    }
                }
                "state" => {
                    o.line("STATE");
                    let s = format!(" Mods {}", cgi.mods.len());
                    o.line(&s);
                    let mut ids: Vec<(&String, &usize)> = cgi.mods.iter().collect();
                    ids.sort_by(|a, b| java::string_compare_to(a.0, b.0));
                    for (k, m) in ids {
                        let s =
                            format!("  {} -> {}", o.p(Some(k)), o.p(Some(&fs.mod_info(*m).dir)));
                        o.line(&s);
                    }
                    let s = format!(" MissingMods {}", o.sorted(cgi.missing_mods.iter()));
                    o.line(&s);
                    let s = format!(" tempStrings {}", o.list(Some(&cgi.temp_strings)));
                    o.line(&s);
                    let s = format!(" modIdToDir {}", fs.mod_id_to_dir().len());
                    o.line(&s);
                    let mut dirs: Vec<(&String, &String)> = fs.mod_id_to_dir().iter().collect();
                    dirs.sort_by(|a, b| java::string_compare_to(a.0, b.0));
                    for (k, d) in dirs {
                        let s = format!("  {} -> {}", o.p(Some(k)), o.p(Some(d)));
                        o.line(&s);
                    }
                    let s = format!(" modDirToMod {}", o.sorted(fs.mod_dir_to_mod().keys()));
                    o.line(&s);
                    let s = format!(" mods {}", o.list(Some(fs.get_mod_ids())));
                    o.line(&s);
                    let s = format!(" modFolders {}", o.list(fs.mod_folders()));
                    o.line(&s);
                    let s = format!(" modFoldersOrder {}", o.list(fs.mod_folders_order()));
                    o.line(&s);
                }
                "mods" => {
                    let s = format!("MODS {}", fs.mod_dir_to_mod().len());
                    o.line(&s);
                    let mut dirs: Vec<(&String, &usize)> = fs.mod_dir_to_mod().iter().collect();
                    dirs.sort_by(|a, b| java::string_compare_to(a.0, b.0));
                    let handles: Vec<usize> = dirs.into_iter().map(|(_, h)| *h).collect();
                    for h in handles {
                        let m = fs.mod_info(h).clone();
                        o.dump_mod(&m);
                    }
                }
                other => {
                    return Err((
                        "IllegalArgumentException",
                        format!("unknown command {other}"),
                    ))
                }
            }
            Ok(())
        })();
        if let Err((class, message)) = r {
            let s = format!("EXC {} {class}: {}", f[0], o.p(Some(&message)));
            o.line(&s);
        }
    }
    Ok(o.sb)
}

/// zombie.GameSounds over the booted server's state, pairs with SoundsProbe.java: the `sound` scripts as
/// ScriptManager loaded them, the registry `GameSounds.ScriptsLoaded` filled, then (with `crafted` = the command
/// file of tests/sounds_cases.txt and a scratch folder for the `sounds.ini` those commands write) the crafted
/// GameSoundScript.Load bodies and GameSounds calls on the emptied registry.
pub fn sounds(root: &Path, crafted: Option<(&Path, &Path)>) -> Result<String, String> {
    use std::collections::HashMap;
    use zomboid_engine::zombie::audio::game_sound::{GameSound, GameSoundClip};
    use zomboid_engine::zombie::core::random::rand_abstract::RandAbstract;
    use zomboid_engine::zombie::game_sounds::GameSounds;
    use zomboid_engine::zombie::scripting::objects::base_script_object::{
        BaseScriptObject, ScriptObject,
    };
    use zomboid_engine::zombie::scripting::objects::game_sound_script::GameSoundScript;
    use zomboid_engine::zombie::scripting::objects::sound_timeline_script::SoundTimelineScript;
    use zomboid_engine::zombie::scripting::script_manager::ScriptManager;
    use zomboid_engine::zombie::vehicles::vehicle_engine_rpm::VehicleEngineRPM;

    /// SoundsProbe.esc: '\\', tab, LF, CR and every UTF-16 unit outside 0x20..0x7e escaped.
    fn esc(s: &str) -> String {
        let mut sb = String::new();
        for u in s.encode_utf16() {
            match u {
                0x5c => sb.push_str("\\\\"),
                0x09 => sb.push_str("\\t"),
                0x0a => sb.push_str("\\n"),
                0x0d => sb.push_str("\\r"),
                0x20..=0x7e => sb.push(char::from(u as u8)),
                _ => {
                    let _ = write!(sb, "\\u{u:04x}");
                }
            }
        }
        sb
    }
    fn opt(s: &Option<String>) -> String {
        s.as_deref().map_or("null".to_string(), esc)
    }
    /// SoundsProbe.unescape
    fn unescape(s: &str) -> String {
        let c: Vec<char> = s.chars().collect();
        let mut sb = String::new();
        let mut i = 0;
        while i < c.len() {
            if c[i] != '\\' || i + 1 >= c.len() {
                sb.push(c[i]);
                i += 1;
                continue;
            }
            i += 1;
            match c[i] {
                'n' => sb.push('\n'),
                't' => sb.push('\t'),
                'r' => sb.push('\r'),
                '\\' => sb.push('\\'),
                'u' => {
                    let hex: String = c[i + 1..i + 5].iter().collect();
                    sb.push(
                        u32::from_str_radix(&hex, 16)
                            .ok()
                            .and_then(char::from_u32)
                            .unwrap_or('\u{fffd}'),
                    );
                    i += 4;
                }
                other => {
                    sb.push('\\');
                    sb.push(other);
                }
            }
            i += 1;
        }
        sb
    }
    fn dump_clip(
        sb: &mut String,
        path: &str,
        owner: &GameSound,
        clip: &GameSoundClip,
        advanced: bool,
    ) {
        let f = java::float_to_string;
        let _ = writeln!(sb, "{path}\tgameSound\tsound:{}", opt(&owner.name));
        let _ = writeln!(sb, "{path}\tevent\t{}", opt(&clip.event));
        let _ = writeln!(sb, "{path}\teventDescription\tnull");
        let _ = writeln!(sb, "{path}\teventDescriptionMp\tnull");
        let _ = writeln!(sb, "{path}\tfile\t{}", opt(&clip.file));
        let _ = writeln!(sb, "{path}\tvolume\t{}", f(clip.volume));
        let _ = writeln!(sb, "{path}\tpitch\t{}", f(clip.pitch));
        let _ = writeln!(sb, "{path}\tdistanceMin\t{}", f(clip.distance_min));
        let _ = writeln!(sb, "{path}\tdistanceMax\t{}", f(clip.distance_max));
        let _ = writeln!(sb, "{path}\treverbMaxRange\t{}", f(clip.reverb_max_range));
        let _ = writeln!(sb, "{path}\treverbFactor\t{}", f(clip.reverb_factor));
        let _ = writeln!(sb, "{path}\tpriority\t{}", clip.priority);
        let _ = writeln!(sb, "{path}\tinitFlags\t{}", clip.init_flags);
        let _ = writeln!(sb, "{path}\treloadEpoch\t{}", clip.reload_epoch);
        let effective = f(clip.get_effective_volume(owner, advanced));
        let _ = writeln!(
            sb,
            "{path}\tderived\thasMin={} hasMax={} stop={} effective={effective} effectiveMenu={effective}",
            clip.has_min_distance(),
            clip.has_max_distance(),
            clip.is_stop_immediate()
        );
    }
    fn dump_sound(sb: &mut String, path: &str, g: &GameSound, advanced: bool) {
        let f = java::float_to_string;
        let _ = writeln!(sb, "{path}\tname\t{}", opt(&g.name));
        let _ = writeln!(sb, "{path}\tcategory\t{}", esc(&g.category));
        let _ = writeln!(sb, "{path}\tloop\t{}", g.r#loop);
        let _ = writeln!(sb, "{path}\tis3d\t{}", g.is_3d);
        let _ = writeln!(sb, "{path}\tclips\tsize {}", g.clips.len());
        for (i, clip) in g.clips.iter().enumerate() {
            dump_clip(sb, &format!("{path}.clips[{i}]"), g, clip, advanced);
        }
        let _ = writeln!(sb, "{path}\tuserVolume\t{}", f(g.user_volume()));
        let _ = writeln!(sb, "{path}\tmaster\t{}", g.master.name());
        let _ = writeln!(
            sb,
            "{path}\tmaxInstancesPerEmitter\t{}",
            g.max_instances_per_emitter
        );
        let _ = writeln!(sb, "{path}\treloadEpoch\t{}", g.reload_epoch);
        let _ = writeln!(
            sb,
            "{path}\tderived\tmaxDistance={} master={} looped={} userVolume={}",
            f(g.get_max_distance_of_clips()),
            g.master.name(),
            g.r#loop,
            f(g.get_user_volume(advanced))
        );
    }
    fn base_line(sb: &mut String, path: &str, b: &BaseScriptObject) {
        let _ = writeln!(
            sb,
            "{path}\tmodule\t{}\tenabled\t{}\tdebugOnly\t{}\tscriptVersion\t{:x}\tbodies\t{}",
            opt(&b.module),
            b.enabled,
            b.debug_only,
            b.script_version,
            b.get_loaded_script_body_count()
        );
    }
    fn dump_script(sb: &mut String, path: &str, s: &GameSoundScript, advanced: bool) {
        base_line(sb, path, &s.base);
        dump_sound(sb, &format!("{path}.gameSound"), &s.game_sound, advanced);
    }
    /// SoundsProbe.dumpTimeline
    fn dump_timeline(sb: &mut String, path: &str, t: &SoundTimelineScript) {
        base_line(sb, path, &t.base);
        let _ = writeln!(sb, "{path}\teventName\t{}", opt(&t.event_name));
        let mut entries: Vec<(&String, &i32)> = t.position_by_name.iter().collect();
        entries.sort_by(|a, b| java::string_compare_to(a.0, b.0));
        let _ = writeln!(sb, "{path}\tpositionByName\tsize {}", entries.len());
        for (k, v) in entries {
            let _ = writeln!(sb, "{path}.positionByName[{}]\t=\t{v}", esc(k));
        }
    }
    /// SoundsProbe.dumpRpm
    fn dump_rpm(sb: &mut String, path: &str, r: &VehicleEngineRPM) {
        base_line(sb, path, &r.base);
        let _ = writeln!(sb, "{path}\tname\t{}", opt(&r.name));
        let _ = writeln!(sb, "{path}\trpmData\tsize {}", r.rpm_data.len());
        for (i, d) in r.rpm_data.iter().enumerate() {
            match d {
                None => {
                    let _ = writeln!(sb, "{path}.rpmData[{i}]\tnull");
                }
                Some(d) => {
                    let _ = writeln!(
                        sb,
                        "{path}.rpmData[{i}]\tgearChange\t{}\tafterGearChange\t{}",
                        java::float_to_string(d.gear_change),
                        java::float_to_string(d.after_gear_change)
                    );
                }
            }
        }
    }
    fn position(gs: &GameSounds, index: usize) -> i64 {
        gs.list_position(index).map_or(-1, |p| p as i64)
    }
    fn names(gs: &GameSounds, list: &[usize]) -> String {
        let p: Vec<String> = list.iter().map(|&i| position(gs, i).to_string()).collect();
        format!("[{}]", p.join(", "))
    }
    /// SoundsProbe.registry
    fn registry(sb: &mut String, path: &str, gs: &GameSounds, full: bool, advanced: bool) {
        let _ = writeln!(sb, "{path}\tsounds\tsize {}", gs.sounds.len());
        if full {
            for (i, &index) in gs.sounds.iter().enumerate() {
                dump_sound(
                    sb,
                    &format!("{path}.sounds[{i}]"),
                    gs.sound(index),
                    advanced,
                );
            }
        }
        let mut keys: Vec<&String> = gs.sound_by_name.keys().collect();
        keys.sort_by(|a, b| java::string_compare_to(a, b));
        let _ = writeln!(sb, "{path}\tsoundByName\tsize {}", keys.len());
        for key in keys {
            let _ = writeln!(
                sb,
                "{path}.soundByName[{}]\t=\tsounds[{}]",
                esc(key),
                position(gs, gs.sound_by_name[key])
            );
        }
        let _ = writeln!(sb, "{path}\tsoundIsPaused\t{}", gs.sound_is_paused);
        let categories = gs.get_categories();
        let escaped: Vec<String> = categories.iter().map(|c| esc(c)).collect();
        let _ = writeln!(sb, "{path}\tgetCategories\t[{}]", escaped.join(", "));
        for c in &categories {
            let _ = writeln!(
                sb,
                "{path}.getSoundsInCategory[{}]\t{}",
                esc(c),
                names(gs, &gs.get_sounds_in_category(c))
            );
        }
    }

    let sm = ScriptManager::load(root)?;
    let mut gs = sm.game_sounds.clone();
    let mut sb = String::new();
    let mut advanced = false;
    let _ = writeln!(sb, "advancedSoundOptions\t{advanced}");
    // BaseSoundBank.instance is only set by GameWindow (not by GameServer).
    let _ = writeln!(sb, "soundBank\tnull");
    let _ = writeln!(sb, "scripts\tsize {}", sm.sounds.len());
    for (i, s) in sm.sounds.iter().enumerate() {
        dump_script(&mut sb, &format!("script[{i}]"), s, advanced);
    }
    registry(&mut sb, "registry", &gs, true, advanced);
    let _ = writeln!(sb, "soundTimelines\tsize {}", sm.sound_timelines.len());
    for (i, t) in sm.sound_timelines.iter().enumerate() {
        dump_timeline(&mut sb, &format!("timeline[{i}]"), t);
    }
    let _ = writeln!(
        sb,
        "vehicleEngineRPMs\tsize {}",
        sm.vehicle_engine_rpms.len()
    );
    for (i, r) in sm.vehicle_engine_rpms.iter().enumerate() {
        dump_rpm(&mut sb, &format!("rpm[{i}]"), r);
    }
    let Some((cases, scratch)) = crafted else {
        return Ok(sb);
    };
    gs.reset();
    let text =
        String::from_utf8(std::fs::read(cases).map_err(|e| format!("{}: {e}", cases.display()))?)
            .map_err(|e| e.to_string())?;
    std::fs::create_dir_all(scratch).map_err(|e| format!("{}: {e}", scratch.display()))?;
    let cache = scratch.to_str().ok_or("non-UTF-8 scratch path")?;
    let mut scripts: HashMap<String, GameSoundScript> = HashMap::new();
    let mut timelines: HashMap<String, SoundTimelineScript> = HashMap::new();
    let mut rpms: HashMap<String, VehicleEngineRPM> = HashMap::new();
    let mut n = 0;
    for line in java::buffered_reader_lines(&text) {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        n += 1;
        let (cmd, rest) = line.split_once(' ').unwrap_or((line, ""));
        let at = format!("cmd[{n}]");
        let _ = writeln!(sb, "{at}\t> {}", esc(line));
        match cmd {
            "load" => {
                let mut a = rest.splitn(3, ' ');
                let label = a.next().unwrap_or("");
                let name = a.next().ok_or("load: no name")?;
                let body = a.next().map(unescape).unwrap_or_default();
                let script = scripts
                    .entry(label.to_string())
                    .or_insert_with(|| GameSoundScript::new(None));
                if let Err(e) = script.load(name, &body, &sm.translator) {
                    let _ = writeln!(sb, "{at}\texception\t{}", esc(&e));
                }
                dump_script(&mut sb, &format!("{at}.script"), script, advanced);
            }
            "timeline" => {
                let mut a = rest.splitn(3, ' ');
                let label = a.next().unwrap_or("");
                let name = a.next().ok_or("timeline: no name")?;
                let body = a.next().map(unescape).unwrap_or_default();
                let t = timelines
                    .entry(label.to_string())
                    .or_insert_with(|| SoundTimelineScript::new(None));
                if let Err(e) = t.load(name, &body, &sm.translator) {
                    let _ = writeln!(sb, "{at}\texception\t{}", esc(&e));
                }
                dump_timeline(&mut sb, &format!("{at}.timeline"), t);
            }
            "timelinepos" => {
                let a: Vec<&str> = rest.split(' ').collect();
                let t = timelines.get(a[0]).ok_or("timelinepos: no script")?;
                let _ = writeln!(sb, "{at}\tgetPosition\t{}", t.get_position(a[1]));
            }
            "timelinereset" => {
                let t = timelines.get_mut(rest).ok_or("timelinereset: no script")?;
                t.reset();
                dump_timeline(&mut sb, &format!("{at}.timeline"), t);
            }
            "rpm" => {
                let mut a = rest.splitn(3, ' ');
                let label = a.next().unwrap_or("");
                let name = a.next().ok_or("rpm: no name")?;
                let body = a.next().map(unescape).unwrap_or_default();
                let r = rpms
                    .entry(label.to_string())
                    .or_insert_with(|| VehicleEngineRPM::new(None));
                if let Err(e) = r.load(name, &body, &sm.translator) {
                    let _ = writeln!(sb, "{at}\texception\t{}", esc(&e));
                }
                dump_rpm(&mut sb, &format!("{at}.rpm"), r);
            }
            "rpmreset" => {
                let r = rpms.get_mut(rest).ok_or("rpmreset: no script")?;
                r.reset();
                dump_rpm(&mut sb, &format!("{at}.rpm"), r);
            }
            "scriptreset" => match scripts.get_mut(rest) {
                None => {
                    let _ = writeln!(sb, "{at}\tno such script");
                }
                Some(s) => {
                    s.reset();
                    dump_script(&mut sb, &format!("{at}.script"), s, advanced);
                }
            },
            "add" => {
                match scripts.remove(rest) {
                    None => {
                        let _ = writeln!(sb, "{at}\tno such script");
                    }
                    Some(s) => {
                        if let Err(e) = gs.add_sound(s.game_sound) {
                            let _ = writeln!(sb, "{at}\texception\t{}", esc(&e));
                        }
                    }
                }
                registry(&mut sb, &at, &gs, false, advanced);
            }
            "get" | "getnull" => {
                let name = if cmd == "getnull" { None } else { Some(rest) };
                match gs.get_sound(name) {
                    None => {
                        let _ = writeln!(sb, "{at}\tresult\tnull");
                    }
                    Some(index) => {
                        let _ = writeln!(sb, "{at}\tresult\tsounds[{}]", position(&gs, index));
                        dump_sound(&mut sb, &format!("{at}.sound"), gs.sound(index), advanced);
                    }
                }
                registry(&mut sb, &at, &gs, false, advanced);
            }
            "known" => {
                let _ = writeln!(sb, "{at}\tisKnownSound\t{}", gs.is_known_sound(Some(rest)));
            }
            "knownnull" => {
                let _ = writeln!(sb, "{at}\tisKnownSound\t{}", gs.is_known_sound(None));
            }
            "cat" => {
                let _ = writeln!(
                    sb,
                    "{at}\tgetSoundsInCategory\t{}",
                    names(&gs, &gs.get_sounds_in_category(rest))
                );
            }
            "volume" => {
                let a: Vec<&str> = rest.split(' ').collect();
                let index = gs.get_sound(Some(a[0])).ok_or("volume: no sound")?;
                gs.sound_mut(index)
                    .set_user_volume(java::parse_float(a[1]).ok_or("volume: bad float")?);
                let g = gs.sound(index);
                let _ = writeln!(
                    sb,
                    "{at}\tuserVolume\t{}\tgetUserVolume\t{}",
                    java::float_to_string(g.user_volume()),
                    java::float_to_string(g.get_user_volume(advanced))
                );
                dump_sound(&mut sb, &format!("{at}.sound"), g, advanced);
            }
            "advanced" => advanced = java::equals_ignore_case(rest, "true"),
            "rand" => {
                let a: Vec<&str> = rest.split(' ').collect();
                let index = gs.get_sound(Some(a[0])).ok_or("rand: no sound")?;
                let seed: i64 = a[1].parse().map_err(|_| "rand: bad seed")?;
                let count: i32 = a[2].parse().map_err(|_| "rand: bad count")?;
                let mut rand = RandAbstract::new(java::Random::new(seed));
                for _ in 0..count {
                    match gs.sound(index).get_random_clip(&mut rand) {
                        Ok(i) => {
                            let _ = writeln!(sb, "{at}\tgetRandomClip\t{i}");
                        }
                        Err(e) => {
                            let _ = writeln!(sb, "{at}\tgetRandomClip\texception\t{}", esc(&e));
                        }
                    }
                }
            }
            "ini" => {
                let file = scratch.join("sounds.ini");
                std::fs::write(&file, unescape(rest))
                    .map_err(|e| format!("{}: {e}", file.display()))?;
                gs.load_ini(cache);
                for (i, &index) in gs.sounds.iter().enumerate() {
                    let _ = writeln!(
                        sb,
                        "{at}.sounds[{i}]\tuserVolume\t{}",
                        java::float_to_string(gs.sound(index).user_volume())
                    );
                }
            }
            "reset" => gs.reset(),
            "state" => registry(&mut sb, &at, &gs, true, advanced),
            _ => {
                let _ = writeln!(sb, "{at}\tunknown command");
            }
        }
    }
    Ok(sb)
}

/// The global object systems of a save's `gos_*.bin` files, the ones the probe's own writer made, hand-made broken
/// files and the writer's file cut at every length (pairs with GosProbe.java): what `SGlobalObjectSystem.load` read
/// and what the shared `GlobalObjectLookup` says about each object.
pub fn gos(root: &Path, save: &Path, crafted: &Path) -> Result<String, String> {
    use zomboid_engine::java::string_hash_code;
    use zomboid_engine::java_nio::ByteBuffer;
    use zomboid_engine::zombie::global_objects::{MetaGridBounds, SGlobalObjects};
    let media = root.join("media");
    let mut g = IsoMetaGrid::default();
    let dirs = IsoMetaGrid::lot_directories(&media, "Muldraugh, KY");
    g.create_step1(&media, &dirs, (-250, -250, 250, 250))?;
    let mut reg = SGlobalObjects::new(MetaGridBounds {
        min_x: g.min_x,
        min_y: g.min_y,
        width: g.grid_x(),
        height: g.grid_y(),
    });
    let mut sb = String::new();
    let _ = writeln!(
        sb,
        "GRID {} {} {} {}",
        g.min_x,
        g.min_y,
        g.grid_x(),
        g.grid_y()
    );
    fn sorted_files(dir: &Path, prefix: &str) -> Result<Vec<(String, std::path::PathBuf)>, String> {
        let mut v = Vec::new();
        for e in std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))? {
            let e = e.map_err(|e| e.to_string())?;
            let n = e.file_name().to_string_lossy().into_owned();
            if n.starts_with(prefix) && n.ends_with(".bin") {
                v.push((n, e.path()));
            }
        }
        v.sort();
        Ok(v)
    }
    fn list(l: &[usize]) -> String {
        if l.is_empty() {
            return "-".to_string();
        }
        l.iter().map(usize::to_string).collect::<Vec<_>>().join(",")
    }
    fn at(i: Option<usize>) -> i64 {
        i.map_or(-1, |i| i as i64)
    }
    let dump = |sb: &mut String, reg: &SGlobalObjects, label: &str, sys: usize| {
        let s = &reg.systems[sys];
        let _ = writeln!(
            sb,
            "SYS {label} v={} n={} md={}",
            s.loaded_world_version,
            s.objects.len(),
            save_dump::table(&s.mod_data)
        );
        for (i, o) in s.objects.iter().enumerate() {
            let _ = writeln!(
                sb,
                "OBJ {label} {i} {} {} {} md={}",
                o.x,
                o.y,
                o.z,
                save_dump::table(&o.mod_data)
            );
            let _ = writeln!(
                sb,
                "AT {label} {i} {} {}",
                at(reg.get_object_at(sys, o.x, o.y, o.z)),
                at(reg.get_object_at(sys, o.x, o.y, o.z.wrapping_add(1)))
            );
            let (wx, wy) = (o.x / 8, o.y / 8);
            let _ = writeln!(
                sb,
                "CHK {label} {i} {wx} {wy} {} {}",
                u8::from(reg.has_objects_in_chunk(sys, wx, wy)),
                list(&reg.get_objects_in_chunk(sys, wx, wy))
            );
            let _ = writeln!(
                sb,
                "ADJ {label} {i} {}",
                list(&reg.get_objects_adjacent_to(sys, o.x, o.y, o.z))
            );
        }
    };
    let hash = |s: &str| format!("{:x}", string_hash_code(s));

    // 1. the real files
    for (n, p) in sorted_files(save, "gos_")? {
        let name = &n[4..n.len() - 4];
        let data = std::fs::read(&p).map_err(|e| format!("{}: {e}", p.display()))?;
        let label = format!("real_{name}");
        let sys = reg.new_system(&label)?;
        if let Err(e) = reg.load_file(sys, Some(&data)) {
            eprintln!("{label}: {e}");
        }
        dump(&mut sb, &reg, &label, sys);
    }
    // 3. the crafted dir (2. is the probe's writer: its file is file_craft.bin there)
    let mut craft_bytes: Option<Vec<u8>> = None;
    for (n, p) in sorted_files(crafted, "")? {
        let data = std::fs::read(&p).map_err(|e| format!("{}: {e}", p.display()))?;
        let base = &n[n.find('_').map_or(0, |i| i + 1)..n.len() - 4];
        if n.starts_with("file_") {
            if n == "file_craft.bin" {
                craft_bytes = Some(data.clone());
            }
            let label = format!("f_{base}");
            let sys = reg.new_system(&label)?;
            if let Err(e) = reg.load_file(sys, Some(&data)) {
                eprintln!("{label}: {e}");
            }
            dump(&mut sb, &reg, &label, sys);
        } else if n.starts_with("payload_") {
            for v in [249, 100, 25] {
                let label = format!("p_{base}_{v}");
                let sys = reg.new_system(&label)?;
                let err = reg
                    .load(sys, &mut ByteBuffer::wrap(&data), v)
                    .err()
                    .unwrap_or_else(|| "-".to_string());
                let _ = writeln!(sb, "PAYLOAD {base} {v} err={err}");
                dump(&mut sb, &reg, &label, sys);
            }
        }
    }
    let craft_bytes = craft_bytes.ok_or("no file_craft.bin in the crafted folder")?;
    let payload = craft_bytes
        .get(8..)
        .ok_or("file_craft.bin is shorter than its header")?;
    for k in 0..=payload.len() {
        let sys = reg.new_system(&format!("t_{k}"))?;
        let err = reg
            .load(sys, &mut ByteBuffer::wrap(&payload[..k]), 249)
            .err()
            .unwrap_or_else(|| "-".to_string());
        let s = &reg.systems[sys];
        let mut objs = String::new();
        for o in &s.objects {
            let _ = write!(
                objs,
                "{},{},{},{};",
                o.x,
                o.y,
                o.z,
                save_dump::table(&o.mod_data)
            );
        }
        let _ = writeln!(
            sb,
            "TR {k} err={err} v={} n={} md={} objs={}",
            s.loaded_world_version,
            s.objects.len(),
            hash(&save_dump::table(&s.mod_data)),
            hash(&objs)
        );
    }
    Ok(sb)
}

/// Four small world-level save files read by their loaders, the player's real files, the files the original
/// writers made from values chosen in the probe, hand-made broken ones and the first writer file cut at every length
/// (pairs with SaveMetaProbe.java): `map_worldgen.bin`, `map_symbols.bin`, `statistics.bin`, `metadata.bin`.
pub fn savemeta(save: &Path, crafted: &Path) -> Result<String, String> {
    use zomboid_engine::java::{string_compare_to, string_hash_code};
    use zomboid_engine::zombie::iso::meta_tracker::{self, Alarm, Helicopter};
    use zomboid_engine::zombie::iso::worldgen::world_gen_params::{LoadResult, WorldGenParams};
    use zomboid_engine::zombie::statistics::StatisticsManager;
    use zomboid_engine::zombie::world_map::symbols::world_map_symbols::{
        WorldMapSymbol, WorldMapSymbols,
    };
    /// The probe's `esc`: printable ASCII except quote and backslash stays, any other UTF-16 unit is `\uXXXX`.
    fn esc(s: &str) -> String {
        let mut o = String::from("\"");
        for u in s.encode_utf16() {
            if (0x20..0x7f).contains(&u) && u != u16::from(b'"') && u != u16::from(b'\\') {
                o.push(char::from(u as u8));
            } else {
                let _ = write!(o, "\\u{u:04x}");
            }
        }
        o.push('"');
        o
    }
    fn hash(s: &str) -> String {
        format!("{:x}", string_hash_code(s))
    }
    fn read(p: &Path) -> Result<Vec<u8>, String> {
        std::fs::read(p).map_err(|e| format!("{}: {e}", p.display()))
    }
    /// The crafted files of one family sorted by name: (label, bytes).
    fn family(dir: &Path, prefix: &str) -> Result<Vec<(String, Vec<u8>)>, String> {
        let mut names = Vec::new();
        for e in std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))? {
            let n = e
                .map_err(|e| e.to_string())?
                .file_name()
                .to_string_lossy()
                .into_owned();
            if n.starts_with(prefix) && n.ends_with(".bin") {
                names.push(n);
            }
        }
        names.sort();
        names
            .into_iter()
            .map(|n| {
                let label = n[prefix.len()..n.len() - 4].to_string();
                Ok((label, read(&dir.join(&n))?))
            })
            .collect()
    }
    let mut sb = String::new();

    // WGEN
    let wgen = |sb: &mut String, label: &str, data: Option<&[u8]>| {
        let mut p = WorldGenParams::default();
        p.set_seed_string("");
        p.min_x_cell = -250;
        p.min_y_cell = -250;
        p.max_x_cell = 250;
        p.max_y_cell = 250;
        let (r, _err) = p.load_file(data);
        let _ = writeln!(
            sb,
            "WGEN {label} {} seed={} {} cells={},{},{},{}",
            match r {
                LoadResult::NotPresent => "NOT_PRESENT",
                LoadResult::Done => "DONE",
            },
            esc(&p.seed_string),
            p.get_seed(),
            p.min_x_cell,
            p.min_y_cell,
            p.max_x_cell,
            p.max_y_cell
        );
    };
    wgen(&mut sb, "missing", None);
    wgen(
        &mut sb,
        "real",
        Some(&read(&save.join("map_worldgen.bin"))?),
    );
    let files = family(crafted, "wgen_")?;
    for (label, data) in &files {
        wgen(&mut sb, label, Some(data));
    }
    let craft = &files
        .iter()
        .find(|(l, _)| l == "craft")
        .ok_or("no wgen_craft.bin")?
        .1;
    for k in 0..=craft.len() {
        wgen(&mut sb, &format!("cut{k}"), Some(&craft[..k]));
    }

    // MSYM
    fn msym_text(label: &str, s: &WorldMapSymbols) -> String {
        let mut o = format!("MSYM {label} n={}", s.symbols.len());
        for sy in &s.symbols {
            let (name, base) = match sy {
                WorldMapSymbol::Text { base, .. } => ("WorldMapTextSymbol", base),
                WorldMapSymbol::Texture { base, .. } => ("WorldMapTextureSymbol", base),
            };
            let _ = write!(o, "\t{name}");
            for v in [
                base.x,
                base.y,
                base.anchor_x,
                base.anchor_y,
                base.scale,
                base.rotation,
                base.r,
                base.g,
                base.b,
                base.a,
            ] {
                let _ = write!(o, ",{}", save_dump::fh(v));
            }
            let flag = |v: bool| if v { '1' } else { '0' };
            let _ = write!(
                o,
                ",{}{}{},{},{}",
                flag(base.collide),
                flag(base.match_perspective),
                flag(base.apply_zoom),
                save_dump::fh(base.min_zoom.unwrap_or(0.0)),
                save_dump::fh(base.max_zoom.unwrap_or(24.0))
            );
            match sy {
                WorldMapSymbol::Text {
                    text,
                    translated,
                    layer_id,
                    ..
                } => {
                    let _ = write!(
                        o,
                        ",{},{},{}",
                        esc(text),
                        flag(*translated),
                        esc(layer_id.as_deref().unwrap_or("text-note"))
                    );
                }
                WorldMapSymbol::Texture { symbol_id, .. } => {
                    let _ = write!(o, ",{}", esc(symbol_id));
                }
            }
        }
        o
    }
    let msym = |sb: &mut String, label: &str, data: Option<&[u8]>, summary: bool| {
        let mut s = WorldMapSymbols::default();
        let _ = s.load_world_map(data);
        let text = msym_text(label, &s);
        if summary {
            let _ = writeln!(
                sb,
                "MSYMCUT {label} n={} {}",
                s.symbols.len(),
                hash(&format!("{text}\n"))
            );
        } else {
            let _ = writeln!(sb, "{text}");
        }
    };
    msym(&mut sb, "missing", None, false);
    msym(
        &mut sb,
        "real",
        Some(&read(&save.join("map_symbols.bin"))?),
        false,
    );
    let files = family(crafted, "msym_")?;
    for (label, data) in &files {
        msym(&mut sb, label, Some(data), false);
    }
    let craft = &files
        .iter()
        .find(|(l, _)| l == "craft")
        .ok_or("no msym_craft.bin")?
        .1;
    for k in 0..=craft.len() {
        msym(&mut sb, &format!("cut{k}"), Some(&craft[..k]), true);
    }

    // STAT
    let stat = |sb: &mut String, label: &str, data: Option<&[u8]>, summary: bool| {
        let mut sm = StatisticsManager::default();
        let _ = sm.load(data);
        let mut parts = String::new();
        let sorted = sm.sorted();
        for st in &sorted {
            let _ = write!(
                parts,
                "\t{},{},{},{},{}",
                esc(&st.name),
                esc(&st.name),
                st.statistic_type.name(),
                st.statistic_category.name(),
                save_dump::fh(st.value)
            );
        }
        let mut ach: Vec<_> = sm.achievements.iter().collect();
        ach.sort_by(|a, b| string_compare_to(a.name, b.name));
        let flags: String = ach
            .iter()
            .map(|a| if a.unlocked { '1' } else { '0' })
            .collect();
        if summary {
            let _ = writeln!(
                sb,
                "STATCUT {label} n={} ach={flags} {}",
                sorted.len(),
                hash(&parts)
            );
        } else {
            let _ = writeln!(sb, "STAT {label} n={} ach={flags}{parts}", sorted.len());
        }
    };
    stat(&mut sb, "missing", None, false);
    stat(
        &mut sb,
        "real",
        Some(&read(&save.join("statistics.bin"))?),
        false,
    );
    let files = family(crafted, "stat_")?;
    for (label, data) in &files {
        stat(&mut sb, label, Some(data), false);
    }
    let craft = &files
        .iter()
        .find(|(l, _)| l == "craft")
        .ok_or("no stat_craft.bin")?
        .1;
    for k in 0..=craft.len() {
        stat(&mut sb, &format!("cut{k}"), Some(&craft[..k]), true);
    }

    // META
    fn meta_text(h: &Helicopter, alarms: &[Alarm]) -> String {
        let mut o = format!(
            "active={} state={} x={} y={} alarms={}",
            u8::from(h.active),
            h.state.map_or("null", |s| s.name()),
            save_dump::fh(h.x),
            save_dump::fh(h.y),
            alarms.len()
        );
        for a in alarms {
            let _ = write!(o, " {},{},{}", a.x, a.y, save_dump::fh(a.end_game_time));
        }
        o
    }
    let meta = |sb: &mut String, label: &str, data: Option<&[u8]>, summary: bool| {
        let mut h = Helicopter::default();
        let mut alarms = Vec::new();
        let _ = meta_tracker::load(data, &mut h, &mut alarms);
        let text = meta_text(&h, &alarms);
        if summary {
            let _ = writeln!(sb, "METACUT {label} {}", hash(&text));
        } else {
            let _ = writeln!(sb, "META {label} {text}");
        }
    };
    meta(&mut sb, "missing", None, false);
    let real_meta = read(&save.join("metadata.bin"))?;
    meta(&mut sb, "real", Some(&real_meta), false);
    {
        // MetaTracker.load into the server's own helicopter: the dedicated server's ambient manager reads nothing
        let mut h = Helicopter::default();
        let mut ignored = Vec::new();
        let _ = meta_tracker::load(Some(&real_meta), &mut h, &mut ignored);
        let t = meta_text(&h, &[]);
        let _ = writeln!(
            sb,
            "METATRACKER real {}",
            t.rsplit_once(" alarms=").map_or(t.as_str(), |(a, _)| a)
        );
    }
    let files = family(crafted, "meta_")?;
    for (label, data) in &files {
        meta(&mut sb, label, Some(data), false);
    }
    let craft = &files
        .iter()
        .find(|(l, _)| l == "craft")
        .ok_or("no meta_craft.bin")?
        .1;
    for k in 0..=craft.len() {
        meta(&mut sb, &format!("cut{k}"), Some(&craft[..k]), true);
    }
    Ok(sb)
}

/// The visited / known map units (`map_visited.bin`) of the ORIGINAL `WorldMapVisited`, driven by the commands of
/// tests/visited_cases.txt (pairs with VisitedProbe.java): the real file, the setters and clearers, `save` and
/// reading back, the current format cut short or too long, and the legacy row format.
pub fn visited(root: &Path, save: &Path, cases: &Path) -> Result<String, String> {
    use zomboid_engine::zombie::world_map::world_map_visited::{GridBounds, WorldMapVisited};
    let media = root.join("media");
    let mut g = IsoMetaGrid::default();
    let dirs = IsoMetaGrid::lot_directories(&media, "Muldraugh, KY");
    g.create_step1(&media, &dirs, (-250, -250, 250, 250))?;
    let grid = GridBounds {
        min_x: g.min_x,
        min_y: g.min_y,
        max_x: g.max_x,
        max_y: g.max_y,
    };
    /// `Arrays.hashCode(byte[])`, as `Integer.toHexString`.
    fn arrays_hash(b: &[u8]) -> String {
        let mut h: i32 = 1;
        for &x in b {
            h = h.wrapping_mul(31).wrapping_add(i32::from(x as i8));
        }
        format!("{h:x}")
    }
    fn unhex(s: &str) -> Result<Vec<u8>, String> {
        (0..s.len() / 2)
            .map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).map_err(|e| e.to_string()))
            .collect()
    }
    fn file_text(f: &Option<Vec<u8>>) -> String {
        f.as_ref().map_or("none".to_string(), |b| {
            format!("{},{}", b.len(), arrays_hash(b))
        })
    }
    fn dump(sb: &mut String, n: usize, v: &WorldMapVisited) {
        let mut c = [0u64; 4];
        for &x in &v.visited {
            for k in 0..4 {
                c[(i32::from(x as i8) >> (2 * k) & 3) as usize] += 1;
            }
        }
        let _ = writeln!(
            sb,
            "DUMP {n} len={} hash={} units={},{},{},{} bounds={},{}",
            v.visited.len(),
            arrays_hash(&v.visited),
            c[0],
            c[1],
            c[2],
            c[3],
            v.min_x,
            v.min_y
        );
    }
    let text = std::fs::read_to_string(cases).map_err(|e| format!("{}: {e}", cases.display()))?;
    let mut sb = String::new();
    let mut file: Option<Vec<u8>> = None;
    let mut map_all_known = false;
    let mut inst: Option<WorldMapVisited> = None;
    let mut n = 0usize;
    for line in text.lines() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        n += 1;
        let t: Vec<&str> = line.split(' ').collect();
        let int = |i: usize| -> Result<i32, String> {
            t.get(i)
                .ok_or_else(|| format!("{line}: missing argument"))?
                .parse::<i32>()
                .map_err(|e| format!("{line}: {e}"))
        };
        match t[0] {
            "mapallknown" => map_all_known = t[1] == "1",
            "open" | "openhex" | "openbody" => {
                match t[0] {
                    "open" => match t[1] {
                        "real" => {
                            let p = save.join("map_visited.bin");
                            file = Some(
                                std::fs::read(&p).map_err(|e| format!("{}: {e}", p.display()))?,
                            );
                        }
                        "none" => file = None,
                        _ => {}
                    },
                    "openhex" => file = Some(unhex(t[1])?),
                    _ => {
                        let probe = WorldMapVisited::new(grid);
                        let len = i64::from(probe.visited_length()) + i64::from(int(1)?);
                        let pat = unhex(t[2])?;
                        let mut b = Vec::new();
                        for v in [249, 2, grid.min_x, grid.min_y, grid.max_x, grid.max_y, 8] {
                            b.extend_from_slice(&i32::to_be_bytes(v));
                        }
                        for i in 0..len.max(0) as usize {
                            b.push(pat[i % pat.len()]);
                        }
                        file = Some(b);
                    }
                }
                // WorldMapVisited.getInstance: load, mark the map known if asked, save (a throw skips the rest)
                let mut v = WorldMapVisited::new(grid);
                if v.load(file.as_deref()).is_ok() {
                    if map_all_known {
                        v.set_known_in_cells(grid.min_x, grid.min_y, grid.max_x, grid.max_y);
                    }
                    file = Some(v.save());
                }
                let _ = writeln!(sb, "OPEN {n} file={}", file_text(&file));
                dump(&mut sb, n, &v);
                inst = Some(v);
            }
            "save" => {
                let v = inst.as_ref().ok_or("save before open")?;
                file = Some(v.save());
                let _ = writeln!(sb, "SAVE {n} file={}", file_text(&file));
            }
            "dump" => dump(&mut sb, n, inst.as_ref().ok_or("dump before open")?),
            cmd => {
                let v = inst.as_mut().ok_or("mutator before open")?;
                let (a, b, c, d) = (int(1)?, int(2)?, int(3)?, int(4)?);
                match cmd {
                    "setVisitedSquares" => v.set_visited_in_squares(a, b, c, d),
                    "setKnownSquares" => v.set_known_in_squares(a, b, c, d),
                    "clearVisitedSquares" => v.clear_visited_in_squares(a, b, c, d),
                    "clearKnownSquares" => v.clear_known_in_squares(a, b, c, d),
                    "setVisitedCells" => v.set_visited_in_cells(a, b, c, d),
                    "setKnownCells" => v.set_known_in_cells(a, b, c, d),
                    "clearVisitedCells" => v.clear_visited_in_cells(a, b, c, d),
                    "clearKnownCells" => v.clear_known_in_cells(a, b, c, d),
                    _ => return Err(format!("unknown command: {line}")),
                }
            }
        }
    }
    Ok(sb)
}

/// `IsoWorld.readWorldVersion` and the `map_ver.bin` read of `IsoWorld.init` on the save's real files and the
/// hand-made pairs of the crafted dir (pairs with MapVerProbe.java).
pub fn mapver(save: &Path, crafted: &Path) -> Result<String, String> {
    use zomboid_engine::zombie::iso::world_version::{read_map_ver, read_world_version};
    /// The probe's `esc`: printable ASCII except quote and backslash stays, any other UTF-16 unit is `\uXXXX`.
    fn esc(s: &str) -> String {
        let mut o = String::from("\"");
        for u in s.encode_utf16() {
            if (0x20..0x7f).contains(&u) && u != u16::from(b'"') && u != u16::from(b'\\') {
                o.push(char::from(u as u8));
            } else {
                let _ = write!(o, "\\u{u:04x}");
            }
        }
        o.push('"');
        o
    }
    fn one(sb: &mut String, label: &str, map_t: Option<&[u8]>, map_ver: Option<&[u8]>) {
        let srv = read_world_version(true, map_t, map_ver);
        let cli = read_world_version(false, map_t, map_ver);
        let _ = writeln!(sb, "MV {label} srv={srv} cli={cli}");
        let r = match map_ver {
            None => "missing".to_string(),
            Some(d) => match read_map_ver(d) {
                Ok((v, m)) => format!("version={v} map={}", esc(&m)),
                Err(e) => format!("exception {}", e.split(':').next().unwrap_or("")),
            },
        };
        let _ = writeln!(sb, "MD {label} {r}");
    }
    let mut sb = String::new();
    one(&mut sb, "none", None, None);
    let t = std::fs::read(save.join("map_t.bin")).ok();
    let v = std::fs::read(save.join("map_ver.bin")).ok();
    one(&mut sb, "real", t.as_deref(), v.as_deref());
    let mut labels: Vec<String> = std::fs::read_dir(crafted)
        .map_err(|e| format!("{}: {e}", crafted.display()))?
        .filter_map(Result::ok)
        .filter_map(|e| {
            let n = e.file_name().to_string_lossy().into_owned();
            n.strip_suffix(".map_t")
                .or_else(|| n.strip_suffix(".map_ver"))
                .map(str::to_string)
        })
        .collect();
    labels.sort();
    labels.dedup();
    for l in labels {
        let t = std::fs::read(crafted.join(format!("{l}.map_t"))).ok();
        let v = std::fs::read(crafted.join(format!("{l}.map_ver"))).ok();
        one(&mut sb, &l, t.as_deref(), v.as_deref());
    }
    Ok(sb)
}

/// The radio on the port, driven by the commands of tests/radio_cases.txt as RadioProbe.java drives the original
/// (see the header of that file for the commands). The Lua handler of `OnLoadRadioScripts` that the original runs
/// (media/lua/server/radio/ISDynamicRadio.lua, which registers the Automated Emergency Broadcast System) is a stand-in
/// here: its state is `dyn_freq` and `dyn_saved`, set by the `dynamic` commands.
pub fn radio(
    root: &Path,
    cases: &Path,
    crafted: &Path,
    real_save: Option<&Path>,
) -> Result<String, String> {
    use zomboid_engine::zombie::core::random::rand_abstract::RandAbstract;
    use zomboid_engine::zombie::core::translator::Translator;
    use zomboid_engine::zombie::radio::channel_category::{ChannelCategory, GameMode};
    use zomboid_engine::zombie::radio::radio_data::RadioData;
    use zomboid_engine::zombie::radio::radio_translation_data::RadioTranslationData;
    use zomboid_engine::zombie::radio::scripting::radio_broad_cast::{BroadcastId, Broadcasts};
    use zomboid_engine::zombie::radio::scripting::radio_channel::RadioChannel;
    use zomboid_engine::zombie::radio::zomboid_radio::{
        weather_interference, InitInputs, ZomboidRadio, LINE_SEPARATOR,
    };

    /// The probe's `esc`: printable ASCII except quote and backslash stays, any other UTF-16 unit is `\uXXXX`.
    fn esc(s: &str) -> String {
        let mut o = String::from("\"");
        for u in s.encode_utf16() {
            if (0x20..0x7f).contains(&u) && u != u16::from(b'"') && u != u16::from(b'\\') {
                o.push(char::from(u as u8));
            } else {
                let _ = write!(o, "\\u{u:04x}");
            }
        }
        o.push('"');
        o
    }
    fn hx(f: f32) -> String {
        format!("{:x}", f.to_bits())
    }
    fn opt(s: Option<&str>) -> String {
        s.unwrap_or("null").to_string()
    }
    fn id_of(b: &Broadcasts, i: Option<BroadcastId>) -> String {
        i.map_or("null".to_string(), |i| b.get(i).id.clone())
    }
    fn line(sb: &mut String, tag: &str, label: &str, rest: &str) {
        let _ = writeln!(sb, "{tag} {label} {rest}");
    }
    fn dump_broadcast(
        sb: &mut String,
        label: &str,
        wh: &str,
        b: &Broadcasts,
        id: BroadcastId,
        depth: i32,
    ) {
        let bc = b.get(id);
        let (donepre, donepost) = bc.segment_flags();
        line(
            sb,
            "BC",
            label,
            &format!(
                "{wh} id={} start={} end={} pre={} post={} lines={} lc={} donepre={donepre} donepost={donepost}",
                bc.id,
                bc.start_stamp,
                bc.end_stamp,
                id_of(b, bc.pre_segment),
                id_of(b, bc.post_segment),
                bc.lines.len(),
                bc.current_line_number()
            ),
        );
        for (j, l) in bc.lines.iter().enumerate() {
            line(
                sb,
                "LN",
                label,
                &format!(
                    "{wh} {j} r={} g={} b={} air={} fx={} text={}",
                    hx(l.r),
                    hx(l.g),
                    hx(l.b),
                    hx(l.air_time()),
                    esc(&l.effects),
                    esc(&l.text)
                ),
            );
        }
        if depth < 2 {
            if let Some(p) = bc.pre_segment {
                dump_broadcast(sb, label, &format!("{wh}/pre"), b, p, depth + 1);
            }
            if let Some(p) = bc.post_segment {
                dump_broadcast(sb, label, &format!("{wh}/post"), b, p, depth + 1);
            }
        }
    }
    fn channel_line(sb: &mut String, label: &str, c: &RadioChannel, b: &Broadcasts) {
        let cur = c.current_script();
        let air = c.airing_broadcast;
        line(
            sb,
            "CH",
            label,
            &format!(
                "{} id={} name={} cat={} tv={} vanilla={} rd={} dynamic={} obf={} synced={} listening={} cur={} loop={} max={} airing={}@{} counter={} lastline={} lastbc={} mult={} minmax={},{}",
                c.frequency,
                c.guid,
                esc(&c.name),
                c.category.name(),
                c.is_tv,
                c.is_vanilla(),
                c.radio_data.as_ref().map_or("null".to_string(), |d| d.is_vanilla.to_string()),
                c.dynamic,
                c.louisville_obfuscate,
                c.is_time_synced,
                c.player_is_listening,
                opt(cur.map(|s| s.name.as_str())),
                c.current_script_loop,
                c.current_script_max_loops,
                id_of(b, air),
                air.map_or(-1, |a| b.get(a).current_line_number()),
                hx(c.air_counter),
                esc(&c.last_aired_line),
                opt(c.last_broadcast_id.as_deref()),
                hx(c.air_counter_multiplier()),
                hx(c.minmod),
                hx(c.maxmod)
            ),
        );
    }
    fn script_line(
        sb: &mut String,
        label: &str,
        c: &RadioChannel,
        s: &zomboid_engine::zombie::radio::scripting::radio_script::RadioScript,
        b: &Broadcasts,
    ) {
        let ex: Vec<String> = s
            .exit_options
            .iter()
            .map(|o| format!("{}:{}:{}", o.scriptname, o.chance, o.start_delay))
            .collect();
        line(
            sb,
            "SC",
            label,
            &format!(
                "{} {} guid={} loops={}-{} startday={} stamp={} internal={} curbc={} aired={} exits=[{}] bcs={}",
                c.frequency,
                s.name,
                s.guid,
                s.loop_min,
                s.loop_max,
                s.start_day,
                s.start_day_stamp,
                s.internal_stamp(),
                id_of(b, s.current_broadcast),
                s.current_has_aired(),
                ex.join(","),
                s.broadcasts.len()
            ),
        );
    }
    fn dump_channel(sb: &mut String, label: &str, c: &RadioChannel, b: &Broadcasts) {
        channel_line(sb, label, c, b);
        let mut names: Vec<&String> = c.scripts.keys().collect();
        names.sort_by(|a, b| java::string_compare_to(a, b));
        for n in names {
            let s = &c.scripts[n];
            script_line(sb, label, c, s, b);
            for (i, &bid) in s.broadcasts.iter().enumerate() {
                dump_broadcast(
                    sb,
                    label,
                    &format!("{} {} {i}", c.frequency, s.name),
                    b,
                    bid,
                    0,
                );
            }
        }
    }
    fn compact_channel(sb: &mut String, label: &str, c: &RadioChannel, b: &Broadcasts) {
        channel_line(sb, label, c, b);
        if let Some(s) = c.current_script() {
            script_line(sb, label, c, s, b);
        }
    }
    fn state(sb: &mut String, label: &str, zr: &ZomboidRadio, rand_id: i32) {
        let m = zr.script_manager.as_ref();
        line(
            sb,
            "ST",
            label,
            &format!(
                "days={} lastHour={} silence={} disable={} louis={} mode={} mgr={} time={} rand={rand_id}",
                zr.days_since_start,
                zr.last_recorded_hour,
                zr.post_radio_silence,
                zr.disable_broadcasting,
                zr.louisville_obfuscation,
                match zr.get_game_mode() {
                    GameMode::SinglePlayer => "SinglePlayer",
                    GameMode::Server => "Server",
                    GameMode::Client => "Client",
                },
                m.is_some(),
                m.map_or(-1, |m| m.current_time_stamp)
            ),
        );
        let kn: Vec<String> = zr
            .names
            .known_frequencies
            .iter()
            .map(i32::to_string)
            .collect();
        line(sb, "KN", label, &format!("[{}]", kn.join(", ")));
        let mut names: Vec<(&i32, &String)> = zr.names.channel_names.iter().collect();
        names.sort();
        let mut s = String::new();
        for (k, v) in names {
            let _ = write!(s, "{k}:{} ", esc(v));
        }
        line(sb, "NM", label, &s);
        let mut cats: Vec<(&String, &std::collections::HashMap<i32, String>)> =
            zr.names.categorized_channels.iter().collect();
        cats.sort_by(|a, b| java::string_compare_to(a.0, b.0));
        for (cat, m) in cats {
            let mut es: Vec<(&i32, &String)> = m.iter().collect();
            es.sort();
            let mut cb = String::new();
            for (k, v) in es {
                let _ = write!(cb, "{k}:{} ", esc(v));
            }
            line(sb, "CT", label, &format!("{cat} {cb}"));
        }
        if let Some(m) = m {
            for (_, c) in &m.channels {
                compact_channel(sb, label, c, &m.broadcasts);
            }
        }
    }
    fn read_save(sb: &mut String, label: &str, content: Option<&str>) {
        let Some(text) = content else {
            line(sb, "SAVE", label, "unchanged");
            return;
        };
        let mut h: i32 = 1;
        for &b in text.as_bytes() {
            h = h.wrapping_mul(31).wrapping_add(i32::from(b as i8));
        }
        line(
            sb,
            "SAVE",
            label,
            &format!("bytes={} hash={:x}", text.len(), h as u32),
        );
        for (n, l) in text.split('\n').enumerate() {
            line(sb, "SL", label, &format!("{n} {}", esc(l)));
        }
    }
    /// `GameTime.getHour()`
    fn hour_of(tod: f32) -> i32 {
        let sec = f64::from(tod * 3600.0f32).floor();
        (sec / 3600.0).floor() as i32
    }
    /// `String.split("\\s+", limit)`: at most `limit` pieces, the last one the rest of the line.
    fn split_n(l: &str, limit: usize) -> Vec<&str> {
        let mut parts = Vec::new();
        let mut rest = l;
        while parts.len() + 1 < limit {
            match rest.find([' ', '\t', '\n', '\u{0B}', '\u{0C}', '\r']) {
                Some(i) => {
                    parts.push(&rest[..i]);
                    rest =
                        rest[i..].trim_start_matches([' ', '\t', '\n', '\u{0B}', '\u{0C}', '\r']);
                }
                None => break,
            }
        }
        parts.push(rest);
        parts
    }
    enum DynFreq {
        Range(i32, i32),
        Fixed(i32),
    }

    let mut sb = String::new();
    let mut tr = Translator::start(root, "EN").map_err(|e| e.to_string())?;
    let mut rand = RandAbstract::new(java::Random::new(0));
    let mut zr = ZomboidRadio::new(GameMode::Server, 0);
    let mut sandbox = 1;
    let mut game_mode = String::from("Sandbox");
    let mut nights = 0;
    let mut time_of_day = 0.0f32;
    let mut wi = 0.0f32;
    let mut multiplier = 1.0f32;
    let mut dyn_freq = DynFreq::Range(88000, 108000);
    let mut dyn_saved: Option<i32> = None;
    let mut radio_save: Option<Vec<u8>> = None;
    // the files "saveto name" wrote (the probe keeps them as files next to its scratch folder)
    let mut generated: std::collections::HashMap<String, Vec<u8>> =
        std::collections::HashMap::new();
    let mut xml_channels: Vec<RadioChannel> = Vec::new();
    let mut xml_broadcasts = Broadcasts::default();
    let text = std::fs::read_to_string(cases).map_err(|e| format!("{}: {e}", cases.display()))?;
    let mut step = 0;
    for raw in java::buffered_reader_lines(&text) {
        let l = java::trim(raw);
        if l.is_empty() || l.starts_with('#') {
            continue;
        }
        step += 1;
        let _ = writeln!(sb, "CMD {step} {l}");
        let p: Vec<&str> = l.split_whitespace().collect();
        let rest = l.split_once(' ').map_or("", |(_, r)| r);
        let st = step.to_string();
        match p[0] {
            "seed" => {
                rand = RandAbstract::new(java::Random::new(
                    p[1].parse::<i64>().map_err(|e| e.to_string())?,
                ));
            }
            "sandbox" => {
                sandbox = p[1]
                    .parse()
                    .map_err(|e: std::num::ParseIntError| e.to_string())?
            }
            "gamemode" => game_mode = rest.to_string(),
            "nights" => {
                nights = p[1]
                    .parse()
                    .map_err(|e: std::num::ParseIntError| e.to_string())?
            }
            "hour" => {
                time_of_day = p[1]
                    .parse()
                    .map_err(|e: std::num::ParseFloatError| e.to_string())?
            }
            "weather" => {
                wi = weather_interference(
                    p[1] == "1",
                    java::parse_float(p[2]).ok_or("weather strength")?,
                    p[3] == "1",
                    p[4] == "1",
                    p[5] == "1",
                );
                zr.weather_interference = wi;
                line(&mut sb, "WI", &st, &hx(wi));
            }
            "flags" => {
                zr.disable_broadcasting = p[1] == "1";
                zr.louisville_obfuscation = p[2] == "1";
            }
            "multiplier" => {
                multiplier = java::parse_float(p[1]).ok_or("multiplier")?;
                // GameTime.getMultiplier on a server that is not fast-forwarding, with the other factors at 1
                let mut m = 1.0f32;
                m *= multiplier;
                m *= 1.0;
                m *= 1.0;
                m *= 1.0;
                m *= 1.0;
                multiplier = m * 0.8f32;
                line(&mut sb, "MULT", &st, &hx(multiplier));
            }
            "dynamic" => {
                dyn_freq = DynFreq::Range(88000, 108000);
                dyn_saved = if p[1] == "reset" {
                    None
                } else {
                    Some(
                        p[2].parse()
                            .map_err(|e: std::num::ParseIntError| e.to_string())?,
                    )
                };
            }
            "radiosave" => {
                radio_save = if p[1] == "none" {
                    None
                } else if p[1] == "@real" {
                    let dir = real_save.ok_or("radiosave @real needs the save folder argument")?;
                    Some(
                        std::fs::read(dir.join("radio/data/RADIO_SAVE.txt"))
                            .map_err(|e| format!("@real: {e}"))?,
                    )
                } else if let Some(g) = p[1].strip_prefix('@') {
                    let f = generated.get(g).cloned();
                    if f.is_none() {
                        line(&mut sb, "RS", &st, "no such generated file");
                    }
                    f
                } else {
                    Some(std::fs::read(crafted.join(p[1])).map_err(|e| format!("{}: {e}", p[1]))?)
                };
            }
            "init" => {
                let v: i32 = p[1]
                    .parse()
                    .map_err(|e: std::num::ParseIntError| e.to_string())?;
                let old = std::mem::replace(
                    &mut zr,
                    ZomboidRadio::new(GameMode::Server, hour_of(time_of_day)),
                );
                zr.disable_broadcasting = old.disable_broadcasting;
                zr.louisville_obfuscation = old.louisville_obfuscation;
                zr.weather_interference = wi;
                let inp = InitInputs {
                    saved_world_version: v,
                    time_since_apo: sandbox,
                    nights_survived: nights,
                    hour: hour_of(time_of_day),
                    game_mode: &game_mode,
                    radio_save: radio_save.as_deref(),
                };
                let mut handler =
                    |radio: &mut ZomboidRadio,
                     _new: bool,
                     rand: &mut RandAbstract<java::Random>| {
                        // media/lua/server/radio/ISDynamicRadio.lua, DynamicRadio.OnLoadRadioScripts
                        if let Some(f) = dyn_saved.take() {
                            dyn_freq = DynFreq::Fixed(f);
                        }
                        if let DynFreq::Range(a, b) = dyn_freq {
                            dyn_freq = DynFreq::Fixed(if a == b {
                                a
                            } else {
                                radio.names.get_random_frequency(a, b, rand)
                            });
                        }
                        if let DynFreq::Fixed(f) = dyn_freq {
                            let mut ch = RadioChannel::new_dynamic(
                                "Automated Emergency Broadcast System",
                                f,
                                ChannelCategory::Emergency,
                                "EMRG-711984",
                            );
                            ch.set_air_counter_multiplier(1.0);
                            radio.add_channel(ch, false);
                        }
                    };
                let _ = zr.init(root, &inp, &mut tr, &mut rand, &mut handler);
                line(&mut sb, "INIT", &st, &format!("v={v} rand={}", rand.id()));
            }
            "state" => state(&mut sb, &st, &zr, rand.id()),
            "fulldump" => {
                state(&mut sb, &st, &zr, rand.id());
                if let Some(m) = zr.script_manager.as_ref() {
                    for (_, c) in &m.channels {
                        dump_channel(&mut sb, &st, c, &m.broadcasts);
                    }
                }
            }
            "updatescripts" => {
                let h: i32 = p[1]
                    .parse()
                    .map_err(|e: std::num::ParseIntError| e.to_string())?;
                let m: i32 = p[2]
                    .parse()
                    .map_err(|e: std::num::ParseIntError| e.to_string())?;
                // the probe deletes the file first, so the file is the text that Save wrote, or none
                let c = zr.update_scripts(h, m, "RADIO_SAVE.txt", &mut rand);
                radio_save = c.as_ref().map(|c| c.clone().into_bytes());
                read_save(&mut sb, &st, c.as_deref());
                if let (Some(c), Some(name)) = (c.as_ref(), p.get(3)) {
                    generated.insert((*name).to_string(), c.clone().into_bytes());
                }
                line(&mut sb, "US", &st, &format!("rand={}", rand.id()));
            }
            "listen" => {
                let f: i32 = p[1]
                    .parse()
                    .map_err(|e: std::num::ParseIntError| e.to_string())?;
                zr.player_listens_channel(f, p[2] == "1", p[3] == "1");
            }
            "tick" => {
                let n: i32 = p[1]
                    .parse()
                    .map_err(|e: std::num::ParseIntError| e.to_string())?;
                for i in 0..n {
                    zr.update(multiplier, &mut rand);
                    if let Some(m) = zr.script_manager.as_ref() {
                        for (_, c) in &m.channels {
                            if let Some(a) = c.airing_broadcast {
                                line(
                                    &mut sb,
                                    "TK",
                                    &st,
                                    &format!(
                                        "{i} {} counter={} lc={} last={}",
                                        c.frequency,
                                        hx(c.air_counter),
                                        m.broadcasts.get(a).current_line_number(),
                                        esc(&c.last_aired_line)
                                    ),
                                );
                            }
                        }
                    }
                }
                line(
                    &mut sb,
                    "TKEND",
                    &st,
                    &format!("rand={} silence={}", rand.id(), zr.post_radio_silence),
                );
            }
            "saveto" => {
                let c = zr.save("RADIO_SAVE.txt", LINE_SEPARATOR);
                radio_save = c.as_ref().map(|c| c.clone().into_bytes());
                read_save(&mut sb, &st, c.as_deref());
                if let Some(c) = c {
                    generated.insert(p[1].to_string(), c.into_bytes());
                }
            }
            "randcheck" => {
                let v = rand.next(1000000);
                line(&mut sb, "RC", &st, &format!("{v} rand={}", rand.id()));
            }
            "scramble" => {
                let parts = split_n(l, 5);
                let custom = if p[3] == "-" { None } else { Some(p[3]) };
                let r = zr.scramble_string(
                    parts[4],
                    p[1].parse()
                        .map_err(|e: std::num::ParseIntError| e.to_string())?,
                    p[2] == "1",
                    custom,
                    &mut rand,
                );
                line(
                    &mut sb,
                    "SCR",
                    &st,
                    &format!("{} rand={}", esc(&r), rand.id()),
                );
            }
            "computerize" => {
                let r = ZomboidRadio::computerize(rest, &mut rand);
                line(
                    &mut sb,
                    "CMP",
                    &st,
                    &format!("{} rand={}", esc(&r), rand.id()),
                );
            }
            "bzzt" => {
                let r = ZomboidRadio::get_random_bzzt_fzzt(&mut rand);
                line(&mut sb, "BZ", &st, &format!("{r} rand={}", rand.id()));
            }
            "randfreq" => {
                let r = zr.names.get_random_frequency(
                    p[1].parse()
                        .map_err(|e: std::num::ParseIntError| e.to_string())?,
                    p[2].parse()
                        .map_err(|e: std::num::ParseIntError| e.to_string())?,
                    &mut rand,
                );
                line(&mut sb, "RF", &st, &format!("{r} rand={}", rand.id()));
            }
            "addname" => zr.names.add_channel_name(
                &p[1].replace('_', " "),
                p[2].parse()
                    .map_err(|e: std::num::ParseIntError| e.to_string())?,
                p[3],
                p[4] == "1",
            ),
            "removename" => zr.names.remove_channel_name(
                p[1].parse()
                    .map_err(|e: std::num::ParseIntError| e.to_string())?,
            ),
            "xml" => {
                let name = p[1];
                let path = crafted.join(name);
                let data = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
                xml_channels = Vec::new();
                xml_broadcasts = Broadcasts::default();
                match RadioData::read_file_bytes(&path.to_string_lossy(), &data) {
                    None => line(
                        &mut sb,
                        "XML",
                        name,
                        &format!("read=null rand={}", rand.id()),
                    ),
                    Some(mut rd) => {
                        line(
                            &mut sb,
                            "XML",
                            name,
                            &format!(
                                "read={},{} rand={}",
                                rd.guid.clone().unwrap_or_default(),
                                rd.version,
                                rand.id()
                            ),
                        );
                        let res =
                            match rd.load_radio_scripts(&mut xml_broadcasts, &mut tr, &mut rand) {
                                Ok(b) => b.to_string(),
                                Err(e) => format!("exception {e}"),
                            };
                        line(
                            &mut sb,
                            "XML",
                            name,
                            &format!("load={res} rand={}", rand.id()),
                        );
                        xml_channels = rd.radio_channels.clone();
                        for (i, c) in xml_channels.iter().enumerate() {
                            dump_channel(&mut sb, &format!("{name}#{i}"), c, &xml_broadcasts);
                        }
                    }
                }
            }
            "xtrans" => {
                let (k, v) = rest.split_once('=').ok_or("xtrans needs key=value")?;
                tr.radio_data_mut().insert(k.to_string(), v.to_string());
            }
            "xchan" => {
                let i: usize = p[1]
                    .parse()
                    .map_err(|e: std::num::ParseIntError| e.to_string())?;
                let c = xml_channels.get_mut(i).ok_or("xchan: no such channel")?;
                match p[2] {
                    "time" => c.update_scripts(
                        p[3].parse()
                            .map_err(|e: std::num::ParseIntError| e.to_string())?,
                        p[4].parse()
                            .map_err(|e: std::num::ParseIntError| e.to_string())?,
                        &xml_broadcasts,
                        &mut rand,
                    ),
                    "listen" => c.set_player_is_listening(p[3] == "1", &mut xml_broadcasts),
                    "update" => {
                        let n: i32 = p[3]
                            .parse()
                            .map_err(|e: std::num::ParseIntError| e.to_string())?;
                        for k in 0..n {
                            let t = c.update(
                                multiplier,
                                zr.disable_broadcasting,
                                zr.louisville_obfuscation,
                                &mut xml_broadcasts,
                            );
                            if let Some(mut t) = t {
                                if t.scramble {
                                    let tmp = t.text.clone();
                                    t.text = zr.scramble_string(&tmp, 85, true, None, &mut rand);
                                }
                                zr.send_transmission(&t, &mut rand);
                            }
                            line(
                                &mut sb,
                                "XK",
                                &st,
                                &format!(
                                    "{k} counter={} last={} airing={}",
                                    hx(c.air_counter),
                                    esc(&c.last_aired_line),
                                    id_of(&xml_broadcasts, c.airing_broadcast)
                                ),
                            );
                        }
                    }
                    _ => return Err(format!("unknown xchan command {l}")),
                }
                compact_channel(&mut sb, &format!("x{i}"), c, &xml_broadcasts);
                line(&mut sb, "XR", &st, &format!("rand={}", rand.id()));
            }
            "charmap" => {
                use zomboid_engine::java_chars;
                let mut start: Option<u32> = None;
                for c in 0..=0x10000u32 {
                    let l = c < 0x10000 && java_chars::is_letter(c as u16);
                    if l && start.is_none() {
                        start = Some(c);
                    }
                    if !l {
                        if let Some(s) = start.take() {
                            line(&mut sb, "LT", &st, &format!("{s:04x} {:04x}", c - 1));
                        }
                    }
                }
                for c in 0..0x10000u32 {
                    if let Some(d) = java_chars::digit10(c as u16) {
                        line(&mut sb, "DG", &st, &format!("{c:04x} {d}"));
                    }
                    if let Some(ch) = char::from_u32(c) {
                        let (lo, up) = (
                            java_chars::to_lower_char(ch) as u32,
                            java_chars::to_upper_char(ch) as u32,
                        );
                        if lo != c || up != c {
                            line(&mut sb, "CM", &st, &format!("{c:04x} {lo:04x} {up:04x}"));
                        }
                    }
                }
            }
            "trans" => {
                let name = p[1];
                let path = crafted.join(name);
                let data = std::fs::read(&path).ok();
                match RadioTranslationData::read_file(
                    &path.to_string_lossy(),
                    data.as_deref(),
                    &mut tr,
                ) {
                    None => line(&mut sb, "TR", name, "read=null"),
                    Some(mut d) => {
                        line(
                            &mut sb,
                            "TR",
                            name,
                            &format!(
                                "guid={} language={} enum={} version={} translators=[{}] valid={}",
                                d.guid.as_deref().map_or("null".to_string(), esc),
                                d.language.as_deref().map_or("null".to_string(), esc),
                                opt(d.language_enum.as_deref()),
                                d.version,
                                d.translators.join(", "),
                                d.validate()
                            ),
                        );
                        let ok = d.load_translations(data.as_deref(), &mut tr);
                        line(
                            &mut sb,
                            "TR",
                            name,
                            &format!("load={ok} count={}", d.translation_count()),
                        );
                        let mut tt: Vec<(&String, &String)> = d.translations.iter().collect();
                        tt.sort_by(|a, b| java::string_compare_to(a.0, b.0));
                        for (k, v) in tt {
                            line(&mut sb, "TT", name, &format!("{}={}", esc(k), esc(v)));
                        }
                    }
                }
            }
            _ => return Err(format!("unknown command {l}")),
        }
    }
    Ok(sb)
}

/// `ErosionConfig.readFile` on the save's `erosion.ini` and on every hand-made `*.ini` of the crafted dir, then
/// `writeFile`'s text (pairs with ErosionIniProbe.java).
pub fn erosionini(save: &Path, crafted: &Path) -> Result<String, String> {
    use zomboid_engine::zombie::erosion::erosion_config::ErosionConfig;
    /// The probe's `esc`: printable ASCII except quote and backslash stays, any other UTF-16 unit is `\uXXXX`.
    fn esc(s: &str) -> String {
        let mut o = String::from("\"");
        for u in s.encode_utf16() {
            if (0x20..0x7f).contains(&u) && u != u16::from(b'"') && u != u16::from(b'\\') {
                o.push(char::from(u as u8));
            } else {
                let _ = write!(o, "\\u{u:04x}");
            }
        }
        o.push('"');
        o
    }
    fn one(sb: &mut String, label: &str, text: Option<&[u8]>) {
        let mut cfg = ErosionConfig::default();
        let ret = cfg.read_file(text);
        let _ = writeln!(sb, "ERO {label} ret={ret}");
        let mut fields: Vec<(String, String)> = Vec::new();
        let bits = |f: f32| format!("{:08x}", f.to_bits());
        for (name, a) in [
            ("seedMain", &cfg.seeds.seed_main),
            ("seedMoisture", &cfg.seeds.seed_moisture),
            ("seedMinerals", &cfg.seeds.seed_minerals),
            ("seedKudzu", &cfg.seeds.seed_kudzu),
        ] {
            for (i, v) in a.iter().enumerate() {
                fields.push((format!("seeds.{name}{i}"), v.to_string()));
            }
        }
        let t = &cfg.time;
        for (n, v) in [
            ("tickunit", t.tickunit),
            ("ticks", t.ticks),
            ("eticks", t.eticks),
            ("epoch", t.epoch),
        ] {
            fields.push((format!("time.{n}"), v.to_string()));
        }
        let s = &cfg.season;
        for (n, v) in [
            ("lat", s.lat),
            ("tempMax", s.temp_max),
            ("tempMin", s.temp_min),
            ("tempDiff", s.temp_diff),
            ("seasonLag", s.season_lag),
            ("seedA", s.seed_a),
            ("seedB", s.seed_b),
            ("seedC", s.seed_c),
        ] {
            fields.push((format!("season.{n}"), v.to_string()));
        }
        fields.push(("season.noon".into(), bits(s.noon)));
        for (m, v) in [
            "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
        ]
        .iter()
        .zip(s.months)
        {
            fields.push((format!("season.{m}"), bits(v)));
        }
        let d = &cfg.debug;
        fields.push(("debug.enabled".into(), d.enabled.to_string()));
        fields.push(("debug.startday".into(), d.startday.to_string()));
        fields.push(("debug.startmonth".into(), d.startmonth.to_string()));
        // the probe prints each section's fields sorted by name
        let section = |n: &str| n.split('.').next().unwrap_or("").to_string();
        for sec in ["seeds", "time", "debug", "season"] {
            let mut part: Vec<&(String, String)> =
                fields.iter().filter(|(n, _)| section(n) == sec).collect();
            part.sort_by(|a, b| a.0.cmp(&b.0));
            for (n, v) in part {
                let _ = writeln!(sb, "F {label} {n}={v}");
            }
        }
        for (i, line) in cfg.write_file().split('\n').enumerate() {
            let _ = writeln!(sb, "W {label} {i} {}", esc(line));
        }
    }
    let mut sb = String::new();
    one(&mut sb, "none", None);
    let real = std::fs::read(save.join("erosion.ini")).ok();
    one(&mut sb, "real", real.as_deref());
    let mut files: Vec<_> = std::fs::read_dir(crafted)
        .map_err(|e| format!("{}: {e}", crafted.display()))?
        .filter_map(Result::ok)
        .filter(|e| e.file_name().to_string_lossy().ends_with(".ini"))
        .collect();
    files.sort_by_key(|e| e.file_name());
    for f in files {
        let name = f.file_name().to_string_lossy().replace(".ini", "");
        let data = std::fs::read(f.path()).map_err(|e| e.to_string())?;
        one(&mut sb, &name, Some(&data));
    }
    Ok(sb)
}

/// The saved part of IsoRegions (`isoregiondata/datachunk_*.bin`, `RegionHeader.bin`) loaded by the port's
/// `IsoRegionWorker.loadSaveMap`, for the player's real folder and each scenario folder of the crafted dir (pairs
/// with IsoRegionProbe.java).
pub fn isoregion(save: &Path, crafted: &Path) -> Result<String, String> {
    use zomboid_engine::zombie::iso::areas::isoregion::iso_region_worker::IsoRegionWorker;
    fn scenario(sb: &mut String, label: &str, dir: Option<&Path>) -> Result<(), String> {
        let mut header = None;
        let mut chunk_files: Vec<(String, Vec<u8>)> = Vec::new();
        if let Some(dir) = dir {
            for e in std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))? {
                let e = e.map_err(|e| e.to_string())?;
                let name = e.file_name().to_string_lossy().into_owned();
                let data = std::fs::read(e.path()).map_err(|e| e.to_string())?;
                if name == "RegionHeader.bin" {
                    header = Some(data);
                } else if name.starts_with("datachunk_") && name.ends_with(".bin") {
                    chunk_files.push((name, data));
                }
            }
        }
        // the original lists the directory in the file system's order; every scenario with more than one file
        // loads them all without error, so the order is only visible in the discovery list, which is printed sorted
        chunk_files.sort();
        let files: Vec<Vec<u8>> = chunk_files.into_iter().map(|(_, d)| d).collect();
        let mut w = IsoRegionWorker::new();
        let _ = w.load_save_map(header.as_deref(), &files);
        w.apply_jobs();
        let mut disc = w.discovered_chunks.clone();
        disc.sort_unstable();
        let mut th = w.thread_discovered_chunks.clone();
        th.sort_unstable();
        let list = |v: &[i32]| {
            format!(
                "[{}]",
                v.iter().map(i32::to_string).collect::<Vec<_>>().join(", ")
            )
        };
        let chunks = w.root_buffer.all_chunks_sorted();
        let _ = writeln!(
            sb,
            "ISO {label} chunks={} disc={} thread={}",
            chunks.len(),
            list(&disc),
            list(&th)
        );
        for c in chunks {
            let mut act = String::new();
            for &b in &c.active_z_layers {
                act.push(if b { '1' } else { '0' });
            }
            let mut h: i32 = 0;
            for z in 0..32 {
                for y in 0..8 {
                    for x in 0..8 {
                        let v = w.root_buffer.get_square_flags(
                            c.chunk_x * 8 + x,
                            c.chunk_y * 8 + y,
                            z,
                        )?;
                        h = h.wrapping_mul(31).wrapping_add(i32::from(v));
                    }
                }
            }
            let flags = c.square_flags.as_ref().map_or("null".to_string(), |f| {
                let mut fh: i32 = 1;
                for &b in f {
                    fh = fh.wrapping_mul(31).wrapping_add(i32::from(b as i8));
                }
                format!("{},{fh:x}", f.len())
            });
            let found = w
                .root_buffer
                .get_data_chunk(c.chunk_x, c.chunk_y)
                .is_some_and(|f| std::ptr::eq(f, c));
            let _ = writeln!(
                sb,
                "CH {label} {} {} id={:x} hz={} flags={flags} active={act} sq={h:x} found={}",
                c.chunk_x,
                c.chunk_y,
                c.hash_id,
                c.highest_z,
                u8::from(found)
            );
        }
        Ok(())
    }
    let mut sb = String::new();
    scenario(&mut sb, "none", None)?;
    scenario(&mut sb, "real", Some(&save.join("isoregiondata")))?;
    let mut dirs: Vec<_> = std::fs::read_dir(crafted)
        .map_err(|e| format!("{}: {e}", crafted.display()))?
        .filter_map(Result::ok)
        .filter(|e| e.path().is_dir())
        .collect();
    dirs.sort_by_key(|e| e.file_name());
    for d in dirs {
        scenario(&mut sb, &d.file_name().to_string_lossy(), Some(&d.path()))?;
    }
    Ok(sb)
}

/// zombie.scripting.ScriptParser over crafted cases, pairs with ScriptParserProbe.java: per case the stripComments
/// text, its parseTokens, and the tree parse() builds from the case text.
pub fn scriptparser(cases: &Path) -> Result<String, String> {
    use zomboid_engine::zombie::scripting::script_parser::{self, Block, BlockElement};
    fn esc(s: Option<&str>) -> String {
        let Some(s) = s else {
            return "null".into();
        };
        let mut sb = String::new();
        for u in s.encode_utf16() {
            match u {
                0x5c => sb.push_str("\\\\"),
                0x09 => sb.push_str("\\t"),
                0x0a => sb.push_str("\\n"),
                0x0d => sb.push_str("\\r"),
                0x20..=0x7e => sb.push(char::from(u as u8)),
                _ => {
                    let _ = write!(sb, "\\u{u:04x}");
                }
            }
        }
        sb
    }
    fn tree(sb: &mut String, b: &Block, depth: usize) {
        // the root block's type is the original's null; a child's is never null
        let ty = if depth == 0 {
            None
        } else {
            Some(b.r#type.as_str())
        };
        let _ = writeln!(
            sb,
            "B\t{depth}\t{}\t{}\tvalues\t{}\tchildren\t{}",
            esc(ty),
            esc(b.id.as_deref()),
            b.values.len(),
            b.children.len()
        );
        for e in &b.elements {
            match e {
                BlockElement::Block(i) => tree(sb, &b.children[*i], depth + 1),
                BlockElement::Value(v) => {
                    let _ = writeln!(
                        sb,
                        "V\t{}\t{}\tkey\t{}\tvalue\t{}",
                        depth + 1,
                        esc(Some(&v.string)),
                        esc(Some(v.get_key())),
                        esc(Some(v.get_value()))
                    );
                }
            }
        }
    }
    let text = std::fs::read_to_string(cases).map_err(|e| format!("{}: {e}", cases.display()))?;
    // text.split("(^|\n)@@case\n", -1)
    let text = match text.strip_prefix("@@case\n") {
        Some(rest) => format!("\n@@case\n{rest}"),
        None => text,
    };
    let mut sb = String::new();
    for (n, c) in text.split("\n@@case\n").enumerate().skip(1) {
        let _ = writeln!(sb, "case\t{n}");
        let stripped = script_parser::strip_comments(c);
        let _ = writeln!(sb, "strip\t{}", esc(Some(&stripped)));
        for t in script_parser::parse_tokens(&stripped) {
            let _ = writeln!(sb, "token\t{}", esc(Some(&t)));
        }
        tree(&mut sb, &script_parser::parse(c), 0);
    }
    Ok(sb)
}

/// Every table of a save's WorldDictionary.bin; pairs with WorldDictionaryProbe.java.
pub fn worlddictionary(path: &Path) -> Result<String, String> {
    use zomboid_engine::zombie::world::dictionary_data::{DictionaryData, DictionaryInfo};
    let data = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let d = DictionaryData::load_from_byte_buffer(&data)?;
    let info = |sb: &mut String, tag: &str, i: &DictionaryInfo| {
        let _ = writeln!(
            sb,
            "{tag}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            i.registry_id,
            i.full_type,
            i.module_name,
            i.name,
            i.mod_id,
            i.is_modded,
            i.exists_as_vanilla,
            i.obsolete,
            i.removed,
            i.mod_overrides
                .as_ref()
                .map_or("null".to_string(), |v| v.join(","))
        );
    };
    let mut sb = String::new();
    let _ = writeln!(
        sb,
        "N\t{}\t{}\t{}",
        d.next_info_id, d.next_object_name_id, d.next_sprite_name_id
    );
    for i in d.items.values() {
        info(&mut sb, "I", i);
    }
    let _ = writeln!(sb, "IT\t{}", d.item_types.len());
    for i in d.entities.values() {
        info(&mut sb, "E", i);
    }
    let _ = writeln!(sb, "ET\t{}", d.entity_types.len());
    for (k, v) in &d.object_names {
        let _ = writeln!(sb, "O\t{k}\t{v}");
    }
    let _ = writeln!(sb, "OT\t{}", d.object_ids.len());
    for (k, v) in &d.sprite_names {
        let _ = writeln!(sb, "S\t{k}\t{v}");
    }
    let _ = writeln!(sb, "ST\t{}", d.sprite_ids.len());
    for r in &d.string_registers {
        let _ = writeln!(sb, "R\t{}\t{}", r.name, r.next_id);
        for i in r.by_id.values() {
            let _ = writeln!(sb, "r\t{}\t{}\t{}", i.registry_id, i.string, i.is_loaded);
        }
    }
    for r in &d.script_registers {
        let _ = writeln!(sb, "C\t{}\t{}", r.name, r.next_id);
        for i in r.by_id.values() {
            let _ = writeln!(
                sb,
                "c\t{}\t{}\t{}\t{}",
                i.registry_id, i.version, i.name, i.is_loaded
            );
        }
    }
    Ok(sb)
}

mod save_dump {
    use std::fmt::Write as _;

    use zomboid_engine::se::krka::kahlua::j2se::kahlua_table_impl::{KahluaTable, LuaValue};
    use zomboid_engine::zombie::core::skinnedmodel::visual::human_visual::HumanVisual;
    use zomboid_engine::zombie::inventory::inventory_item::{InventoryItem, ItemKind, ItemType};
    use zomboid_engine::zombie::inventory::item_container::ItemContainer;
    use zomboid_engine::zombie::inventory::types::alarm_clock::AlarmClockSave;
    use zomboid_engine::zombie::inventory::types::clothing::ClothingSave;
    use zomboid_engine::zombie::inventory::types::moveable::MoveableSave;
    use zomboid_engine::zombie::iso::chunk_save::{ClassSave, ObjectSave};
    use zomboid_engine::zombie::iso::iso_chunk::IsoChunk;
    use zomboid_engine::zombie::iso::iso_grid_square::IsoGridSquare;
    use zomboid_engine::zombie::iso::iso_object::{IsoObject, ObjectKind};
    use zomboid_engine::zombie::iso::objects::iso_dead_body::IsoDeadBody;
    use zomboid_engine::zombie::iso::sprite::iso_sprite_manager::{IsoSpriteManager, SpriteRef};
    use zomboid_engine::zombie::world::dictionary_data::DictionaryData;
    use zomboid_engine::zombie::world_map::symbols::world_map_symbols::WorldMapSymbol;

    /// `Integer.toHexString(Float.floatToRawIntBits(f))`
    pub fn fh(f: f32) -> String {
        format!("{:x}", f.to_bits())
    }
    pub fn dh(d: f64) -> String {
        format!("{:x}", d.to_bits())
    }
    fn b(v: bool) -> char {
        if v {
            '1'
        } else {
            '0'
        }
    }
    fn s(v: Option<&str>) -> &str {
        v.unwrap_or("null")
    }
    /// `sp == null ? "-" : String.valueOf(sp.name)`
    fn sn(sm: &IsoSpriteManager, sp: Option<SpriteRef>) -> String {
        sp.map_or("-".to_string(), |r| {
            s(sm.sprite(r).name.as_deref()).to_string()
        })
    }
    fn lua(v: &LuaValue) -> String {
        match v {
            LuaValue::String(x) => format!("s:{x}"),
            LuaValue::Number(x) => format!("n:{}", dh(*x)),
            LuaValue::Boolean(x) => format!("b:{x}"),
            LuaValue::Table(t) => table(t),
        }
    }
    pub fn table(t: &KahluaTable) -> String {
        let mut l: Vec<String> = t
            .entries
            .iter()
            .map(|(k, v)| format!("{}={}", lua(k), lua(v)))
            .collect();
        l.sort();
        format!("{{{}}}", l.join(","))
    }
    /// `hasModData() ? lua(getModData()) : "-"`
    fn mod_data(t: Option<&KahluaTable>) -> String {
        t.filter(|t| !t.entries.is_empty())
            .map_or("-".to_string(), table)
    }
    fn color(c: Option<[i32; 3]>) -> String {
        c.map_or("null".to_string(), |c| {
            c.map(|v| fh(v as f32 / 255.0)).join(",")
        })
    }
    fn bytes(a: Option<&[i8]>) -> String {
        a.map_or("null".to_string(), |a| {
            a.iter()
                .map(|v| v.to_string())
                .collect::<Vec<_>>()
                .join(",")
        })
    }
    fn human_visual(sb: &mut String, v: &HumanVisual) {
        let white = Some([255, 255, 255]);
        let _ = writeln!(
            sb,
            "HV\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            color(v.skin_color.or(white)),
            v.skin_texture,
            s(v.skin_texture_name.as_deref()),
            v.zombie_rot_stage,
            color(v.hair_color),
            color(v.beard_color),
            color(v.natural_hair_color),
            color(v.natural_beard_color),
            s(v.hair_model.as_deref()),
            s(v.beard_model.as_deref()),
            v.body_hair,
            bytes(Some(&v.blood)),
            bytes(Some(&v.dirt)),
            bytes(Some(&v.holes)),
            s(v.non_attached_hair.as_deref())
        );
        for iv in &v.body_visuals {
            let _ = write!(
                sb,
                "IV\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
                iv.full_type,
                iv.alternate_model_name,
                iv.clothing_item_name,
                color(iv.tint),
                iv.base_texture,
                iv.texture_choice,
                fh(iv.hue),
                s(iv.decal.as_deref())
            );
            for a in [
                &iv.blood,
                &iv.dirt,
                &iv.holes,
                &iv.basic_patches,
                &iv.denim_patches,
                &iv.leather_patches,
            ] {
                let _ = write!(sb, "\t{}", bytes(a.as_ref().map(|a| &a[..])));
            }
            sb.push('\n');
        }
    }
    thread_local! {
        /// The byte ranges of the containers and items the dump prints, while `invcorpus` collects them:
        /// (`CONT` or `ITEM`, hex of the bytes the original wrote), first seen first.
        static CORPUS: std::cell::RefCell<Option<Vec<(&'static str, String)>>> =
            const { std::cell::RefCell::new(None) };
    }
    pub fn corpus_begin() {
        CORPUS.with(|c| *c.borrow_mut() = Some(Vec::new()));
    }
    pub fn corpus_take() -> Vec<(&'static str, String)> {
        CORPUS.with(|c| c.borrow_mut().take().unwrap_or_default())
    }
    fn corpus_add(kind: &'static str, raw: &[u8]) {
        if raw.is_empty() {
            return;
        }
        CORPUS.with(|c| {
            if let Some(v) = c.borrow_mut().as_mut() {
                let hex: String = raw.iter().map(|x| format!("{x:02x}")).collect();
                v.push((kind, hex));
            }
        });
    }

    fn container(sb: &mut String, dict: &DictionaryData, c: &ItemContainer) {
        corpus_add("CONT", &c.raw);
        let _ = write!(
            sb,
            "C\t{}\t{}\t{}\t{}",
            c.r#type,
            b(c.explored),
            b(c.has_been_looted),
            c.capacity
        );
        for it in &c.items {
            let _ = write!(sb, "\t{}:{}", it.full_type, it.id);
        }
        sb.push('\n');
        for it in &c.items {
            item(sb, dict, it);
        }
    }

    /// `o == null ? "null" : "s:" + o`
    fn st(v: Option<&str>) -> String {
        v.map_or("null".to_string(), |v| format!("s:{v}"))
    }
    /// java.util.List.toString
    fn list(v: Option<&[String]>) -> String {
        v.map_or("null".to_string(), |v| format!("[{}]", v.join(", ")))
    }

    /// The rest of the V line, the W line, the key and the P lines ChunkSaveProbe.java prints for a loaded vehicle.
    pub fn vehicle(
        sb: &mut String,
        dict: &DictionaryData,
        v: &zomboid_engine::zombie::vehicles::base_vehicle::BaseVehicle,
    ) {
        let m = &v.moving;
        let _ = writeln!(
            sb,
            "\t{},{},{}\t{},{}\t{}\t{}",
            fh(m.x),
            fh(m.y),
            fh(m.z),
            fh(m.offset_x),
            fh(m.offset_y),
            m.dir & 7,
            mod_data(m.table.as_ref())
        );
        let blood: Vec<String> = v
            .blood_intensity
            .iter()
            .map(|(k, b)| format!("{k}={b}"))
            .collect();
        let mut by_id: Vec<String> = v
            .parts
            .parts_by_id
            .iter()
            .map(|(k, i)| format!("{k}={i}"))
            .collect();
        by_id.sort();
        let idx = |i: Option<usize>| i.map_or(-1, |i| i as i64);
        let q = v.saved_rot;
        let _ = writeln!(
            sb,
            "W\t{}\t{},{},{},{}\t{}\t{}\t{},{},{},{}\t{}\t{}\t{}{}{}{}\t{},{}\t{}{}{}{}\t{},{},{},{}\t{}\t{}\t{}\t{}\t{}\t[{}]\t{},{},{},{}\t{}\t{}{}\t{}\t{}\t{},{}\t[{}]",
            fh(v.saved_physics_z),
            fh(q[0]),
            fh(q[1]),
            fh(q[2]),
            fh(q[3]),
            v.script_name,
            v.skin_index,
            v.front_end_durability,
            v.rear_end_durability,
            v.current_front_end_durability,
            v.current_rear_end_durability,
            v.key_id,
            v.key_spawned,
            b(v.headlights_on),
            b(v.created),
            b(v.sound_horn_on),
            b(v.sound_back_move_on),
            v.lightbar_lights_mode,
            v.lightbar_siren_mode,
            b(v.key_is_on_door),
            b(v.hotwired),
            b(v.hotwired_broken),
            b(v.keys_in_ignition),
            fh(v.rust),
            fh(v.color_hue),
            fh(v.color_saturation),
            fh(v.color_value),
            v.mechanical_id,
            b(v.alarmed),
            dh(v.vehicle_alarm.start_time),
            s(v.vehicle_alarm.chosen_sound.as_deref()),
            dh(v.siren_start_time),
            blood.join(", "),
            v.vehicle_towing_id,
            s(v.tow_attachment_self.as_deref()),
            s(v.tow_attachment_other.as_deref()),
            fh(v.row_constraint_z_offset),
            fh(v.regulator_speed),
            b(v.previously_entered),
            b(v.previously_moved),
            b(v.do_damage_overlay),
            v.parts.parts.len(),
            idx(v.parts.engine),
            idx(v.parts.battery),
            by_id.join(", ")
        );
        if let Some(k) = &v.current_key {
            sb.push_str("KEY\n");
            item(sb, dict, k);
        }
        for (i, p) in v.parts.parts.iter().enumerate() {
            let light = p.light.as_ref().map_or("-".to_string(), |l| {
                format!(
                    "{},{},{},{},{},{}",
                    b(l.active),
                    fh(l.offset_x),
                    fh(l.offset_y),
                    fh(l.intensity),
                    fh(l.dist),
                    l.focusing
                )
            });
            let door = p.door.map_or("-".to_string(), |d| {
                format!("{},{},{}", b(d.open), b(d.locked), b(d.lock_broken))
            });
            let window = p.window.map_or("-".to_string(), |w| {
                format!("{},{}", b(w.open), fh(w.open_delta))
            });
            let engine = p.engine.map_or("-".to_string(), |e| {
                format!("{},{},{},{}", b(e.running), e.quality, e.loudness, e.power)
            });
            let device = p.device_data.as_ref().map_or("-".to_string(), |d| {
                format!(
                    "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
                    d.device_name,
                    b(d.two_way),
                    d.transmit_range,
                    d.mic_range,
                    b(d.mic_is_muted),
                    fh(d.base_volume_range),
                    fh(d.device_volume),
                    b(d.is_portable),
                    b(d.is_television),
                    b(d.is_high_tier),
                    b(d.is_turned_on),
                    d.channel,
                    d.min_channel_range,
                    d.max_channel_range,
                    b(d.is_battery_powered),
                    b(d.has_battery),
                    fh(d.power_delta),
                    fh(d.use_delta),
                    d.headphone_type,
                    d.media_index,
                    d.media_type,
                    s(d.media_item.as_deref()),
                    b(d.no_transmit)
                )
            });
            let mut comps: Vec<i16> = p.components.iter().flatten().map(|(id, _)| *id).collect();
            comps.sort();
            let comps: Vec<String> = comps.iter().map(|c| c.to_string()).collect();
            let it = p.item.as_ref().map_or("-".to_string(), |it| {
                format!(
                    "{}:{},{}",
                    it.full_type,
                    it.id,
                    b(it.broken.unwrap_or(false))
                )
            });
            let _ = writeln!(
                sb,
                "P\t{i}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{light}\t{door}\t{window}\t{engine}\t{device}\t[{}]\t{it}",
                p.part_id,
                b(p.created),
                fh(p.last_updated),
                p.condition,
                fh(p.wheel_friction),
                p.mechanic_skill_installer,
                fh(p.suspension_compression),
                fh(p.suspension_damping),
                mod_data(p.mod_data.as_ref()),
                comps.join(", ")
            );
            if let Some(it) = &p.item {
                item(sb, dict, it);
            }
            if let Some(c) = &p.container {
                container(sb, dict, c);
            }
        }
    }

    /// `"s:" + v` / "null", as ChunkSaveProbe.v prints a String.
    fn sv(v: Option<&str>) -> String {
        v.map_or("null".to_string(), |v| format!("s:{v}"))
    }
    fn sorted(mut l: Vec<String>) -> String {
        l.sort();
        format!("[{}]", l.join(", "))
    }

    /// The rest of the PL line and the SD, HV, C/I, SI, CH, XP, BD, BP, TH, TN and PLY lines ChunkSaveProbe.java prints
    /// for a loaded local player.
    pub fn player(
        sb: &mut String,
        dict: &DictionaryData,
        p: &zomboid_engine::zombie::characters::iso_player::IsoPlayer,
        remaining: usize,
    ) {
        let c = &p.character;
        let m = &c.moving;
        let _ = writeln!(
            sb,
            "\t{remaining}\t{},{},{}\t{},{}\t{}\t{}",
            fh(m.x),
            fh(m.y),
            fh(m.z),
            fh(m.offset_x),
            fh(m.offset_y),
            m.dir & 7,
            mod_data(m.table.as_ref())
        );
        if let Some(desc) = &c.descriptor {
            let _ = writeln!(
                sb,
                "SD\t{}\t{}\t{}\t{}\t{}\t[{}]\t{}\t{}\t{}",
                desc.id,
                desc.forename,
                desc.surname,
                desc.torso,
                b(desc.female),
                desc.extra.join(", "),
                desc.voice_prefix,
                fh(desc.voice_pitch),
                desc.voice_type
            );
        }
        human_visual(sb, &c.visual);
        container(sb, dict, &c.inventory);
        let _ = write!(sb, "SI\t{}", c.saved_items.len());
        for (i, it) in c.saved_items.iter().enumerate() {
            if it.is_none() {
                let _ = write!(sb, "\t{i}");
            }
        }
        sb.push('\n');
        let stats: Vec<String> = c.stats.values.iter().map(|v| fh(*v)).collect();
        let hand = |h: Option<usize>| {
            h.filter(|&i| c.saved_items[i].is_some())
                .map_or(-1, |i| i as i64)
        };
        let effects = [
            c.depress_effect,
            c.depress_first_take_time,
            c.beta_effect,
            c.beta_delta,
            c.pain_effect,
            c.pain_delta,
            c.sleeping_tablet_effect,
            c.sleeping_tablet_delta,
        ]
        .map(fh)
        .join(",");
        let books: Vec<String> = c
            .read_books
            .iter()
            .map(|(t, n)| format!("{t}:{n}"))
            .collect();
        let lit: Vec<String> = c
            .read_literature
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect();
        let media: Vec<String> = c.read_print_media.iter().cloned().collect();
        let cheats: Vec<String> = c.cheats.cheats.iter().map(|i| i.to_string()).collect();
        let _ = writeln!(
            sb,
            "CH\t{},{}\t{}\t{},{}\t{effects}\t[{}]\t{}\t[{}]\t{},{},{},{},{},{}\t{}\t{}\t{}\t[{}]",
            b(c.asleep),
            fh(c.force_wake_up_time),
            stats.join(","),
            hand(c.left_hand),
            hand(c.right_hand),
            books.join(", "),
            fh(c.reduce_infection_power),
            c.known_recipes.join(", "),
            c.last_hour_sleeped,
            fh(c.time_since_last_smoke),
            fh(c.beard_grow_timing),
            fh(c.hair_grow_timing),
            b(c.sneaking),
            b(c.death_drag_down),
            sorted(lit),
            sorted(media),
            c.last_animal_pet,
            cheats.join(", ")
        );
        let traits: Vec<String> = c
            .traits
            .known_traits
            .iter()
            .map(|t| t.to_string())
            .collect();
        let xp_map: Vec<String> =
            c.xp.xp_map
                .iter()
                .map(|(k, v)| format!("{k}={}", fh(*v)))
                .collect();
        let perks: Vec<String> =
            c.xp.perk_list
                .iter()
                .map(|(k, v)| format!("{k}={v}"))
                .collect();
        let mult: Vec<String> =
            c.xp.xp_map_multiplier
                .iter()
                .map(|(k, x)| format!("{k}={},{},{}", fh(x.multiplier), x.min_level, x.max_level))
                .collect();
        let _ = writeln!(
            sb,
            "XP\t[{}]\t{},{},{}\t{}\t[{}]\t{}",
            traits.join(", "),
            fh(c.xp.total_xp),
            c.xp.level,
            c.xp.lastlevel,
            sorted(xp_map),
            perks.join(", "),
            sorted(mult)
        );
        let bd = &c.body_damage;
        let _ = writeln!(
            sb,
            "BD\t{},{},{},{},{},{},{},{},{},{},{},{}",
            fh(bd.catch_a_cold),
            b(bd.has_a_cold),
            fh(bd.cold_strength),
            fh(bd.time_to_sneeze_or_cough),
            b(bd.reduce_fake_infection),
            fh(bd.health_from_food_timer),
            fh(bd.pain_reduction),
            fh(bd.cold_reduction),
            fh(bd.infection_time),
            fh(bd.infection_mortality_duration),
            fh(bd.cold_damage_stage),
            fh(bd.overall_body_health)
        );
        for (i, q) in bd.body_parts.iter().enumerate() {
            let f = [
                fh(q.health),
                b(q.bandaged).to_string(),
                b(q.bitten).to_string(),
                b(q.bleeding).to_string(),
                b(q.is_bleeding_stemmed).to_string(),
                b(q.is_cauterized).to_string(),
                b(q.scratched).to_string(),
                b(q.stitched).to_string(),
                b(q.deep_wounded).to_string(),
                b(q.is_infected).to_string(),
                b(q.is_fake_infected).to_string(),
                fh(q.bandage_life),
                fh(q.scratch_time),
                fh(q.bite_time),
                b(q.alcoholic_bandage).to_string(),
                fh(q.stiffness),
                fh(q.wound_infection_level),
                b(q.infected_wound).to_string(),
                fh(q.bleeding_time),
                fh(q.deep_wound_time),
                b(q.have_glass).to_string(),
                fh(q.stitch_time),
                fh(q.alcohol_level),
                fh(q.additional_pain),
                sv(q.bandage_type.as_deref()),
                b(q.get_bandage_xp).to_string(),
                b(q.get_stitch_xp).to_string(),
                b(q.get_splint_xp).to_string(),
                fh(q.fracture_time),
                b(q.splint).to_string(),
                fh(q.splint_factor),
                b(q.have_bullet).to_string(),
                fh(q.burn_time),
                b(q.need_burn_wash).to_string(),
                fh(q.last_time_burn_wash),
                sv(q.splint_item.as_deref()),
                fh(q.plantain_factor),
                fh(q.comfrey_factor),
                fh(q.garlic_factor),
                fh(q.cut_time),
                b(q.cut).to_string(),
                fh(q.wetness),
            ];
            let _ = writeln!(sb, "BP\t{i}\t{}", f.join(","));
        }
        if let Some(t) = &bd.thermoregulator {
            let _ = writeln!(
                sb,
                "TH\t{},{},{},{},{},{},{},{},{}",
                fh(t.set_point),
                fh(t.metabolic_rate),
                fh(t.metabolic_rate_real),
                fh(t.metabolic_target),
                fh(t.body_heat_delta),
                fh(t.core_heat_delta),
                fh(t.thermal_damage),
                fh(t.damage_counter),
                fh(t.thermal_duration)
            );
            for (i, n) in &t.nodes {
                let _ = writeln!(
                    sb,
                    "TN\t{i}\t{}",
                    [
                        n.celcius,
                        n.skin_celcius,
                        n.heat_delta,
                        n.primary_delta,
                        n.secondary_delta,
                        n.insulation,
                        n.windresist,
                        n.body_wetness,
                        n.clothing_wetness
                    ]
                    .map(fh)
                    .join(",")
                );
            }
        }
        let worn: Vec<String> = p
            .worn_items
            .iter()
            .map(|(l, i)| format!("{l}={i}"))
            .collect();
        let n = &p.nutrition;
        let sv_ = p.saved_vehicle;
        let mech: Vec<String> = p
            .mechanics_item
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect();
        let f = &p.fitness;
        let fit = format!(
            "{}{}{}[{}]{}",
            sorted(
                f.stiffness_inc_map
                    .iter()
                    .map(|(k, v)| format!("{k}={}", fh(*v)))
                    .collect()
            ),
            sorted(
                f.stiffness_timer_map
                    .iter()
                    .map(|(k, v)| format!("{k}={v}"))
                    .collect()
            ),
            sorted(
                f.regularity_map
                    .iter()
                    .map(|(k, v)| format!("{k}={}", fh(*v)))
                    .collect()
            ),
            f.bodypart_to_inc_stiffness.join(", "),
            sorted(
                f.exe_timer
                    .iter()
                    .map(|(k, v)| format!("{k}={v}"))
                    .collect()
            )
        );
        let craft: Vec<String> = p
            .craft_history
            .craft_history
            .iter()
            .map(|(k, e)| format!("{k}=s:{},{}", e.craft_count, dh(e.last_craft_time)))
            .collect();
        let known: Vec<String> = p.known_media_lines.iter().cloned().collect();
        let _ = writeln!(
            sb,
            "PLY\t{},{},{}\t{}\t{},{},{},{},{}\t{},s:{}\t{},{},{}\t{},{},{},{},{},{},{},{}\t{}\t{fit}\t[{}]\t{}\t{}",
            dh(p.hours_survived),
            p.zombie_kills,
            p.survivor_kills,
            sorted(worn),
            fh(n.calories),
            fh(n.proteins),
            fh(n.lipids),
            fh(n.carbohydrates),
            dh(n.weight),
            b(p.all_chat_muted),
            p.tag_prefix,
            fh(p.tag_color[0]),
            fh(p.tag_color[1]),
            fh(p.tag_color[2]),
            sv(Some(&p.display_name)),
            b(p.show_tag),
            b(p.faction_pvp),
            b(p.auto_drink.unwrap_or(true)),
            fh(sv_.map_or(0.0, |v| v.x)),
            fh(sv_.map_or(0.0, |v| v.y)),
            sv_.map_or(-1, |v| i32::from(v.seat)),
            b(sv_.is_some_and(|v| v.running)),
            sorted(mech),
            p.already_read_book.join(", "),
            sorted(known),
            sorted(craft)
        );
    }

    /// The I and J lines ChunkSaveProbe.java prints for an item (and its bag's contents).
    pub fn item(sb: &mut String, dict: &DictionaryData, it: &InventoryItem) {
        corpus_add("ITEM", &it.raw);
        let cls = it.item_type.class_name().unwrap_or("?");
        let tinted = matches!(
            it.item_type,
            ItemType::Clothing | ItemType::AlarmClockClothing | ItemType::Container
        );
        let moveable = matches!(it.item_type, ItemType::Moveable | ItemType::Radio);
        let names: Vec<String> = it
            .extra_items
            .iter()
            .map(|id| {
                dict.items
                    .get(id)
                    .map_or("null".to_string(), |i| i.full_type.clone())
            })
            .collect();
        let extra = format!("[{}]", names.join(", "));
        let _ = write!(
            sb,
            "I\t{}:{}\t{cls}\t{}\t{}/{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{extra}\t{}\t{}\t{}\t{},{}\t{},{},{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{},{},{}\t{}\t{}",
            it.full_type,
            it.id,
            it.uses,
            it.condition,
            it.condition_max,
            b(it.custom_color),
            if tinted { "~".to_string() } else { it.col.map(fh).join(",") },
            fh(it.item_capacity),
            b(it.activated),
            it.have_been_repaired,
            if moveable || it.recorded_media_id.is_some() {
                "~".to_string()
            } else {
                it.name.as_deref().map_or("=".to_string(), |n| format!("s:{n}"))
            },
            it.byte_data.as_ref().map_or("-".to_string(), |d| d.len().to_string()),
            b(it.custom_name),
            if moveable { "~".to_string() } else { it.custom_weight.map_or("-".to_string(), fh) },
            it.key_id,
            it.remote_control_id,
            it.remote_range,
            fh(it.color_red),
            fh(it.color_green),
            fh(it.color_blue),
            st(it.worker.as_deref()),
            fh(it.wet_cooldown),
            b(it.favorite),
            st(it.stash_map.as_deref()),
            b(it.zombie_infected),
            it.current_ammo_count,
            it.attached_slot,
            st(it.attached_slot_type.as_deref()),
            st(it.attached_to_model.as_deref()),
            it.max_capacity,
            it.recorded_media_id.as_deref().map_or(it.recorded_media_index.to_string(), |id| format!(
                "{}={id}",
                it.recorded_media_index
            )),
            fh(it.world_x_rotation),
            fh(it.world_y_rotation),
            fh(it.world_z_rotation),
            fh(it.world_scale),
            b(it.is_initialised)
        );
        let _ = writeln!(
            sb,
            "\t{}\t{}",
            mod_data(it.table.as_ref()),
            it.custom_name_entry.as_ref().map_or("-".to_string(), |n| {
                if moveable {
                    "~".to_string()
                } else {
                    n.as_deref().map_or("=".to_string(), |n| format!("s:{n}"))
                }
            })
        );
        let mut x = String::from("J");
        match &it.kind {
            ItemKind::Plain => {}
            ItemKind::Key(k) => {
                let _ = write!(x, "\t{}", k.number_of_key);
            }
            ItemKind::AlarmClock(a) => alarm(&mut x, a),
            ItemKind::Clothing(c, a) => {
                clothing(&mut x, c);
                if let Some(a) = a {
                    alarm(&mut x, a);
                }
            }
            ItemKind::Container(c) => {
                let _ = write!(x, "\t{}", c.weight_reduction);
            }
            ItemKind::Food(f) => {
                let la = if f.last_aged == -1.0 {
                    "W".to_string()
                } else {
                    fh(f.last_aged)
                };
                let _ = write!(x, "\t{}\t{la}", fh(f.age));
                for v in [
                    f.calories,
                    f.proteins,
                    f.lipids,
                    f.carbohydrates,
                    f.hung_change,
                    f.base_hunger,
                    f.unhappy_change,
                    f.boredom_change,
                    f.thirst_change,
                    f.heat,
                ] {
                    let _ = write!(x, "\t{}", fh(v));
                }
                let _ = write!(
                    x,
                    "\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
                    f.last_cook_minute,
                    fh(f.cooking_time),
                    b(f.cooked),
                    b(f.burnt),
                    b(f.is_cookable),
                    b(f.dangerous_uncooked),
                    f.poison_detection_level,
                    list(f.spices.as_deref()),
                    f.poison_power,
                    st(f.chef.as_deref()),
                    f.off_age,
                    f.off_age_max,
                    fh(f.pain_reduction),
                    f.flu_reduction,
                    f.food_sickness_change,
                    b(f.poison),
                    f.use_for_poison,
                    fh(f.freezing_time),
                    b(f.frozen),
                    fh(f.last_frozen_update),
                    fh(f.rotten_time),
                    fh(f.compost_time),
                    b(f.cooked_in_microwave),
                    fh(f.fatigue_change),
                    fh(f.end_change),
                    f.milk.as_ref().map_or(0, |m| m.0),
                    st(f.milk.as_ref().map(|m| m.1.as_str())),
                    fh(f.stress_change)
                );
            }
            ItemKind::Literature(l) => {
                let pages = l.custom_pages.as_ref().map_or(String::new(), |p| {
                    p.iter()
                        .enumerate()
                        .map(|(i, t)| format!("{}={t}", i + 1))
                        .collect::<Vec<_>>()
                        .join(", ")
                });
                let _ = write!(
                    x,
                    "\t{}\t{}\t{}\t{{{pages}}}\t{}\t{}",
                    l.number_of_pages,
                    l.already_read_pages,
                    b(l.can_be_write),
                    st(l.locked_by.as_deref()),
                    list(l.learned_recipes.as_deref())
                );
            }
            ItemKind::Weapon(w) => {
                let _ = write!(
                    x,
                    "\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t",
                    fh(w.max_range),
                    fh(w.min_range_ranged),
                    w.clip_size,
                    fh(w.min_damage),
                    fh(w.max_damage),
                    w.recoil_delay,
                    w.aiming_time,
                    w.reload_time,
                    w.hit_chance,
                    fh(w.min_angle),
                    w.explosion_timer,
                    fh(w.max_angle),
                    fh(w.blood_level),
                    b(w.is_jammed)
                );
                for p in &w.parts {
                    let _ = write!(x, "{p};");
                }
            }
            ItemKind::Moveable(m) => moveable_fields(&mut x, m),
            ItemKind::Radio(m, r) => {
                moveable_fields(&mut x, m);
                let d = r.device_data.clone().unwrap_or_default();
                let _ = write!(
                    x,
                    "\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
                    d.device_name,
                    b(d.two_way),
                    d.transmit_range,
                    d.mic_range,
                    b(d.mic_is_muted),
                    fh(d.base_volume_range),
                    fh(d.device_volume),
                    b(d.is_portable),
                    b(d.is_television),
                    b(d.is_high_tier),
                    b(d.is_turned_on),
                    d.channel,
                    d.min_channel_range,
                    d.max_channel_range,
                    b(d.is_battery_powered),
                    b(d.has_battery),
                    fh(d.power_delta),
                    fh(d.use_delta),
                    d.headphone_type,
                    d.media_index,
                    d.media_type,
                    s(d.media_item.as_deref()),
                    b(d.no_transmit)
                );
            }
            ItemKind::Map(m) => {
                let _ = write!(x, "\t{}", st(Some(&m.map_id)));
                for sy in &m.symbols.symbols {
                    let (name, base) = match sy {
                        WorldMapSymbol::Text { base, .. } => ("WorldMapTextSymbol", base),
                        WorldMapSymbol::Texture { base, .. } => ("WorldMapTextureSymbol", base),
                    };
                    let _ = write!(x, "\t{name}");
                    for v in [
                        base.x,
                        base.y,
                        base.anchor_x,
                        base.anchor_y,
                        base.scale,
                        base.rotation,
                        base.r,
                        base.g,
                        base.b,
                        base.a,
                    ] {
                        let _ = write!(x, ",{}", fh(v));
                    }
                    let _ = write!(
                        x,
                        ",{}{}{},{},{}",
                        b(base.collide),
                        b(base.match_perspective),
                        b(base.apply_zoom),
                        fh(base.min_zoom.unwrap_or(0.0)),
                        fh(base.max_zoom.unwrap_or(24.0))
                    );
                    match sy {
                        WorldMapSymbol::Text {
                            text,
                            translated,
                            layer_id,
                            ..
                        } => {
                            let _ = write!(
                                x,
                                ",s:{text},{},{}",
                                b(*translated),
                                layer_id.as_deref().unwrap_or("text-note")
                            );
                        }
                        WorldMapSymbol::Texture { symbol_id, .. } => {
                            let _ = write!(x, ",s:{symbol_id}");
                        }
                    }
                }
            }
        }
        if x.len() > 1 {
            sb.push_str(&x);
            sb.push('\n');
        }
        if let ItemKind::Container(c) = &it.kind {
            container(sb, dict, &c.container);
        }
    }

    fn alarm(x: &mut String, a: &AlarmClockSave) {
        let _ = write!(
            x,
            "\t{}\t{}\t{}\t{}\t{}",
            a.alarm_hour,
            a.alarm_minutes,
            b(a.alarm_set),
            dh(f64::from(a.ring_since)),
            a.force_dont_ring
        );
    }

    fn clothing(x: &mut String, c: &ClothingSave) {
        let _ = write!(x, "\t{}\t", fh(c.wetness));
        for (k, p) in c.patches.iter().flatten() {
            let _ = write!(
                x,
                "{k}:{}/{}/{}/{}/{}/{};",
                p.tailor_lvl,
                p.fabric_type,
                p.scratch_defense,
                p.bite_defense,
                b(p.has_hole),
                p.condition_gain
            );
        }
    }

    fn moveable_fields(x: &mut String, m: &MoveableSave) {
        let _ = write!(
            x,
            "\t{}\t{}",
            st(Some(&m.world_sprite)),
            b(m.light.is_some())
        );
        if let Some(l) = &m.light {
            let _ = write!(
                x,
                "\t{}\t{}\t{}",
                b(l.use_battery),
                b(l.has_battery),
                st(Some(l.bulb_item.as_deref().unwrap_or("Base.LightBulb")))
            );
            for v in [l.power, l.delta, l.r, l.g, l.b] {
                let _ = write!(x, "\t{}", fh(v));
            }
        }
    }

    fn object(
        sb: &mut String,
        sm: &IsoSpriteManager,
        dict: &DictionaryData,
        sq: &IsoGridSquare,
        index: usize,
        o: &IsoObject,
    ) {
        let default = ObjectSave::default();
        let os = o.save.as_deref().unwrap_or(&default);
        let world = sq
            .save
            .as_ref()
            .is_some_and(|s| s.world_objects.contains(&index));
        let key_id = match &os.class {
            ClassSave::Door { key_id, .. } => *key_id,
            _ => os.key_id,
        };
        let mut comps: Vec<String> = os.components.iter().map(|(id, _)| id.to_string()).collect();
        comps.sort_by_key(|c| c.parse::<i32>().unwrap_or(0));
        let _ = writeln!(
            sb,
            "O\t{index}\t{}\t{}\t{}\t{}\t{}{}\t{}{}{}{}{}\t{}\t{key_id}\t{}\t{}\t{}\t{}\t{}\t[{}]\t{}",
            o.kind.class_name(),
            sn(sm, o.sprite),
            s(os.name.as_deref()),
            s(os.sprite_name.as_deref()),
            b(o.special),
            b(world),
            b(o.outline_on_mouseover),
            b(os.do_not_sync),
            b(os.special_tooltip),
            b(os.uses_external_water_source),
            b(os.moved_thumpable),
            fh(o.render_y_offset),
            os.custom_color.map_or("-".to_string(), |c| c.map(fh).join(",")),
            os.sheet_rope_health.map_or("-".to_string(), fh),
            sn(sm, os.overlay_sprite),
            os.overlay_sprite_color.map_or("-".to_string(), |c| c.map(fh).join(",")),
            s(os.sprite_model_name.as_deref()),
            comps.join(", "),
            mod_data(os.mod_data.as_ref())
        );
        for (spr, a) in o.attached_anim_sprite.iter().flatten().zip(&os.attached) {
            let d = a.data.unwrap_or([0.0, 0.0, 0.0, 1.0, 1.0, 1.0]);
            let _ = writeln!(
                sb,
                "A\t{}\t{}{}{}\t{},{},{}\t{},{},{}\t{}",
                sn(sm, Some(*spr)),
                b(a.flip),
                b(a.copy_target_alpha),
                b(a.multiply_object_alpha),
                fh(d[0]),
                fh(d[1]),
                fh(d[2]),
                fh(d[3]),
                fh(d[4]),
                fh(d[5]),
                fh(a.alpha.unwrap_or(1.0))
            );
        }
        for w in &os.wall_blood_splats {
            let _ = writeln!(sb, "W\t{}", sn(sm, w.sprite));
        }
        for c in &os.containers {
            container(sb, dict, c);
        }
        let mut x = String::from("X");
        match (&o.kind, &os.class) {
            (
                ObjectKind::IsoDoor(d),
                ClassSave::Door {
                    health,
                    max_health,
                    key_id,
                    has_curtain,
                    curtain_open,
                    curtain_inside,
                },
            ) => {
                let _ = write!(
                    x,
                    "\t{}\t{}\t{}\t{health}\t{max_health}\t{}\t{}\t{key_id}\t{}\t{}\t{}\t{}",
                    b(d.open),
                    b(d.locked),
                    b(d.north),
                    sn(sm, d.closed_sprite),
                    sn(sm, d.open_sprite),
                    b(d.locked_by_key),
                    b(*has_curtain),
                    b(*curtain_open),
                    b(*curtain_inside)
                );
            }
            (ObjectKind::IsoWindow(w), ClassSave::Window { health, max_health }) => {
                let _ = write!(
                    x,
                    "\t{}\t{}\t{health}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{max_health}",
                    b(w.open),
                    b(w.north),
                    b(w.locked),
                    b(w.perma_locked),
                    b(w.smashed),
                    b(w.glass_removed),
                    sn(sm, w.open_sprite),
                    sn(sm, w.closed_sprite),
                    sn(sm, w.smashed_sprite),
                    sn(sm, w.glass_removed_sprite)
                );
            }
            (
                ObjectKind::IsoCurtain(c),
                ClassSave::Curtain {
                    health,
                    barricade_strength,
                },
            ) => {
                let _ = write!(
                    x,
                    "\t{}\t{}\t{health}\t{barricade_strength}\t{}\t{}",
                    b(c.open),
                    b(c.north),
                    sn(sm, c.closed_sprite),
                    sn(sm, c.open_sprite)
                );
            }
            (ObjectKind::IsoWindowFrame { north }, _) => {
                let _ = write!(x, "\t{}", b(*north));
            }
            (ObjectKind::IsoTree(t), _) => {
                let _ = write!(x, "\t{}\t{}\t{}", t.size, t.log_yield, t.damage);
            }
            (_, ClassSave::LightSwitch(l)) => {
                let activated = if l.activated_from_room {
                    "?".to_string()
                } else {
                    b(l.activated).to_string()
                };
                let _ = write!(
                    x,
                    "\t{}\t{}\t{activated}\t{}\t{}\t{}\t{}",
                    b(l.light_room),
                    l.room_id,
                    b(l.can_be_modified),
                    b(l.use_battery),
                    b(l.has_battery),
                    s(l.bulb_item.as_deref())
                );
                for f in [l.power, l.delta, l.primary[0], l.primary[1], l.primary[2]] {
                    let _ = write!(x, "\t{}", fh(f));
                }
                let _ = write!(
                    x,
                    "\t{}\t{}\t{}",
                    l.last_minute_stamp,
                    l.bulb_burn_minutes,
                    b(l.street_light)
                );
            }
            (
                ObjectKind::IsoBarbecue {
                    has_propane_tank,
                    fuel_amount_minutes,
                },
                ClassSave::Barbecue {
                    lit,
                    last_update_time,
                    minutes_since_extinguished,
                    normal,
                    no_tank,
                },
            ) => {
                let _ = write!(
                    x,
                    "\t{}\t{fuel_amount_minutes}\t{}\t{}\t{minutes_since_extinguished}\t{}\t{}",
                    b(*has_propane_tank),
                    b(*lit),
                    fh(*last_update_time),
                    sn(sm, *normal),
                    sn(sm, *no_tank)
                );
            }
            (
                _,
                ClassSave::Fireplace {
                    fuel_amount_minutes,
                    lit,
                    last_update_time,
                    minutes_since_extinguished,
                },
            ) => {
                let _ = write!(
                    x,
                    "\t{fuel_amount_minutes}\t{}\t{}\t{minutes_since_extinguished}",
                    b(*lit),
                    fh(*last_update_time)
                );
            }
            (
                _,
                ClassSave::Stove {
                    activated,
                    seconds_timer,
                    max_temperature,
                    first_turn_on,
                    broken,
                },
            ) => {
                let _ = write!(
                    x,
                    "\t{}\t{seconds_timer}\t{}\t{}\t{}",
                    b(*activated),
                    fh(*max_temperature),
                    b(*first_turn_on),
                    b(*broken)
                );
            }
            (
                _,
                ClassSave::Washer {
                    activated,
                    last_update,
                },
            ) => {
                let _ = write!(x, "\t{}\t{}", b(*activated), fh(*last_update));
            }
            (_, ClassSave::Dryer { activated }) => {
                let _ = write!(x, "\t{}", b(*activated));
            }
            (
                _,
                ClassSave::WasherDryer {
                    dryer,
                    washer_activated,
                    washer_last_update,
                    dryer_activated,
                },
            ) => {
                let _ = write!(
                    x,
                    "\t{}\t{}\t{}\t{}",
                    b(*dryer),
                    b(*washer_activated),
                    fh(*washer_last_update),
                    b(*dryer_activated)
                );
            }
            (_, ClassSave::WaveSignal(d)) => {
                let d = d.clone().unwrap_or_default();
                let _ = write!(
                    x,
                    "\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
                    d.device_name,
                    b(d.two_way),
                    d.transmit_range,
                    d.mic_range,
                    b(d.mic_is_muted),
                    fh(d.base_volume_range),
                    fh(d.device_volume),
                    b(d.is_portable),
                    b(d.is_television),
                    b(d.is_high_tier),
                    b(d.is_turned_on),
                    d.channel,
                    d.min_channel_range,
                    d.max_channel_range,
                    b(d.is_battery_powered),
                    b(d.has_battery),
                    fh(d.power_delta),
                    fh(d.use_delta),
                    d.headphone_type,
                    d.media_index,
                    d.media_type,
                    s(d.media_item.as_deref()),
                    b(d.no_transmit)
                );
                let p = d.presets.unwrap_or_default();
                let _ = write!(
                    x,
                    "\t{}",
                    if p.max_presets == 0 && p.presets.is_empty() {
                        10
                    } else {
                        p.max_presets
                    }
                );
                for (name, freq) in &p.presets {
                    let _ = write!(x, "\t{name}:{freq}");
                }
            }
            (_, ClassSave::Mannequin(m)) => {
                let _ = write!(
                    x,
                    "\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
                    m.dir & 7,
                    b(m.init),
                    b(m.female),
                    b(m.zombie),
                    b(m.skeleton),
                    m.mannequin_script_name,
                    m.pose
                );
            }
            (_, ClassSave::WorldInventory(w)) => {
                let (ft, id) = w
                    .item
                    .as_ref()
                    .map_or(("null".to_string(), 0), |i| (i.full_type.clone(), i.id));
                let _ = write!(
                    x,
                    "\t{}\t{}\t{}\t{ft}:{id}\t{}\t{}\t{}",
                    fh(w.xoff),
                    fh(w.yoff),
                    fh(w.zoff),
                    dh(w.drop_time),
                    b(w.ignore_remove_sandbox),
                    b(w.extended_placement)
                );
            }
            _ => {}
        }
        if x.len() > 1 {
            sb.push_str(&x);
            sb.push('\n');
        }
        if let ClassSave::Mannequin(m) = &os.class {
            human_visual(sb, &m.human_visual);
        }
        if let ClassSave::WorldInventory(w) = &os.class {
            if let Some(i) = &w.item {
                item(sb, dict, i);
            }
        }
    }

    fn corpse(sb: &mut String, dict: &DictionaryData, d: &IsoDeadBody) {
        let _ = write!(
            sb,
            "D\t{},{},{}\t{}{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            fh(d.moving.x),
            fh(d.moving.y),
            fh(d.moving.z),
            b(d.female),
            b(d.was_zombie),
            d.object_id.0,
            d.persistent_outfit_id,
            fh(d.death_time),
            fh(d.reanimate_time),
            b(d.fall_on_front),
            b(d.killed_by_fall),
            b(d.was_skeleton),
            fh(d.angle),
            d.zombie_rot_stage_at_death,
            d.animal_rot_stage_at_death,
            s(d.rotten_texture.as_deref()),
            s(d.skel_inv_icon.as_deref())
        );
        let _ = writeln!(
            sb,
            "\t{}\t{}\t{}\t{}",
            b(d.crawling),
            b(d.fake_dead),
            b(d.ragdoll_fall),
            mod_data(d.moving.table.as_ref())
        );
        if let Some(desc) = &d.desc {
            let _ = writeln!(
                sb,
                "SD\t{}\t{}\t{}\t{}\t{}\t[{}]\t{}\t{}\t{}",
                desc.id,
                desc.forename,
                desc.surname,
                desc.torso,
                b(desc.female),
                desc.extra.join(", "),
                desc.voice_prefix,
                fh(desc.voice_pitch),
                desc.voice_type
            );
        }
        human_visual(sb, &d.human_visual);
        if let Some(c) = &d.container {
            container(sb, dict, c);
        }
    }

    /// The text ChunkSaveProbe.java prints for one loaded chunk.
    pub fn chunk(sb: &mut String, sm: &IsoSpriteManager, dict: &DictionaryData, ch: &IsoChunk) {
        let Some(c) = ch.save.as_deref() else { return };
        let _ = write!(
            sb,
            "K\t{}\t{}\t{}\t{}\t{}{}",
            ch.wx,
            ch.wy,
            ch.get_min_level(),
            ch.get_max_level(),
            b(c.blending_done_full),
            b(c.blending_done_partial)
        );
        for f in c.blending_modified {
            sb.push(b(f));
        }
        let _ = write!(
            sb,
            "\t{}\t{}",
            bytes(Some(&c.blending_depth)),
            b(c.attachments_done_full)
        );
        for f in c.attachments_state {
            sb.push(b(f));
        }
        let e = &c.erosion;
        let _ = write!(
            sb,
            "\t{}\t{},{},{},{},{},{}\t{}\t{}",
            c.attachments_partial
                .as_ref()
                .map_or("-".to_string(), |v| v.len().to_string()),
            b(e.init),
            e.e_tick_stamp,
            e.epoch,
            fh(e.moisture),
            fh(e.minerals),
            e.soil,
            c.generators_touching_this_chunk
                .as_ref()
                .map_or("-".to_string(), |v| v.len().to_string()),
            c.loot_respawn_hour
        );
        for (i, r) in c.spawned_rooms.iter().enumerate() {
            let _ = write!(sb, "{}{r}", if i == 0 { '\t' } else { ',' });
        }
        sb.push('\n');
        for s in &c.floor_blood_splats {
            let _ = writeln!(
                sb,
                "B\t{},{},{}\t{}\t{}",
                fh(s.x),
                fh(s.y),
                fh(s.z),
                s.r#type,
                s.index
            );
        }
        for z in ch.get_min_level()..=ch.get_max_level() {
            for i in 0..64 {
                let Some(sq) = ch.get_grid_square(i % 8, i / 8, z) else {
                    continue;
                };
                let default = Default::default();
                let ss = sq.save.as_deref().unwrap_or(&default);
                let es = &ss.erosion;
                let _ = write!(
                    sb,
                    "Q\t{}\t{}\t{}\t{}\t{}{}\t{}{}{}{}\t{}{},{},{},{}",
                    sq.x,
                    sq.y,
                    sq.z,
                    sq.room_id,
                    b(sq.room_id != -1 && sq.room.is_some()),
                    b(sq.roof_hide_building.is_some()),
                    b(ss.overlay_done),
                    b(sq.have_roof),
                    b(ss.burnt_out),
                    b(ss.have_sheet_rope),
                    b(es.init),
                    b(es.do_nothing),
                    es.noise_main_byte,
                    es.soil,
                    es.magic_num_byte
                );
                for r in &es.regions {
                    let _ = write!(
                        sb,
                        ",{}/{}/{}{}{}/{}/{}/{}",
                        r.region_id,
                        r.category_id,
                        b(r.do_nothing),
                        b(r.has_spawned),
                        b(r.disp_bloom),
                        r.stage,
                        r.disp_season,
                        r.game_obj
                    );
                }
                let _ = writeln!(sb, "\t{}", mod_data(ss.mod_data.as_ref()));
                for (oi, o) in sq.objects.iter().enumerate() {
                    object(sb, sm, dict, sq, oi, o);
                }
                for d in &ss.static_moving_objects {
                    corpse(sb, dict, d);
                }
            }
        }
    }
}

/// Every chunk of a save loaded through the port of IsoChunk.LoadFromDiskOrBuffer, in the world the oracle server
/// runs (seeded basements merged into the meta grid); pairs with ChunkSaveProbe.java.
/// The cases of oracle `invbytes`: every container and item of the chunks, vehicles and players of a save, as the bytes the
/// game wrote (`raw` of the loaded structures), once each, with `reset` lines first. `rawcont`/`rawitem` of
/// `tests/original_probe/InventoryProbe.java`.
pub fn invcorpus(root: &Path, save: &Path) -> Result<String, String> {
    save_dump::corpus_begin();
    let r = chunksave(root, save);
    let ranges = save_dump::corpus_take();
    r?;
    let dir: String = save
        .to_string_lossy()
        .bytes()
        .map(|b| format!("{b:02x}"))
        .collect();
    let mut out = format!("reset 1 tod=7.5,nights=1000\nrecordedmedia {dir}\n");
    let mut seen = std::collections::HashSet::new();
    for (n, (kind, hex)) in ranges.into_iter().enumerate() {
        if seen.insert((kind, hex.clone())) {
            let cmd = if kind == "CONT" { "rawcont" } else { "rawitem" };
            let _ = writeln!(out, "{cmd} s{n} {hex}");
        }
    }
    Ok(out)
}

pub fn chunksave(root: &Path, save: &Path) -> Result<String, String> {
    use zomboid_engine::zombie::basements::Basements;
    use zomboid_engine::zombie::inventory::inventory_item_factory::InventoryItemFactory;
    use zomboid_engine::zombie::iso::chunk_save::SaveLoadContext;
    use zomboid_engine::zombie::radio::media::recorded_media::RecordedMedia;
    use zomboid_engine::zombie::scripting::script_manager::ScriptManager;
    use zomboid_engine::zombie::world::dictionary_data::DictionaryData;

    let media = root.join("media");
    let (mut sm, alias, mut defs) = load_tile_definitions(root)?;
    sm.add_sprite("media/ui/missing-tile.png");
    defs.loaded_tile_definitions(&mut sm, &alias, &|_| false)?;
    let mut g = IsoMetaGrid::default();
    let dirs = IsoMetaGrid::lot_directories(&media, "Muldraugh, KY");
    g.create_step1(&media, &dirs, (-250, -250, 250, 250))?;
    for e in g.load_all_cells(&mut |_| 1, &mut |_, _| Vec::new()) {
        eprintln!("{e}");
    }
    let alarm = AlarmSettings {
        last_stand: false,
        power_grid_exists: true,
        alarm: 1,
        alarm_decay: 1,
    };
    g.create_step2(&alarm, &mut |_| 1, &mut |a, _| a);
    let seed = java::string_hash_code("DeadOxideOracle") as i64;
    Basements::before_load_meta_grid(&media, &mut g, &dirs, seed)?;
    let dict_path = save.join("WorldDictionary.bin");
    let dictionary = DictionaryData::load_from_byte_buffer(
        &std::fs::read(&dict_path).map_err(|e| format!("{}: {e}", dict_path.display()))?,
    )?;
    let scripts = ScriptManager::load(root)?;
    let media_path = save.join("recorded_media.bin");
    let recorded_media = RecordedMedia::load(
        &std::fs::read(&media_path).map_err(|e| format!("{}: {e}", media_path.display()))?,
    )?;
    let lua_path = media.join("lua/shared/RecordedMedia/recorded_media.lua");
    let factory = InventoryItemFactory::new(
        &dictionary,
        &scripts,
        recorded_media,
        &std::fs::read_to_string(&lua_path).map_err(|e| format!("{}: {e}", lua_path.display()))?,
    );
    let mut files = std::collections::BTreeMap::new();
    let map = save.join("map");
    for xd in std::fs::read_dir(&map).map_err(|e| format!("{}: {e}", map.display()))? {
        let xd = xd.map_err(|e| e.to_string())?.path();
        let Some(wx) = xd
            .file_name()
            .and_then(|n| n.to_str())
            .and_then(|n| n.parse::<i32>().ok())
        else {
            continue;
        };
        for yf in std::fs::read_dir(&xd).map_err(|e| format!("{}: {e}", xd.display()))? {
            let yf = yf.map_err(|e| e.to_string())?.path();
            let Some(wy) = yf
                .file_name()
                .and_then(|n| n.to_str())
                .and_then(|n| n.strip_suffix(".bin"))
                .and_then(|n| n.parse::<i32>().ok())
            else {
                continue;
            };
            files.insert((wx, wy), yf);
        }
    }
    let mut rand = RandAbstract::new(java::Random::new(0));
    let mut sb = String::new();
    for ((wx, wy), path) in files {
        let data = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let has_lot_header = g.get_header(wx * 8 / 256, wy * 8 / 256).is_some();
        let mut ctx = SaveLoadContext {
            sprites: &mut sm,
            alias: &alias,
            dictionary: &dictionary,
            items: &factory,
            meta_grid: &mut g,
            rand: &mut rand,
            has_lot_header,
        };
        match IsoChunk::load_from_disk_or_buffer(&data, wx, wy, &mut ctx) {
            Ok(ch) => save_dump::chunk(&mut sb, ctx.sprites, &dictionary, &ch),
            Err(e) => {
                let _ = writeln!(sb, "K\t{wx}\t{wy}\tFAIL\t{e}");
            }
        }
    }
    // zombie.vehicles.VehiclesDB2: every chunk that holds vehicles, its rows as SQLStore.loadChunk reads them.
    use zomboid_engine::zombie::vehicles::vehicles_db2::SqlStore;
    let store = SqlStore::open(&save.join("vehicles.db"))?;
    for (wx, wy) in store.chunks()? {
        for vb in store.load_chunk(wx, wy)? {
            let _ = write!(
                sb,
                "V\t{}\t{wx}\t{wy}\t{},{}\t{}\t{}",
                vb.id,
                save_dump::fh(vb.x),
                save_dump::fh(vb.y),
                vb.world_version,
                u8::from(vb.in_meta)
            );
            match vb.load_vehicle(&factory) {
                Ok(v) => save_dump::vehicle(&mut sb, &dictionary, &v),
                Err(e) => {
                    let _ = writeln!(sb, "\tFAIL\t{e}");
                }
            }
        }
    }
    // zombie.savefile.PlayerDB: every local player, as PlayerDB.loadPlayer reads it.
    use zomboid_engine::java_nio::ByteBuffer;
    use zomboid_engine::zombie::characters::body_damage::body_part::BodySandbox;
    use zomboid_engine::zombie::characters::iso_player::{player_db, IsoPlayer};
    let sandbox = zomboid_engine::zombie::sandbox_options::SandboxOptions::new();
    let opt = |n: &str| sandbox.get_int(n).unwrap_or(0);
    let body_sandbox = BodySandbox {
        transmission: opt("ZombieLore.Transmission"),
        mortality: opt("ZombieLore.Mortality"),
        injury_severity: opt("InjurySeverity"),
    };
    for pd in player_db::load_local_players(&save.join("players.db"))? {
        let _ = write!(
            sb,
            "PL\t{}\t{}\t{},{},{}\t{}\t{}",
            pd.sql_id,
            pd.name.as_deref().unwrap_or("null"),
            save_dump::fh(pd.x),
            save_dump::fh(pd.y),
            save_dump::fh(pd.z),
            pd.world_version,
            u8::from(pd.is_dead)
        );
        let mut b = ByteBuffer::wrap(&pd.data);
        match IsoPlayer::load(
            &mut b,
            pd.world_version,
            &factory,
            &dictionary,
            &body_sandbox,
        ) {
            Ok(p) => save_dump::player(&mut sb, &dictionary, &p, b.remaining()),
            Err(e) => {
                let _ = writeln!(sb, "\tFAIL\t{e}");
            }
        }
    }
    Ok(sb)
}

/// java.util.UUID.toString of (most, least) significant bits.
fn uuid_string(id: (i64, i64)) -> String {
    let (m, l) = (id.0 as u64, id.1 as u64);
    format!(
        "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
        m >> 32,
        (m >> 16) & 0xffff,
        m & 0xffff,
        l >> 48,
        l & 0xffff_ffff_ffff
    )
}

/// The save's zones (pairs with MapZoneProbe.java): the meta grid's cells from `IsoMetaGrid.Create`, then
/// `IsoMetaGrid.loadZone` of the save's `map_zone.bin`; every registered zone, the outline of each wide polyline
/// (its points in the native order; only with `outlines`), with `triangles` every zone's `Zone.getTotalArea` and the
/// triangles and triangle areas behind it, the zones of every meta chunk per cell, and the
/// spawned-zombie zones.
pub fn mapzone(
    root: &Path,
    save: &Path,
    outlines: bool,
    triangles: bool,
) -> Result<String, String> {
    use zomboid_engine::java_nio::ByteBuffer;
    let media = root.join("media");
    let mut g = IsoMetaGrid::default();
    let dirs = IsoMetaGrid::lot_directories(&media, "Muldraugh, KY");
    g.create_step1(&media, &dirs, (-250, -250, 250, 250))?;
    for e in g.load_all_cells(&mut |_| 1, &mut |_, _| Vec::new()) {
        eprintln!("{e}");
    }
    let p = save.join("map_zone.bin");
    let data = std::fs::read(&p).map_err(|e| format!("{}: {e}", p.display()))?;
    let mut bb = ByteBuffer::wrap(&data);
    let mut sb = String::new();
    let wv = data
        .get(4..8)
        .map_or(0, |s| i32::from_be_bytes([s[0], s[1], s[2], s[3]]));
    let _ = writeln!(
        sb,
        "MZ {wv} {} {} {} {}",
        g.min_x, g.min_y, g.max_x, g.max_y
    );
    let m = g.load_zone(&mut bb, -1)?;
    for l in &m.log {
        eprintln!("{l}");
    }
    let _ = writeln!(sb, "REST {}", bb.remaining());
    let _ = writeln!(sb, "ZN {}", g.zones.len());
    let null = |s: &Option<String>| s.clone().unwrap_or_else(|| "null".to_string());
    for (i, z) in g.zones.iter().enumerate() {
        let mut h: u64 = 0xcbf29ce484222325;
        for &v in &z.points {
            mix(&mut h, v);
        }
        let _ = writeln!(
            sb,
            "Z {i} {} {} {} {} {} {} {} {} {} {} {h:x} {} {} {} {} {} {}",
            null(&z.type_),
            null(&z.name),
            z.x,
            z.y,
            z.z,
            z.w,
            z.h,
            z.geometry_type.name(),
            z.polyline_width,
            z.points.len(),
            z.hour_last_seen,
            u8::from(z.have_construction),
            z.last_action_timestamp,
            null(&z.original_name),
            z.id.map_or_else(|| "random".to_string(), uuid_string),
            u8::from(z.is_preferred_zone_for_square)
        );
        if let Some(o) = z.polyline_outline_points.as_ref().filter(|_| outlines) {
            let n = o.len() / 2;
            let _ = write!(sb, "O {i} {n}");
            for p in o.chunks_exact(2) {
                let _ = write!(sb, " {},{}", p[0].to_bits() as i32, p[1].to_bits() as i32);
            }
            sb.push('\n');
        }
    }
    if triangles {
        // Zone.clipper, shared by every zone
        let mut clipper = zomboid_engine::zombie::vehicles::clipper::Clipper::new();
        for (i, z) in g.zones.iter_mut().enumerate() {
            let area = match z.get_total_area(&mut clipper) {
                Ok(a) => a,
                Err(e) => {
                    let _ = writeln!(sb, "AX {i} {}", e.0.replace('/', "."));
                    continue;
                }
            };
            let _ = writeln!(sb, "A {i} {}", area.to_bits() as i32);
            let Some(t) = &z.triangles else {
                continue;
            };
            let _ = write!(sb, "T {i} {}", t.len());
            for v in t {
                let _ = write!(sb, " {}", v.to_bits() as i32);
            }
            let areas = z.triangle_areas.as_deref().unwrap_or(&[]);
            let _ = write!(sb, "\nTA {i} {}", areas.len());
            for v in areas {
                let _ = write!(sb, " {}", v.to_bits() as i32);
            }
            sb.push('\n');
        }
    }
    let mut total = 0;
    for x in g.min_x..=g.max_x {
        for y in g.min_y..=g.max_y {
            let Some(cell) = g.get_cell_data(x, y) else {
                continue;
            };
            let mut chunks = 0;
            let mut h: u64 = 0xcbf29ce484222325;
            for i in 0..1024 {
                let Some(c) = cell.chunk(i) else {
                    continue;
                };
                if c.get_zones_size() == 0 {
                    continue;
                }
                chunks += 1;
                mix(&mut h, i as i32);
                mix(&mut h, c.get_zones_size() as i32);
                for zr in c.zones() {
                    mix(&mut h, zr.0 as i32);
                }
                total += c.get_zones_size();
            }
            if chunks > 0 {
                let _ = writeln!(
                    sb,
                    "ZC {x} {y} {} {chunks} {h:x}",
                    u8::from(cell.info_priority.is_some())
                );
            }
        }
    }
    let _ = writeln!(sb, "ZT {total}");
    let _ = writeln!(sb, "SZ {}", m.spawned_zombie_zone.len());
    for (k, ids) in &m.spawned_zombie_zone {
        let ids: Vec<String> = ids
            .iter()
            .map(|id| id.map_or_else(|| "random".to_string(), uuid_string))
            .collect();
        let _ = writeln!(sb, "S {k} [{}]", ids.join(", "));
    }
    Ok(sb)
}

/// `metagrid/metacell_*.bin` through `IsoMetaCell.load`, then `IsoMetaGrid.loadAnimalZones` of the save's
/// `map_animals.bin` into a meta grid built from the lot headers; every animal zone ordered by id (`UUID.compareTo`:
/// signed most, then least significant bits) with its junctions, and the zones of each cell (pairs with
/// AnimalZonesProbe.java).
pub fn animalzones(root: &Path, save: &Path) -> Result<String, String> {
    use zomboid_engine::java_nio::ByteBuffer;
    let media = root.join("media");
    let mut g = IsoMetaGrid::default();
    let dirs = IsoMetaGrid::lot_directories(&media, "Muldraugh, KY");
    g.create_step1(&media, &dirs, (-250, -250, 250, 250))?;
    for e in g.load_all_cells(&mut |_| 1, &mut |_, _| Vec::new()) {
        eprintln!("{e}");
    }
    let mut sb = String::new();
    for l in g.load_metacells(save) {
        eprintln!("{l}");
    }
    let mut names: Vec<String> = std::fs::read_dir(save.join("metagrid"))
        .map(|rd| {
            rd.filter_map(|e| e.ok().and_then(|e| e.file_name().into_string().ok()))
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    for name in names {
        let Some(rest) = name
            .strip_prefix("metacell_")
            .and_then(|r| r.strip_suffix(".bin"))
        else {
            continue;
        };
        let Some((cx, cy)) = rest
            .split_once('_')
            .and_then(|(a, b)| Some((a.parse::<i32>().ok()?, b.parse::<i32>().ok()?)))
        else {
            continue;
        };
        let generated = g
            .get_cell_data(cx, cy)
            .is_some_and(|c| c.animal_zones_generated);
        let _ = writeln!(sb, "MC {cx} {cy} {}", u8::from(generated));
    }
    let p = save.join("map_animals.bin");
    let data = std::fs::read(&p).map_err(|e| format!("{}: {e}", p.display()))?;
    let mut bb = ByteBuffer::wrap(&data);
    let wv = data
        .get(4..8)
        .map_or(0, |s| i32::from_be_bytes([s[0], s[1], s[2], s[3]]));
    let _ = writeln!(sb, "MA {wv}");
    let m = g.load_animal_zones(&mut bb, -1)?;
    for l in &m.log {
        eprintln!("{l}");
    }
    let _ = writeln!(sb, "REST {}", bb.remaining());
    // the handler's zones ordered by id; arena index -> that order
    let mut handler: Vec<((i64, i64), u32)> = g
        .animal_zone_handler
        .iter()
        .map(|(id, r)| (*id, r.0))
        .collect();
    handler.sort();
    let mut index = vec![-1i32; g.animal_zones.len()];
    for (i, (_, r)) in handler.iter().enumerate() {
        index[*r as usize] = i as i32;
    }
    let _ = writeln!(sb, "AZN {}", handler.len());
    let null = |s: &Option<String>| s.clone().unwrap_or_else(|| "null".to_string());
    for (i, (id, r)) in handler.iter().enumerate() {
        let a = &g.animal_zones[*r as usize];
        let z = &a.zone;
        let mut h: u64 = 0xcbf29ce484222325;
        for &v in &z.points {
            mix(&mut h, v);
        }
        let nj = a.junctions.as_ref().map_or(-1, |j| j.len() as i64);
        let _ = writeln!(
            sb,
            "AZ {i} {} {} {} {} {} {} {} {} {} {} {} {h:x} {} {} {} {} {} {nj}",
            uuid_string(*id),
            null(&z.type_),
            null(&z.name),
            z.x,
            z.y,
            z.z,
            z.w,
            z.h,
            z.geometry_type.name(),
            z.polyline_width,
            z.points.len(),
            null(&z.original_name),
            null(&a.action),
            null(&a.animal_type),
            u8::from(a.spawn_animal),
            u8::from(a.spawned_animals)
        );
        for j in a.junctions.iter().flatten() {
            let other = j.zone_other.map_or(-1, |o| index[o.0 as usize]);
            let _ = writeln!(
                sb,
                "AJ {i} {} {other} {} {}",
                j.point_index_self,
                j.point_index_other,
                j.distance_from_start.to_bits() as i32
            );
        }
    }
    for x in g.min_x..=g.max_x {
        for y in g.min_y..=g.max_y {
            let Some(cell) = g.get_cell_data(x, y) else {
                continue;
            };
            if cell.animal_zones.is_empty() {
                continue;
            }
            let mut h: u64 = 0xcbf29ce484222325;
            for r in &cell.animal_zones {
                mix(&mut h, index[r.0 as usize]);
            }
            let _ = writeln!(sb, "AC {x} {y} {} {h:x}", cell.animal_zones.len());
        }
    }
    Ok(sb)
}

/// ClimateProbe.java's scenarios on the ported ClimateManager: the same clock core, season timer and
/// `ClimateManager.update` per tick, printing the same lines (floats as `Float.toString`).
///
/// The inputs the probe reads from the booted original dedicated server and prints in its SETUP line (the sandbox's
/// Rain, climate and fog cycles, day/night cycle, fog quality, the meta grid bounds of the harness's world, the game
/// mode) are fixed here to what that line shows; the diff checks them against the original.
pub fn climate(root: &Path, scenarios: &[String]) -> Result<String, String> {
    use zomboid_engine::java::{double_to_string as d, float_to_string as f};
    use zomboid_engine::zombie::erosion::season::erosion_season::erosion_main_season;
    use zomboid_engine::zombie::game_time::GameTime;
    use zomboid_engine::zombie::iso::weather::climate_color_info::ClimateColorInfo;
    use zomboid_engine::zombie::iso::weather::climate_forecaster::ForecastValue;
    use zomboid_engine::zombie::iso::weather::climate_main_lua::apply_climate_main;
    use zomboid_engine::zombie::iso::weather::climate_manager::{
        AirFront, ClimateEnv, ClimateManager, BOOL_IS_SNOW, COLOR_GLOBAL_LIGHT, COLOR_NEW_FOG,
        FLOAT_MAX,
    };
    use zomboid_engine::zombie::iso::weather::weather_period::WeatherPeriod;

    let lua_path = root.join("media/lua/server/Climate/ClimateMain.lua");
    let lua =
        std::fs::read_to_string(&lua_path).map_err(|e| format!("{}: {e}", lua_path.display()))?;
    let b = |v: bool| u8::from(v);
    let col = |c: &ClimateColorInfo| {
        format!(
            "{} {} {} {} {} {} {} {}",
            f(c.interior[0]),
            f(c.interior[1]),
            f(c.interior[2]),
            f(c.interior[3]),
            f(c.exterior[0]),
            f(c.exterior[1]),
            f(c.exterior[2]),
            f(c.exterior[3])
        )
    };
    let front = |fr: &AirFront| {
        format!(
            "{} {} {} {} {} {} {} {}",
            fr.type_,
            f(fr.strength),
            f(fr.days),
            f(fr.max_noise),
            f(fr.total_noise),
            f(fr.noise_cache[0]),
            f(fr.noise_cache[1]),
            f(fr.front_wind_angle_degrees)
        )
    };
    let fv = |v: &ForecastValue| {
        [
            v.day_min,
            v.day_max,
            v.day_mean,
            v.night_min,
            v.night_max,
            v.night_mean,
            v.total_min,
            v.total_max,
            v.total_mean,
        ]
        .iter()
        .map(|x| f(*x))
        .collect::<Vec<_>>()
        .join(",")
    };
    let stages = |wp: &WeatherPeriod| {
        let l: Vec<String> = wp
            .weather_stages
            .iter()
            .map(|s| s.stage_id.to_string())
            .collect();
        format!("[{}]", l.join(","))
    };
    let forecasts = |sb: &mut String, cm: &ClimateManager| {
        let fc = &cm.climate_forecaster;
        for (i, &id) in fc.forecasts.iter().enumerate() {
            let df = &fc.days[id];
            let wp = &df.weather_period;
            let list: Vec<String> = df.weather_stages.iter().map(|x| x.to_string()).collect();
            let _ = writeln!(
                sb,
                "FC {i} offset={} starts={} start={} end={} snow={} front={} fog={} fogStr={} fogDur={} overlap={} heavy={} storm={} tropical={} blizzard={} dawn={} dusk={} daylight={} stages=[{}]",
                df.index_offset,
                b(df.weather_starts),
                f(df.weather_start_time),
                f(df.weather_end_time),
                b(df.chance_on_snow),
                df.air_front_string,
                b(df.has_fog),
                f(df.fog_strength),
                f(df.fog_duration),
                fc.index_of(df.weather_overlap),
                b(df.has_heavy_rain),
                b(df.has_storm),
                b(df.has_tropical_storm),
                b(df.has_blizzard),
                f(df.dawn),
                f(df.dusk),
                f(df.day_light_hours),
                list.join(", ")
            );
            let _ = writeln!(
                sb,
                " FCA {} wp={} dur={} str={} angle={} {}",
                front(&df.air_front),
                b(wp.is_running),
                d(wp.duration),
                f(wp.get_total_strength()),
                f(wp.get_wind_angle_degrees()),
                stages(wp)
            );
            let _ = writeln!(
                sb,
                " FCV t={} h={} wd={} wp={} c={}",
                fv(&df.temperature),
                fv(&df.humidity),
                fv(&df.wind_direction),
                fv(&df.wind_power),
                fv(&df.cloudiness)
            );
        }
    };

    let mut sb = String::new();
    for spec in scenarios {
        let p: Vec<&str> = spec.split(':').collect();
        if p.len() != 6 && p.len() != 7 {
            return Err(format!(
                "scenario {spec}: seed:year:month:day:hour:days[:options]"
            ));
        }
        let num = |s: &str| s.parse::<i64>().map_err(|e| format!("{s}: {e}"));
        let seed = num(p[0])?;
        let (year, month, day) = (num(p[1])? as i32, num(p[2])? as i32, num(p[3])? as i32);
        let hour: f32 = p[4].parse().map_err(|e| format!("{}: {e}", p[4]))?;
        let days = num(p[5])?;
        let _ = writeln!(sb, "SCENARIO {spec}");

        let mut rand = RandAbstract::new(java::Random::new(seed));
        let mut gt = GameTime {
            year,
            month,
            day,
            time_of_day: hour,
            nights_survived: 0,
            minutes_per_day: 60.0,
            multiplier: 1.0,
            fps_multiplier: 2.0,
            multiplier_bias: 1.0,
            per_object_multiplier: 1.0,
            ..GameTime::default()
        };
        gt.set_minutes_stamp();

        // The optional options field (rain, climate, fog, daynight, mode), as ClimateProbe reads it.
        let mut env = ClimateEnv {
            server: true,
            client: false,
            debug: false,
            rain_modifier: 3,
            climate_cycle: 1,
            fog_cycle: 1,
            day_night_cycle: 1,
            fog_quality: 0,
            weather_shader: false,
            no_players: true,
            disable_lightning_during_storms: false,
        };
        let mut mode = "Multiplayer".to_string();
        if let Some(opts) = p.get(6) {
            for kv in opts.split(',') {
                let (k, v) = kv.split_once('=').ok_or(format!("option {kv}"))?;
                let n = || v.parse::<i32>().map_err(|e| format!("{kv}: {e}"));
                match k {
                    "rain" => env.rain_modifier = n()?,
                    "climate" => env.climate_cycle = n()?,
                    "fog" => env.fog_cycle = n()?,
                    "daynight" => env.day_night_cycle = n()?,
                    "mode" => mode = v.to_string(),
                    _ => return Err(format!("option {kv}")),
                }
            }
        }
        let grid = [-250, 250, -250, 250];

        // ErosionMain's season (temperature option 3 = normal), set to the date as ErosionMain.mainTimer does.
        let mut season = erosion_main_season(3, env.day_night_cycle);
        let mut e_date = [gt.day, gt.month, gt.year];
        season.set_day(e_date[0], e_date[1], e_date[2]);
        let _ = writeln!(
            sb,
            "SETUP server=1 fogQuality={} multiplier={} unmodded={} rain={} climateCycle={} fogCycle={} dayNight={} gameMode={} weatherShader={} offscreen=0 debug={} forceSnow=0 grid={},{},{},{}",
            env.fog_quality,
            f(gt.get_multiplier()),
            f(gt.get_unmodded_multiplier()),
            env.rain_modifier,
            env.climate_cycle,
            env.fog_cycle,
            env.day_night_cycle,
            mode,
            b(env.weather_shader),
            b(env.debug),
            grid[0],
            grid[1],
            grid[2],
            grid[3]
        );
        let _ = writeln!(
            sb,
            "SEASONCFG {} {} {} {} {} {} {} {} {} {}",
            season.get_lat(),
            season.get_temp_max(),
            season.get_temp_min(),
            season.get_temp_diff(),
            season.get_season_lag(),
            f(season.get_high_noon()),
            season.get_seed_a(),
            season.get_seed_b(),
            season.get_seed_c(),
            f(season.get_rain_year_average())
        );

        let mut cm = ClimateManager::new(&mut rand);
        apply_climate_main(&mut cm, &lua)?;
        cm.init(season, grid[0], grid[1], grid[2], grid[3], &mode, &gt, &env);
        let ts = &cm.thunder_storm;
        let _ = writeln!(
            sb,
            "OFFSETS {} {} {} {} map={},{},{},{} rand={}",
            d(cm.simplex_offset_a),
            d(cm.simplex_offset_b),
            d(cm.simplex_offset_c),
            d(cm.simplex_offset_d),
            ts.map_min_x,
            ts.map_max_x,
            ts.map_min_y,
            ts.map_max_y,
            rand.id()
        );
        let _ = writeln!(
            sb,
            "COL night {} | noMoon {} | moon {}",
            col(&cm.col_night),
            col(&cm.col_night_no_moon),
            col(&cm.col_night_moon)
        );
        let _ = writeln!(
            sb,
            "COL fog {} | legacy {} | new {}",
            col(&cm.col_fog),
            col(&cm.col_fog_legacy),
            col(&cm.col_fog_new)
        );
        let _ = writeln!(
            sb,
            "COL storm {} | tropical {}",
            col(&cm.fog_tint_storm),
            col(&cm.fog_tint_tropical)
        );
        let wpm = &cm.weather_period;
        let _ = writeln!(
            sb,
            "COL clouds {} | {} | {} | {} | {} | {}",
            col(&wpm.cloud_color_reddish),
            col(&wpm.cloud_color_greenish),
            col(&wpm.cloud_color_blueish),
            col(&wpm.cloud_color_purplish),
            col(&wpm.cloud_color_tropical),
            col(&wpm.cloud_color_blizzard)
        );
        for seg in 0..3i32 {
            for temp in 0..3usize {
                for s in 0..4usize {
                    let c = cm
                        .get_season_color(seg, temp, s)
                        .ok_or("season colour out of range")?;
                    let _ = writeln!(sb, "SEASONCOL {seg} {temp} {s} {}", col(c));
                }
            }
        }
        forecasts(&mut sb, &cm);

        let mut minutes_mod = ((gt.time_of_day - gt.time_of_day as i32 as f32) * 60.0) as i32 / 10;
        let mut last_start = f64::NAN;
        let mut last_running = false;
        let mut last_event = "0 0 000".to_string();
        let ticks = days * 108_000;
        for t in 1..=ticks {
            if gt.update_clock(&mut minutes_mod) {
                if gt.day != e_date[0] || gt.month != e_date[1] || gt.year != e_date[2] {
                    e_date = [gt.day, gt.month, gt.year];
                    cm.season.set_day(e_date[0], e_date[1], e_date[2]);
                }
                cm.update_every_ten_mins();
            }
            cm.update(&mut gt, &env, &mut rand);
            if !cm.tick_is_climate_tick {
                continue;
            }
            let _ = writeln!(
                sb,
                "T {t} {}-{}-{} {} age={} stamp={} hour={} day={} rand={}",
                gt.year,
                gt.month,
                gt.day,
                f(gt.time_of_day),
                d(gt.get_world_age_hours()),
                gt.minutes_stamp,
                b(cm.tick_is_hour_change),
                b(cm.tick_is_day_change),
                rand.id()
            );
            let mut v = String::from("V");
            let mut i_ = String::from("I");
            for i in 0..FLOAT_MAX {
                let cf = cm.get_climate_float(i);
                v.push(' ');
                v.push_str(&f(cf.final_value));
                i_.push(' ');
                i_.push_str(&f(cf.internal_value));
            }
            let _ = writeln!(sb, "{v}");
            let _ = writeln!(sb, "{i_}");
            let _ = writeln!(
                sb,
                "G {} | {} | {} | snow={}{}",
                col(&cm.get_climate_color(COLOR_GLOBAL_LIGHT).final_value),
                col(&cm.get_climate_color(COLOR_GLOBAL_LIGHT).internal_value),
                col(&cm.get_climate_color(COLOR_NEW_FOG).final_value),
                b(cm.get_climate_bool(BOOL_IS_SNOW).final_value),
                b(cm.get_climate_bool(BOOL_IS_SNOW).internal_value)
            );
            let cs = cm.get_season();
            let _ = writeln!(
                sb,
                "X {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} season={},{} moon={} view={},{} front={}",
                f(cm.air_mass),
                f(cm.air_mass_daily),
                f(cm.air_mass_temperature),
                f(cm.base_temperature),
                f(cm.snow_fall),
                f(cm.snow_strength),
                f(cm.snow_melt_strength),
                f(cm.snow_frac_now),
                b(cm.can_do_winter_sprites),
                b(cm.day_do_fog),
                f(cm.day_fog_strength),
                f(cm.fog_lerp_value),
                f(cm.wind_power),
                f(cm.day_light_lagged),
                f(cm.night_lagged),
                cs.get_season(),
                f(cs.get_season_progression()),
                cm.moon.get_current_moon_phase(),
                f(gt.view_dist_min),
                f(gt.view_dist_max),
                front(&cm.current_front)
            );
            let wp = &cm.weather_period;
            let start = wp.start_time;
            if wp.is_running != last_running || start != last_start {
                last_running = wp.is_running;
                last_start = start;
                let _ = writeln!(
                    sb,
                    "WP start={} running={} dur={} front={} dirMod={} cloud={}",
                    d(start),
                    b(wp.is_running),
                    d(wp.duration),
                    front(&wp.front_cache),
                    f(wp.wind_angle_dir_mod),
                    col(wp.cloud_color())
                );
                for (i, s) in wp.weather_stages.iter().enumerate() {
                    let _ = writeln!(
                        sb,
                        " WS {i} {} {} {} {} {} {} {} {} {} {}",
                        s.stage_id,
                        d(s.stage_start),
                        d(s.stage_end),
                        d(s.stage_duration),
                        f(s.entry_strength),
                        f(s.exit_strength),
                        f(s.target_strength),
                        s.lerp_mid_val.map_or("null", |v| v.name()),
                        s.lerp_end_val.map_or("null", |v| v.name()),
                        f(s.fog_strength)
                    );
                }
            }
            if wp.is_running {
                let cur = wp.current_stage.map(|i| &wp.weather_stages[i]);
                let stage = match cur {
                    None => "null".to_string(),
                    Some(s) => format!(
                        "{} {} {} {} {} {} {}",
                        s.stage_id,
                        f(s.linear_t),
                        f(s.parabolic_t),
                        b(s.is_cycle_first_half),
                        b(s.has_started_cloud),
                        f(s.entry_strength),
                        f(s.exit_strength)
                    ),
                };
                let _ = writeln!(
                    sb,
                    "W {} {} str={} rain={} noise={} total={} stage={} precip={} tinf={} flags={}{}{} cloud={}",
                    wp.weather_stage_index,
                    stage,
                    f(wp.current_strength),
                    f(wp.rain_threshold),
                    d(f64::from(wp.weather_noise)),
                    f(wp.total_progress),
                    f(wp.stage_progress),
                    f(wp.precipitation_final),
                    f(wp.temperature_influence),
                    b(wp.is_thunder_storm),
                    b(wp.is_tropical_storm),
                    b(wp.is_blizzard),
                    col(wp.cloud_color())
                );
            }
            let ts = &cm.thunder_storm;
            for (i, c) in ts.clouds.iter().enumerate() {
                if !c.is_running && c.start_time == 0.0 {
                    continue;
                }
                let st = &c.suspend_timer;
                let _ = writeln!(
                    sb,
                    "TC {i} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} timer={},{},{},{} strike={}",
                    b(c.is_running),
                    c.current_x,
                    c.current_y,
                    c.start_x,
                    c.start_y,
                    c.end_x,
                    c.end_y,
                    d(c.start_time),
                    d(c.end_time),
                    d(c.duration),
                    f(c.strength),
                    f(c.angle),
                    f(c.radius),
                    f(c.event_frequency),
                    f(c.thunder_ratio),
                    f(c.percentage_offset),
                    f(st.elapsed),
                    f(st.duration),
                    b(st.finished),
                    st.ticks,
                    ts.strike_radius
                );
            }
            let ne = &ts.network_thunder_event;
            let te = format!(
                "{} {} {}{}{}",
                ne.event_x,
                ne.event_y,
                b(ne.do_strike),
                b(ne.do_lightning),
                b(ne.do_rumble)
            );
            if te != last_event {
                let _ = writeln!(sb, "TE {te}");
                last_event = te;
            }
            if cm.tick_is_day_change {
                forecasts(&mut sb, &cm);
            }
        }
        let _ = writeln!(sb, "END {spec} rand={}", rand.id());
    }
    Ok(sb)
}

/// The original ErosionMain (with ErosionWorld, the eight categories, ErosionObj*, ErosionIceQueen and ErosionSeason)
/// run over chunks of the real map the way a dedicated server runs it: chunk load, `LoadGridsquare` and `ChunkLoaded`,
/// then simulated days of 144 ten-minute `EveryTenMinutes`; in ErosionProbe.java's format. The probe's draw counter of
/// the global Rand is 0 on the Rust side by construction: the erosion code takes no Rand.
pub fn erosion(root: &Path, scenarios: &[String]) -> Result<String, String> {
    use zomboid_engine::java::float_to_string as f;
    use zomboid_engine::zombie::erosion::erosion_data::{self, Square};
    use zomboid_engine::zombie::erosion::erosion_main::{ErosionMain, ErosionOptions};
    use zomboid_engine::zombie::game_time::GameTime;
    use zomboid_engine::zombie::iso::server_map::ServerMap;

    let mut rand = RandAbstract::new(java::Random::new(12345));
    let alarm = AlarmSettings {
        last_stand: false,
        power_grid_exists: true,
        alarm: 1,
        alarm_decay: 1,
    };
    let mut world = IsoWorld::init(root, "Muldraugh, KY", &|_| false, &alarm, None, &mut rand)?;
    let rands = erosion_data::load_rands(root)?;
    // zombie.erosion.ErosionGlobals.Boot (called by IsoWorld.init): the server's own ErosionMain, made before any
    // probe runs; it leaves the 90 winter sprites the ice queen makes in the sprite manager (the scenarios' ErosionMain
    // then finds them there).
    let _boot = ErosionMain::new(
        ErosionOptions::default(),
        None,
        &GameTime::default(),
        &mut world.sprites,
        &world.alias,
        rands.clone(),
    );
    // the Lua of OnServerStarted runs after the world is booted
    apply_custom_tile_props(root, &mut world.sprites, &world.alias)?;
    let mut sb = String::new();
    // the erosion.ini of the save: what the last ErosionMain.mainTimer wrote
    let mut saved_ini: Option<String> = None;
    for spec in scenarios {
        let p: Vec<&str> = spec.split(':').collect();
        if p.len() < 6 {
            return Err(format!("bad scenario {spec}"));
        }
        let int = |s: &str| s.parse::<i32>().map_err(|e| format!("{spec}: {e}"));
        let rc: Vec<i32> = p[0].split(',').map(int).collect::<Result<_, _>>()?;
        if rc.len() != 4 {
            return Err(format!("bad chunk range in {spec}"));
        }
        let (year, month, day, days) = (int(p[1])?, int(p[2])?, int(p[3])?, int(p[4])?);
        let dumps: Vec<i32> = p[5].split('/').map(int).collect::<Result<_, _>>()?;
        let mut opts = ErosionOptions::default();
        let mut keep = false;
        if p.len() > 6 {
            for kv in p[6].split(',') {
                let (k, v) = kv.split_once('=').unwrap_or((kv, ""));
                match k {
                    "speed" => opts.erosion_speed = int(v)?,
                    "edays" => opts.erosion_days = int(v)?,
                    "apo" => opts.time_since_apo = int(v)?,
                    "temp" => opts.temperature_modifier = int(v)?,
                    "keep" => keep = int(v)? != 0,
                    _ => return Err(format!("bad option {kv}")),
                }
            }
        }
        let _ = writeln!(sb, "SCENARIO {spec}");
        if !keep {
            saved_ini = None;
        }
        let mut gt = GameTime {
            year,
            month,
            day,
            time_of_day: 12.0,
            nights_survived: 0,
            ..GameTime::default()
        };
        let sprites_before = world.sprites.named_map.len();
        let mut em = ErosionMain::new(
            opts,
            saved_ini.as_deref(),
            &gt,
            &mut world.sprites,
            &world.alias,
            rands.clone(),
        );
        let describe = |em: &ErosionMain| {
            format!(
                "ticks={} eTicks={} epoch={} tickUnit={} date={}-{}-{} season={} seasonDay={} seasonDays={} progression={} snow={}",
                em.ticks,
                em.e_ticks,
                em.epoch,
                em.tick_unit,
                em.year,
                em.month,
                em.day,
                em.season.get_season(),
                f(em.season.get_season_day()),
                f(em.season.get_season_days()),
                f(em.season.get_season_progression()),
                u8::from(em.is_snow)
            )
        };
        let mut map = ServerMap::new();
        let _ = writeln!(
            sb,
            "SETUP server=1 speed={} edays={} apo={} temp={} loadedCells={} {} newSprites={}",
            opts.erosion_speed,
            opts.erosion_days,
            opts.time_since_apo,
            opts.temperature_modifier,
            map.loaded_cells.len(),
            describe(&em),
            world.sprites.named_map.len() - sprites_before
        );

        let mut keys = Vec::new();
        {
            let IsoWorld {
                sprites,
                alias,
                tile_definitions,
                meta_grid,
                ..
            } = &mut world;
            let mut ctx = LoadContext {
                sprites,
                alias,
                glass_removed_window_sprite_map: &tile_definitions.glass_removed_window_sprite_map,
                smashed_window_sprite_map: &tile_definitions.smashed_window_sprite_map,
                meta_grid,
                rand: &mut rand,
                locked_houses: 6,
                missing_tiles: Vec::new(),
                lotpack_cache: None,
            };
            for wx in rc[0]..=rc[2] {
                for wy in rc[1]..=rc[3] {
                    *ctx.rand = RandAbstract::new(java::Random::new(12345));
                    let mut ch = IsoChunk::new(wx, wy);
                    cell_loader::load_cell_binary_chunk(&mut ctx, wx, wy, &mut ch, &mut Vec::new());
                    ch.load_in_world_streamer_thread(&*ctx.sprites, &*ctx.meta_grid, ctx.alias);
                    map.add_chunk(ch);
                    keys.push((wx, wy));
                }
            }
        }
        map.set_all_cells_loaded();
        for ch in map.chunks.values_mut() {
            ch.set_loaded();
        }

        let _ = writeln!(sb, "RAND before=0");
        let mut cells: Vec<(i32, i32)> = map.loaded_cells.iter().map(|c| (c.wx, c.wy)).collect();
        cells.sort();
        for (cwx, cwy) in cells {
            for x in 0..8 {
                for y in 0..8 {
                    let key = (cwx * 8 + x, cwy * 8 + y);
                    if !map.chunks.contains_key(&key) {
                        continue;
                    }
                    let mut zz = map.chunks[&key].get_min_level();
                    while zz <= map.chunks[&key].get_max_level() {
                        for sx in 0..8 {
                            for sy in 0..8 {
                                let pos = map.chunks[&key]
                                    .get_grid_square(sx, sy, zz)
                                    .filter(|s| !s.objects.is_empty())
                                    .map(|s| (s.x, s.y, s.z));
                                if let Some(pos) = pos {
                                    em.load_gridsquare(
                                        &gt,
                                        &mut map,
                                        &mut world.sprites,
                                        &world.alias,
                                        pos,
                                    )?;
                                }
                            }
                        }
                        zz += 1;
                    }
                    em.load_chunk(&mut map, key.0, key.1)?;
                }
            }
        }
        let _ = writeln!(sb, "RAND after load=0");

        let mut dumped = 0;
        let do_dump =
            |sb: &mut String, em: &ErosionMain, map: &ServerMap, world: &IsoWorld, d: i32| {
                let _ = writeln!(
                    sb,
                    "DUMP {d} {} newSprites={}",
                    describe(em),
                    world.sprites.named_map.len() - sprites_before
                );
                let mut all: Vec<(i32, i32, i32)> = Vec::new();
                for ((wx, wy), ch) in &map.chunks {
                    let mut bb = Vec::new();
                    ch.erosion.save(&mut bb);
                    let _ = writeln!(
                        sb,
                        "C {wx} {wy} {} moisture={} minerals={}",
                        hex(&bb),
                        f(ch.erosion.moisture),
                        f(ch.erosion.minerals)
                    );
                    for z in ch.get_min_level()..=ch.get_max_level() {
                        for y in 0..8 {
                            for x in 0..8 {
                                if let Some(sq) = ch.get_grid_square(x, y, z) {
                                    if !sq.objects.is_empty() {
                                        all.push((sq.x, sq.y, sq.z));
                                    }
                                }
                            }
                        }
                    }
                }
                all.sort();
                for (x, y, z) in all {
                    let Some(sq) = map.get_grid_square(x, y, z) else {
                        continue;
                    };
                    let default_ed = Square::default();
                    let ed = sq.erosion.as_deref().unwrap_or(&default_ed);
                    let mut bb = Vec::new();
                    ed.save(&mut bb);
                    let flags: Vec<&str> = sq
                        .properties
                        .get_flags_list()
                        .iter()
                        .map(|fl| fl.name())
                        .collect();
                    let _ = write!(
                        sb,
                        "S {x} {y} {z} {} kudzu={:x} {} {} {:x}",
                        hex(&bb),
                        ed.noise_kudzu.to_bits(),
                        u8::from(sq.have_roof),
                        flags.join(","),
                        sq.has_types
                    );
                    for o in &sq.objects {
                        let sprite_name =
                        |r: zomboid_engine::zombie::iso::sprite::iso_sprite_manager::SpriteRef| {
                            world
                                .sprites
                                .sprite(r)
                                .name
                                .clone()
                                .unwrap_or_else(|| "null".to_string())
                        };
                        let _ = write!(
                            sb,
                            " {}:{}:{}",
                            o.kind.class_name(),
                            o.name.as_deref().unwrap_or("null"),
                            o.sprite.map_or_else(|| "null".to_string(), sprite_name)
                        );
                        if let Some(att) = &o.attached_anim_sprite {
                            for a in att {
                                let _ = write!(sb, "+{}", sprite_name(*a));
                            }
                        }
                        if let ObjectKind::IsoTree(t) = &o.kind {
                            let _ = write!(sb, "{{s{}}}", t.size);
                        }
                    }
                    sb.push('\n');
                }
            };
        if dumps.contains(&0) {
            do_dump(&mut sb, &em, &map, &world, 0);
            dumped += 1;
        }
        for d in 1..=days {
            gt.advance_one_day();
            for _ in 0..144 {
                em.main_timer(&gt, &mut map, &mut world.sprites, &world.alias)?;
            }
            if dumps.contains(&d) {
                do_dump(&mut sb, &em, &map, &world, d);
                dumped += 1;
            }
        }
        let _ = writeln!(sb, "RAND end=0 dumps={dumped}");
        let ini = em
            .ini_file
            .clone()
            .or(if keep { saved_ini.clone() } else { None });
        let _ = writeln!(sb, "INI exists={}", u8::from(ini.is_some()));
        if let Some(text) = &ini {
            for line in text.lines() {
                let _ = writeln!(sb, "INI {line}");
            }
        }
        saved_ini = ini;
        let _ = writeln!(sb, "END {spec}");
    }
    Ok(sb)
}

/// What `media/lua/shared/Util/CustomTileProps.lua` does at `OnServerStarted` (the server has run it before a probe
/// does): for every tile of its `CustomTileProps_SetPair`, `_RemovePair` and `_SetFlag` tables, in that order,
/// `IsoSpriteManager.instance:getSprite(tile):getProperties()` gets `set(key, value)`, `unset(value)` or `set(flag)`.
/// The ported Lua VM does not expose the sprite manager yet, so the file (read from the install, not copied) is read
/// as the three table constructors it consists of; any other statement shape is an error, so a changed file cannot be
/// half applied unnoticed. UNKNOWN: whether other Lua of the install changes sprites at server start.
fn apply_custom_tile_props(
    root: &Path,
    sm: &mut IsoSpriteManager,
    alias: &TilePropertyAliasMap,
) -> Result<usize, String> {
    use std::collections::BTreeMap;
    let path = root.join("media/lua/shared/Util/CustomTileProps.lua");
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut code = String::new();
    for line in text.lines() {
        let mut in_str = false;
        let b = line.as_bytes();
        let mut end = line.len();
        for i in 0..b.len() {
            if b[i] == b'"' {
                in_str = !in_str;
            } else if !in_str && b[i] == b'-' && b.get(i + 1) == Some(&b'-') {
                end = i;
                break;
            }
        }
        code.push_str(&line[..end]);
        code.push('\n');
    }
    // the tables end where the handler `OnGameStart` (also run at `OnServerStarted`) begins
    let code = code
        .split_once("local function OnGameStart()")
        .map(|(tables, _)| tables.to_string())
        .ok_or("CustomTileProps.lua: no OnGameStart handler")?;
    let mut pairs: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();
    let mut removes: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut flags: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut rest = code.as_str();
    while let Some(i) = rest.find("CustomTileProps_") {
        rest = &rest[i + "CustomTileProps_".len()..];
        let kind_end = rest
            .find(|c: char| !c.is_ascii_alphabetic())
            .ok_or("CustomTileProps.lua: truncated")?;
        let kind = &rest[..kind_end];
        rest = rest[kind_end..].trim_start();
        if let Some(r) = rest.strip_prefix('=') {
            // `CustomTileProps_SetPair = {}`
            if r.trim_start().starts_with("{}") {
                continue;
            }
            return Err(format!(
                "CustomTileProps.lua: unexpected statement after {kind}"
            ));
        }
        let r = rest
            .strip_prefix("[\"")
            .ok_or_else(|| format!("CustomTileProps.lua: unexpected text after {kind}"))?;
        let q = r.find("\"]").ok_or("CustomTileProps.lua: bad tile name")?;
        let tile = r[..q].to_string();
        let r = r[q + 2..].trim_start();
        let r = r
            .strip_prefix('=')
            .ok_or("CustomTileProps.lua: missing =")?
            .trim_start();
        let r = r
            .strip_prefix('{')
            .ok_or("CustomTileProps.lua: missing {")?;
        let close = r.find('}').ok_or("CustomTileProps.lua: missing }")?;
        let body = &r[..close];
        rest = &r[close + 1..];
        let items: Vec<&str> = body
            .split(',')
            .map(str::trim)
            .filter(|x| !x.is_empty())
            .collect();
        match kind {
            "SetPair" => {
                let mut v: Vec<(String, String)> = Vec::new();
                for it in items {
                    let (k, val) = it
                        .split_once('=')
                        .ok_or_else(|| format!("CustomTileProps.lua: bad pair {it}"))?;
                    let val = val
                        .trim()
                        .strip_prefix('"')
                        .and_then(|x| x.strip_suffix('"'))
                        .ok_or_else(|| format!("CustomTileProps.lua: bad value {it}"))?;
                    let k = k.trim().to_string();
                    v.retain(|(ek, _)| *ek != k);
                    v.push((k, val.to_string()));
                }
                pairs.insert(tile, v);
            }
            "RemovePair" => {
                let mut v = Vec::new();
                for it in items {
                    let val = it
                        .strip_prefix('"')
                        .and_then(|x| x.strip_suffix('"'))
                        .ok_or_else(|| format!("CustomTileProps.lua: bad value {it}"))?;
                    v.push(val.to_string());
                }
                removes.insert(tile, v);
            }
            "SetFlag" => {
                let mut v = Vec::new();
                for it in items {
                    let name = it
                        .strip_prefix("IsoFlagType.")
                        .ok_or_else(|| format!("CustomTileProps.lua: bad flag {it}"))?;
                    v.push(name.to_string());
                }
                flags.insert(tile, v);
            }
            _ => return Err(format!("CustomTileProps.lua: unknown table {kind}")),
        }
    }
    let mut n = 0;
    for (tile, values) in &pairs {
        let r = sm.get_sprite(tile);
        for (k, v) in values {
            sm.sprite_mut(r).properties.set_str(alias, k, v, true);
        }
        n += 1;
    }
    for (tile, values) in &removes {
        let r = sm.get_sprite(tile);
        for v in values {
            sm.sprite_mut(r).properties.unset_str(alias, v);
        }
        n += 1;
    }
    for (tile, values) in &flags {
        let r = sm.get_sprite(tile);
        for v in values {
            let f = IsoFlagType::from_string(v);
            if f == IsoFlagType::MAX {
                return Err(format!("CustomTileProps.lua: unknown flag {v}"));
            }
            sm.sprite_mut(r).properties.set(f);
        }
        n += 1;
    }
    Ok(n)
}

/// ZombieProbe's survivor name lists: the game's own translations and the name lists of `MainCreationMethods.lua` (a
/// stand-in for the `OnGameBoot` Lua, see `SurvivorFactory::from_main_creation_methods`).
fn zombie_survivors(
    root: &Path,
) -> Result<zomboid_engine::zombie::characters::survivor_factory::SurvivorFactory, String> {
    use zomboid_engine::zombie::characters::survivor_factory::SurvivorFactory;
    use zomboid_engine::zombie::core::translator::Translator;
    let mut tr = Translator::new(root);
    tr.languages.init(root);
    tr.load_files()
        .map_err(|e| format!("Translator.loadFiles: {e:?}"))?;
    let lua_path = root.join("media/lua/shared/NPCs/MainCreationMethods.lua");
    let lua =
        std::fs::read_to_string(&lua_path).map_err(|e| format!("{}: {e}", lua_path.display()))?;
    SurvivorFactory::from_main_creation_methods(&lua, &mut |k| tr.get_text(k, &[]))
}

/// The lore options of a ZombieProbe scenario: the fixed base of ZombieProbe.BASE, then the scenario's overrides
/// ("Key=value,..."), set the way the probe sets them.
fn zombie_sandbox(
    overrides: Option<&str>,
) -> Result<zomboid_engine::zombie::sandbox_options::SandboxOptions, String> {
    use zomboid_engine::zombie::sandbox_options::SandboxOptions;
    const BASE: [(&str, i32); 12] = [
        ("Speed", 2),
        ("SprinterPercentage", 33),
        ("Strength", 2),
        ("Toughness", 2),
        ("Cognition", 3),
        ("DoorOpeningPercentage", 33),
        ("CrawlUnderVehicle", 5),
        ("Memory", 2),
        ("Sight", 2),
        ("Hearing", 2),
        ("ActiveOnly", 1),
        ("DisableFakeDead", 1),
    ];
    let mut sandbox = SandboxOptions::new();
    let mut want: Vec<(String, i32)> = BASE.iter().map(|(k, v)| (k.to_string(), *v)).collect();
    if let Some(opts) = overrides.filter(|o| !o.is_empty()) {
        for kv in opts.split(',') {
            let (k, v) = kv
                .split_once('=')
                .ok_or_else(|| format!("bad option {kv}"))?;
            let v: i32 = v.parse().map_err(|e| format!("{kv}: {e}"))?;
            match want.iter_mut().find(|(wk, _)| wk == k) {
                Some(e) => e.1 = v,
                None => want.push((k.to_string(), v)),
            }
        }
    }
    for (k, v) in &want {
        let name = format!("ZombieLore.{k}");
        let i = sandbox
            .defs()
            .iter()
            .position(|d| d.name == name)
            .ok_or_else(|| format!("no sandbox option {name}"))?;
        sandbox.parse(i, &v.to_string());
    }
    Ok(sandbox)
}

/// ZombieProbe's output for the port: `zombie.characters.IsoZombie` constructed over the same fixed lore options, global
/// `Rand` (java.util.Random(seed), draw counter at 0) and OutfitRNG seed. The name lists come from running the effect
/// of `MainCreationMethods.lua` (a stand-in for the `OnGameBoot` Lua, see `SurvivorFactory::from_main_creation_methods`)
/// with the game's own translations. Scenario "seed:count:ticks[:Key=value,...]" as in ZombieProbe.java.
pub fn zombie(root: &Path, scenarios: &[String]) -> Result<String, String> {
    use zomboid_engine::java::{float_to_string as fl, Random};
    use zomboid_engine::zombie::characters::iso_zombie::{IsoZombie, Lore, ZombieWorld};
    use zomboid_engine::zombie::core::skinnedmodel::population::outfit_rng::OutfitRng;

    let survivors = zombie_survivors(root)?;

    let mut sb = String::new();
    let _ = writeln!(
        sb,
        "NAMES female={} male={} surnames={}",
        survivors.female_forenames.len(),
        survivors.male_forenames.len(),
        survivors.surnames.len()
    );
    for (name, list) in [
        ("female", &survivors.female_forenames),
        ("male", &survivors.male_forenames),
        ("surname", &survivors.surnames),
    ] {
        let n = list.len();
        if n == 0 {
            return Err(format!("empty {name} list"));
        }
        let _ = writeln!(
            sb,
            "NAME {name} 0={} {}={} {}={}",
            list[0],
            n / 2,
            list[n / 2],
            n - 1,
            list[n - 1]
        );
    }
    for spec in scenarios {
        let p: Vec<&str> = spec.split(':').collect();
        if p.len() < 3 {
            return Err(format!("bad scenario {spec}"));
        }
        let seed: i64 = p[0].parse().map_err(|e| format!("{spec}: {e}"))?;
        let count: usize = p[1].parse().map_err(|e| format!("{spec}: {e}"))?;
        let ticks: i32 = p[2].parse().map_err(|e| format!("{spec}: {e}"))?;
        let sandbox = zombie_sandbox(p.get(3).copied())?;
        let _ = writeln!(sb, "SCENARIO {spec}");
        let lore = Lore::from_sandbox(&sandbox)?;
        let get = |n: &str| sandbox.get_int(&format!("ZombieLore.{n}")).unwrap_or(-1);
        let _ = writeln!(
            sb,
            "LORE speed={} sprinterPercentage={} strength={} toughness={} cognition={} doorOpeningPercentage={} crawlUnderVehicle={} memory={} sight={} hearing={} activeOnly={} disableFakeDead={}",
            get("Speed"),
            get("SprinterPercentage"),
            get("Strength"),
            get("Toughness"),
            get("Cognition"),
            get("DoorOpeningPercentage"),
            get("CrawlUnderVehicle"),
            get("Memory"),
            get("Sight"),
            get("Hearing"),
            get("ActiveOnly"),
            get("DisableFakeDead")
        );
        if ticks != 0 && count != 1 {
            return Err(format!(
                "{spec}: a scenario with ticks makes exactly one zombie"
            ));
        }
        let mut outfit_rng = OutfitRng::default();
        outfit_rng.set_seed(seed);
        let mut world = ZombieWorld {
            rand: RandAbstract::new(Random::new(seed)),
            outfit_rng,
            lore,
            survivors: survivors.clone(),
            survivor_id_count: 0,
            // ZombieProbe's ENV line: the booted server's GameTime multiplier and RainManager.isRaining()
            multiplier: 0.8,
            raining: false,
            anim_assets: if ticks > 0 {
                Some(zombie_anim_assets(root, false)?)
            } else {
                None
            },
            sp: None,
        };
        let b = |v: bool| u8::from(v);
        for i in 0..count {
            let mut z = IsoZombie::new(&mut world)?;
            let d = z
                .character
                .descriptor
                .as_ref()
                .ok_or("a zombie has no descriptor")?;
            let _ = writeln!(
                sb,
                "ZOMBIE {i} draws={} outfitSeed={}",
                world.rand.id(),
                world.outfit_rng.get_seed()
            );
            let _ = writeln!(
                sb,
                " desc id+{} forename={} surname={} female={} torso={} voice={} type={}",
                d.id,
                d.forename,
                d.surname,
                b(d.female),
                d.torso,
                d.voice_prefix,
                d.survivor_type.name()
            );
            let _ = writeln!(
                sb,
                " stats bravery={} aggressiveness={} compassion={} loner={} temper={} friendliness={} favourindoors={} loyalty={}",
                fl(d.bravery),
                fl(d.aggressiveness),
                fl(d.compassion),
                fl(d.loner),
                fl(d.temper),
                fl(d.friendliness),
                fl(d.favourindoors),
                fl(d.loyalty)
            );
            let c = &z.character;
            let _ = writeln!(
                sb,
                " body health={} weight={} width={} patience={} dir={} fwd={},{}",
                fl(c.health),
                fl(c.moving.weight),
                fl(c.moving.width),
                c.patience,
                c.forward_iso_direction().name(),
                fl(c.forward_direction.x),
                fl(c.forward_direction.y)
            );
            let _ = writeln!(
                sb,
                " look palette={} sprite={} tint={},{},{} zombieId={} hurt={}",
                z.palette,
                z.sprite_name,
                fl(z.tint[0]),
                fl(z.tint[1]),
                fl(z.tint[2]),
                z.zombie_id,
                z.hurt_sound
            );
            let _ = writeln!(
                sb,
                " stats2 speedType={} strength={} cognition={} memory={} sight={} hearing={} walkType={} walkVariant={} speedMod={} lunger={} running={} crawling={} inactive={} canCrawlUnder={} turnDelta={}",
                z.speed_type,
                z.strength,
                z.cognition,
                z.memory,
                z.sight,
                z.hearing,
                z.walk_type.as_deref().unwrap_or("null"),
                z.walk_variant,
                fl(z.speed_mod),
                b(z.lunger),
                b(z.running),
                b(z.crawling),
                b(z.inactive),
                b(z.can_crawl_under_vehicle),
                fl(c.turn_delta_normal)
            );
            let _ = write!(sb, " blood");
            for v in &c.human_visual.blood {
                let _ = write!(sb, " {}", *v as u8);
            }
            sb.push('\n');
            if ticks > 0 {
                let chunks = zombie_chunks(root)?;
                let grid = ZombieGrid::new(&chunks);
                zombie_ticks(&mut sb, &mut z, &mut world, &grid, ticks)?;
            }
        }
    }
    Ok(sb)
}

/// The game's own zombie action group, "zombie" animation set and the human skeleton's animation clips.
///
/// `keyframes`: the clips load their bone keyframes on demand (the deferred movement needs them; the `zombie` case's
/// server zombie does not move by animation).
fn zombie_anim_assets(
    root: &Path,
    keyframes: bool,
) -> Result<zomboid_engine::zombie::characters::iso_zombie::ZombieAnimAssets, String> {
    use zomboid_engine::zombie::characters::action::action_group::ActionGroups;
    use zomboid_engine::zombie::core::skinnedmodel::advancedanimation::animation_set::AnimationSets;
    use zomboid_engine::zombie::core::skinnedmodel::animation::animation_clips::AnimationClips;
    use zomboid_engine::zombie::zomboid_file_system::ZomboidFileSystem;
    let fs = ZomboidFileSystem::init(root)?;
    let mut groups = ActionGroups::default();
    let mut sets = AnimationSets::default();
    let (mut clips, _dups) = AnimationClips::load(&fs, "Human")?;
    if keyframes {
        clips = clips.with_keyframes(&fs, "Human")?;
    }
    let (motion_scale, rotation_scale) =
        zomboid_engine::zombie::core::skinnedmodel::advancedanimation::advanced_animator::load_defaults(&fs)?;
    Ok(
        zomboid_engine::zombie::characters::iso_zombie::ZombieAnimAssets {
            action_group: groups.get_action_group_rc(&fs, "zombie"),
            anim_set: sets.get_animation_set_rc(&fs, "zombie", false)?,
            clips: std::rc::Rc::new(clips),
            motion_scale,
            rotation_scale,
        },
    )
}

/// BodyProbe's scenarios on the port: "seed:ticks:run:every:show[:options]" (see tests/original_probe/BodyProbe.java).
/// The probe's player stands on a real square and is built by the game's constructors; the port's body is built from
/// the same defaults and the options set the same fields.
pub fn body(root: &Path, cases: &Path) -> Result<String, String> {
    use zomboid_engine::se::krka::kahlua::vm::object::Object;
    use zomboid_engine::zombie::characters::body_damage::body_fields::{
        body_field, char_field, node_field, part_field, thermo_field, BODY_FIELDS, CHAR_FIELDS,
        NODE_FIELDS, PART_FIELDS, THERMO_FIELDS,
    };
    use zomboid_engine::zombie::characters::body_damage::body_sim::{BodyWorld, PlayerBody};
    use zomboid_engine::zombie::characters::stats::{CharacterStat, ALL_STATS};
    use zomboid_engine::zombie::game_time::GameTime;
    use zomboid_engine::zombie::iso::weather::climate_manager::{
        FLOAT_HUMIDITY, FLOAT_PRECIPITATION_INTENSITY, FLOAT_TEMPERATURE,
    };
    use zomboid_engine::zombie::sandbox_options::SandboxOptions;
    use zomboid_engine::zombie::scripting::objects::resource_location::ResourceLocation;

    fn hx(v: f32) -> String {
        format!("{:x}", v.to_bits())
    }
    fn parse_f(v: &str) -> Result<f32, String> {
        v.parse::<f32>().map_err(|e| format!("float {v}: {e}"))
    }
    use zomboid_engine::zombie::characters::body_damage::body_sim::SquareFacts;
    use zomboid_engine::zombie::iso::weather::climate_manager::ClimateManager;
    use zomboid_engine::zombie::lua::lua_manager::{run_lua, LuaManager};
    use zomboid_engine::zombie::scripting::objects::moodle_type::ALL_MOODLE_TYPES;
    let text = std::fs::read_to_string(cases).map_err(|e| format!("{}: {e}", cases.display()))?;
    let mut sb = String::new();
    // ZomboidGlobals.Load of the real defines.lua (the table the game reads them from)
    let globals = {
        let mut lm = LuaManager::init(root).map_err(|e| format!("lua: {e:?}"))?;
        let defines = root.join("media/lua/shared/defines.lua");
        run_lua(
            &lm.loader,
            &lm.platform,
            &lm.env,
            &mut lm.thread,
            &defines.to_string_lossy(),
        )
        .map_err(|e| format!("defines.lua: {e:?}"))?;
        let mut g = zomboid_engine::zombie::zomboid_globals::ZomboidGlobals::default();
        g.load(&lm.env)
            .map_err(|e| format!("ZomboidGlobals.Load: {e:?}"))?;
        g
    };
    for (name, bits) in globals.fields() {
        let _ = writeln!(sb, "GLOBALS {name} {bits}");
    }
    for spec in text
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
        .map(str::trim)
    {
        let p: Vec<&str> = spec.splitn(6, ':').collect();
        if p.len() < 5 {
            return Err(format!("body scenario {spec}"));
        }
        let seed: i64 = p[0].parse().map_err(|e| format!("seed: {e}"))?;
        let ticks: i32 = p[1].parse().map_err(|e| format!("ticks: {e}"))?;
        let run: Vec<&str> = p[2].split('+').collect();
        let every: i32 = p[3].parse().map_err(|e| format!("every: {e}"))?;
        let show: Vec<&str> = p[4].split('+').collect();
        let mut o: Vec<(String, String)> = Vec::new();
        if p.len() > 5 && !p[5].is_empty() {
            for kv in p[5].split(',') {
                let (k, v) = kv.split_once('=').ok_or_else(|| format!("option {kv}"))?;
                o.push((k.to_string(), v.to_string()));
            }
        }
        let opt = |name: &str, default: &str| -> String {
            o.iter()
                .find(|(k, _)| k == name)
                .map(|(_, v)| v.clone())
                .unwrap_or_else(|| default.to_string())
        };
        let _ = writeln!(sb, "SCENARIO {spec}");
        let gt = GameTime {
            multiplier: parse_f(&opt("mult", "1.0"))?,
            minutes_per_day: parse_f(&opt("mpd", "30.0"))?,
            // the booted server's clock stands at 9:00 of night 0
            time_of_day: parse_f(&opt("tod", "9.0"))?,
            nights_survived: opt("nights", "0")
                .parse()
                .map_err(|e| format!("nights: {e}"))?,
            ..GameTime::default()
        };
        let mut sandbox = SandboxOptions::new();
        for (name, v) in [
            ("Nutrition", 1.0),
            ("MuscleStrainFactor", 0.7),
            ("WoundInfectionFactor", 1.0),
        ] {
            if !sandbox.set_by_name(name, &Object::Number(v)) {
                return Err(format!("sandbox option {name}"));
            }
        }
        for (k, v) in &o {
            if let Some(name) = k.strip_prefix("sb.") {
                let d: f64 = v.parse().map_err(|e| format!("sandbox {k}: {e}"))?;
                if !sandbox.set_by_name(name, &Object::Number(d)) {
                    return Err(format!("sandbox option {name}"));
                }
            }
        }
        let mut climate = ClimateManager::default();
        let mut world = BodyWorld::new(gt, sandbox, globals.clone(), climate.clone(), seed);
        world.hydro_power_on = true;
        for (k, v) in &o {
            match k.as_str() {
                "temp" => climate.climate_floats[FLOAT_TEMPERATURE].final_value = parse_f(v)?,
                "wind" => climate.wind_power = parse_f(v)?,
                "daylight" => climate.day_light_lagged = parse_f(v)?,
                "night" => climate.night_lagged = parse_f(v)?,
                "rain" => {
                    climate.climate_floats[FLOAT_PRECIPITATION_INTENSITY].final_value = parse_f(v)?
                }
                "humidity" => climate.climate_floats[FLOAT_HUMIDITY].final_value = parse_f(v)?,
                _ => {}
            }
        }
        world.climate = climate;
        let mut pl = PlayerBody::new(world);
        // the headless player the probe builds: outdoors on (8280, 11670, 0), no `square` field, nothing carried
        pl.ch.current_square = Some(SquareFacts {
            in_room: false,
            have_roof: false,
            z: 0,
        });
        pl.ch.bed_type = "averageBed".to_string();
        pl.ch.sleep_blocked = true;
        for (k, v) in &o {
            match k.as_str() {
                _ if k.starts_with("stat.") => {
                    let st = CharacterStat::by_id(&k[5..]).ok_or_else(|| format!("stat {k}"))?;
                    pl.stats.set(st, parse_f(v)?);
                }
                "weight" => {
                    let _ = pl
                        .nutrition
                        .set_weight(v.parse().map_err(|e| format!("weight: {e}"))?);
                }
                "cal" => pl.nutrition.set_calories(parse_f(v)?),
                "carb" => pl.nutrition.set_carbohydrates(parse_f(v)?),
                "lip" => pl.nutrition.set_lipids(parse_f(v)?),
                "prot" => pl.nutrition.set_proteins(parse_f(v)?),
                "trait" => {
                    for t in v.split('+') {
                        pl.ch.traits.set(ResourceLocation::of(t)?, true);
                    }
                }
                "moving" => pl.ch.player_moving = v == "1",
                "running" => pl.ch.running = v == "1",
                "sprint" => pl.ch.sprinting = v == "1",
                "asleep" => pl.ch.asleep = v == "1",
                "sit" => pl.ch.sitting = v == "1",
                "bed" => pl.ch.bed_type = v.clone(),
                "delay" => pl.ch.delay_to_actually_sleep = parse_f(v)?,
                "fitness" => pl.ch.perk_fitness = v.parse().map_err(|e| format!("fitness: {e}"))?,
                "sleep" => pl.ch.sleep_blocked = v != "1",
                "sqfield" => pl.ch.square_field_set = v == "1",
                "strength" => {
                    pl.ch.perk_strength = v.parse().map_err(|e| format!("strength: {e}"))?
                }
                "zombies" => {
                    pl.stats.num_visible_zombies = v.parse().map_err(|e| format!("zombies: {e}"))?
                }
                "sneak" => pl.ch.sneaking = v == "1",
                // fields set directly, as a loaded save would: no Rand draw of a setter is made
                _ if k.starts_with("bp.") => {
                    let mut it = k.splitn(3, '.');
                    let (_, idx, name) = (it.next(), it.next(), it.next());
                    let (Some(idx), Some(name)) = (idx, name) else {
                        return Err(format!("option {k}"));
                    };
                    let idx: usize = idx.parse().map_err(|e| format!("{k}: {e}"))?;
                    let part = pl
                        .body
                        .body_parts
                        .get_mut(idx)
                        .ok_or_else(|| format!("{k}: no part"))?;
                    part_field(part, name)?.assign(v)?;
                }
                _ if k.starts_with("bd.") => body_field(&mut pl.body, &k[3..])?.assign(v)?,
                _ if k.starts_with("ch.") => char_field(&mut pl.ch, &k[3..])?.assign(v)?,
                _ if k.starts_with("th.") => {
                    let t = pl
                        .body
                        .thermoregulator
                        .as_mut()
                        .ok_or("no thermoregulator")?;
                    thermo_field(t, &k[3..])?.assign(v)?;
                }
                _ if k.starts_with("node.") => {
                    let mut it = k.splitn(3, '.');
                    let (_, idx, name) = (it.next(), it.next(), it.next());
                    let (Some(idx), Some(name)) = (idx, name) else {
                        return Err(format!("option {k}"));
                    };
                    let idx: usize = idx.parse().map_err(|e| format!("{k}: {e}"))?;
                    let t = pl
                        .body
                        .thermoregulator
                        .as_mut()
                        .ok_or("no thermoregulator")?;
                    let n = t
                        .nodes
                        .get_mut(&idx)
                        .ok_or_else(|| format!("{k}: no node"))?;
                    node_field(n, name)?.assign(v)?;
                }
                "building" => pl.ch.in_building = v == "1",
                // the square the probe found for `room`: its facts, as FACTS prints them
                "inroom" => {
                    if let Some(sq) = pl.ch.current_square.as_mut() {
                        sq.in_room = v == "1";
                    }
                }
                "roof" => {
                    if let Some(sq) = pl.ch.current_square.as_mut() {
                        sq.have_roof = v == "1";
                    }
                }
                "floor" => {
                    if let Some(sq) = pl.ch.current_square.as_mut() {
                        sq.z = v.parse().map_err(|e| format!("floor: {e}"))?;
                    }
                }
                k if k.starts_with("sb.")
                    || matches!(
                        k,
                        "mult"
                            | "mpd"
                            | "temp"
                            | "wind"
                            | "daylight"
                            | "night"
                            | "room"
                            | "rain"
                            | "humidity"
                            | "tod"
                            | "nights"
                    ) => {}
                _ => return Err(format!("option {k}")),
            }
        }
        let gt = &pl.world.game_time;
        let _ = writeln!(
            sb,
            "ENV tod={} nights={} mult={} delta={} gwsslu={} mpd={} dmpd={}",
            hx(gt.time_of_day),
            gt.nights_survived,
            hx(gt.get_multiplier()),
            hx(gt.get_time_delta()),
            hx(gt.get_game_world_seconds_since_last_update()),
            hx(gt.minutes_per_day),
            hx(gt.get_delta_minutes_per_day())
        );
        let _ = write!(sb, "SANDBOX");
        for name in [
            "Nutrition",
            "InjurySeverity",
            "StatsDecrease",
            "ZombieLore.Mortality",
            "ZombieLore.Transmission",
            "MuscleStrainFactor",
            "WoundInfectionFactor",
            "DecayingCorpseHealthImpact",
            "EndRegen",
            "BoneFracture",
            "ClothingDegradation",
        ] {
            let i = pl
                .world
                .sandbox
                .get_option_by_name(name)
                .ok_or_else(|| format!("sandbox option {name}"))?;
            let _ = write!(sb, " {name}={}", pl.world.sandbox.value_as_string(i));
        }
        sb.push('\n');
        let sq = pl.ch.current_square;
        let _ = writeln!(
            sb,
            "FACTS inroom={} z={} hydro={} capw={} maxw={} lastsq={} vehicle={} building={} toxic={} sleepblock={} allasleep={} instance={} heatsources=0 airtemp={} airwind={} windchill={} fitnessperk={} strperk={} hours={:x} corpses={} cdm={} mwbase={} mwdelta={} roof={} health={} allowsprint={} movespeed={} rain={} humidity={}",
            u8::from(sq.is_some_and(|s| s.in_room)),
            sq.map_or(0, |s| s.z),
            u8::from(pl.world.hydro_power_on),
            hx(pl.ch.capacity_weight),
            pl.ch.max_weight,
            if pl.ch.last_square_same { "same" } else { "null" },
            u8::from(pl.ch.vehicle_inside_temperature.is_some()),
            u8::from(pl.ch.in_building),
            u8::from(pl.ch.in_toxic_building),
            u8::from(pl.ch.sleep_blocked),
            u8::from(pl.ch.all_players_asleep),
            u8::from(pl.ch.is_local_instance),
            hx(pl.ch.air_temperature_for_character(&pl.world, false)),
            hx(pl.ch.air_temperature_for_character(&pl.world, true)),
            hx(pl.ch.wind_chill_amount(&pl.world)),
            pl.ch.perk_fitness,
            pl.ch.perk_strength,
            pl.ch.hours_survived.to_bits(),
            pl.ch.corpse_count,
            hx(pl.ch.clothing_discomfort_modifier),
            pl.ch.max_weight_base,
            hx(pl.ch.max_weight_delta),
            u8::from(sq.is_some_and(|s| s.have_roof)),
            hx(pl.ch.health),
            u8::from(pl.ch.allow_sprint),
            hx(pl.ch.move_speed),
            hx(pl.world.climate.get_rain_intensity()),
            hx(pl.world.climate.get_humidity())
        );
        for n in 1..=ticks {
            for r in &run {
                match *r {
                    "nutrition" => pl.update_nutrition(),
                    "calc" => pl.calculate_stats()?,
                    "moodles" => pl.update_moodles()?,
                    "body" => pl.update_body()?,
                    _ => return Err(format!("run {r}")),
                }
            }
            if n <= 3 || n % every == 0 || n == ticks {
                let _ = write!(sb, "TICK {n}");
                for sec in &show {
                    match *sec {
                        "stats" => {
                            sb.push_str(" stats");
                            for st in ALL_STATS {
                                let _ = write!(sb, " {}", hx(pl.stats.get(st)));
                            }
                        }
                        "nut" => {
                            let nu = &pl.nutrition;
                            let _ = write!(
                                sb,
                                " nut {} {} {} {} {:x} {}{}{} {} {} {}",
                                hx(nu.calories),
                                hx(nu.proteins),
                                hx(nu.lipids),
                                hx(nu.carbohydrates),
                                nu.weight.to_bits(),
                                u8::from(nu.inc_weight),
                                u8::from(nu.inc_weight_lot),
                                u8::from(nu.dec_weight),
                                nu.updated_weight,
                                hx(nu.calories_max),
                                hx(nu.calories_min)
                            );
                        }
                        "moodles" => {
                            sb.push_str(" moodles");
                            for t in ALL_MOODLE_TYPES {
                                let m = &pl.moodles.moodles[t as usize];
                                let _ = write!(sb, " {}", m.level);
                                if matches!(
                                    t,
                                    zomboid_engine::zombie::scripting::objects::moodle_type::MoodleType::Hyperthermia
                                        | zomboid_engine::zombie::scripting::objects::moodle_type::MoodleType::Hypothermia
                                ) {
                                    let _ = write!(
                                        sb,
                                        "/{}{}{}",
                                        m.chevron_count,
                                        u8::from(m.chevron_is_up),
                                        m.chevron_color as u8
                                    );
                                }
                            }
                        }
                        "traits" => {
                            sb.push_str(" traits");
                            for t in &pl.ch.traits.known_traits {
                                let _ = write!(sb, " {t}");
                            }
                        }
                        "draws" => {
                            let _ = write!(sb, " draws={}", pl.world.rand.id());
                        }
                        "thermo" => {
                            let t = pl
                                .body
                                .thermoregulator
                                .as_mut()
                                .ok_or("no thermoregulator")?;
                            sb.push_str(" thermo");
                            for name in THERMO_FIELDS {
                                let _ = write!(sb, " {}", thermo_field(t, name)?.show());
                            }
                            for n in t.nodes.values_mut() {
                                sb.push_str(" |");
                                for name in NODE_FIELDS {
                                    let _ = write!(sb, " {}", node_field(n, name)?.show());
                                }
                            }
                        }
                        "parts" => {
                            sb.push_str(" parts");
                            let sev = pl.world.sandbox_int("InjurySeverity");
                            for part in &mut pl.body.body_parts {
                                let _ = write!(sb, " | {}", hx(part.get_pain(sev)));
                                for name in PART_FIELDS {
                                    let _ = write!(sb, " {}", part_field(part, name)?.show());
                                }
                            }
                        }
                        "bodyst" => {
                            sb.push_str(" bodyst");
                            for name in BODY_FIELDS {
                                let _ = write!(sb, " {}", body_field(&mut pl.body, name)?.show());
                            }
                            sb.push_str(" |");
                            for name in CHAR_FIELDS {
                                let _ = write!(sb, " {}", char_field(&mut pl.ch, name)?.show());
                            }
                        }
                        _ => return Err(format!("show {sec}")),
                    }
                }
                sb.push('\n');
            }
        }
    }
    Ok(sb)
}

/// The chunks 1034..=1035 x 1458..=1459 of the real map as ZombieProbe loads them, with their squares' properties.
fn zombie_chunks(root: &Path) -> Result<Vec<IsoChunk>, String> {
    zombie_chunks_alias(root).map(|(chunks, _, _)| chunks)
}

/// `zombie_chunks` with the world's tile property aliases (the squares' floor and slope tests need them).
fn zombie_chunks_alias(
    root: &Path,
) -> Result<
    (
        Vec<IsoChunk>,
        zomboid_engine::zombie::core::tile_property_alias_map::TilePropertyAliasMap,
        zomboid_engine::zombie::iso::sprite::iso_sprite_manager::IsoSpriteManager,
    ),
    String,
> {
    let mut rand = RandAbstract::new(java::Random::new(12345));
    let alarm = AlarmSettings {
        last_stand: false,
        power_grid_exists: true,
        alarm: 1,
        alarm_decay: 1,
    };
    let mut world = IsoWorld::init(root, "Muldraugh, KY", &|_| false, &alarm, None, &mut rand)?;
    let mut chunks = Vec::new();
    {
        let IsoWorld {
            sprites,
            alias,
            tile_definitions,
            meta_grid,
            ..
        } = &mut world;
        let mut ctx = LoadContext {
            sprites,
            alias,
            glass_removed_window_sprite_map: &tile_definitions.glass_removed_window_sprite_map,
            smashed_window_sprite_map: &tile_definitions.smashed_window_sprite_map,
            meta_grid,
            rand: &mut rand,
            locked_houses: 6,
            missing_tiles: Vec::new(),
            lotpack_cache: None,
        };
        for wx in 1034..=1035 {
            for wy in 1458..=1459 {
                *ctx.rand = RandAbstract::new(java::Random::new(12345));
                let mut ch = IsoChunk::new(wx, wy);
                cell_loader::load_cell_binary_chunk(&mut ctx, wx, wy, &mut ch, &mut Vec::new());
                // ActionContextProbe.loadChunks: `ch.loadInWorldStreamerThread()` (the chunk is registered in the
                // ServerMap only afterwards, so no square sees a neighbouring chunk's squares)
                ch.load_in_world_streamer_thread(&*ctx.sprites, &*ctx.meta_grid, ctx.alias);
                chunks.push(ch);
            }
        }
    }
    let IsoWorld { sprites, alias, .. } = world;
    Ok((chunks, alias, sprites))
}

struct ZombieGrid<'a>(std::collections::HashMap<(i32, i32, i32), &'a IsoGridSquare>);
impl<'a> ZombieGrid<'a> {
    fn new(chunks: &'a [IsoChunk]) -> Self {
        let mut m = std::collections::HashMap::new();
        for ch in chunks {
            for z in ch.get_min_level()..=ch.get_max_level() {
                for y in 0..8 {
                    for x in 0..8 {
                        if let Some(sq) = ch.get_grid_square(x, y, z) {
                            m.insert((sq.x, sq.y, sq.z), sq);
                        }
                    }
                }
            }
        }
        ZombieGrid(m)
    }
}
impl GetSquare for ZombieGrid<'_> {
    fn get_grid_square(&self, x: i32, y: i32, z: i32) -> Option<&IsoGridSquare> {
        self.0.get(&(x, y, z)).copied()
    }
}

/// ZombieProbe.tickPhase for the port: the zombie stands at (8280.5, 11670.5, 0), alone, and is updated `ticks` times.
fn zombie_ticks(
    sb: &mut String,
    z: &mut zomboid_engine::zombie::characters::iso_zombie::IsoZombie,
    world: &mut zomboid_engine::zombie::characters::iso_zombie::ZombieWorld,
    squares: &ZombieGrid,
    ticks: i32,
) -> Result<(), String> {
    use zomboid_engine::java::float_to_string as fl;
    z.place(8280.5, 11670.5, 0.0);
    let _ = writeln!(
        sb,
        "ENV raining={} multiplier={} thirty={}",
        world.raining,
        fl(world.multiplier),
        fl(world.thirty_fps_multiplier())
    );
    let _ = writeln!(
        sb,
        "PLACED x={} y={} z={} sqMoving=1",
        fl(z.character.moving.x),
        fl(z.character.moving.y),
        fl(z.character.moving.z)
    );
    let mut prev = world.rand.id();
    for n in 1..=ticks {
        z.tick(world, squares)?;
        let draws = world.rand.id();
        if draws - prev != 1 || n < 4 || n % 250 == 0 {
            let m = &z.character.moving;
            let _ = writeln!(
                sb,
                "TICK {n} x={} y={} z={} state={} timer={} repath={} tsf={} tsrts={} corpse={} idleTicks={} goal={} tgt={},{},{} draws={draws}",
                fl(m.x),
                fl(m.y),
                fl(m.z),
                z.current_state.map_or("null", |s| s.name()),
                fl(m.state_event_delay_timer),
                fl(z.allow_repath_delay),
                fl(z.time_since_seen_flesh),
                fl(z.time_since_respond_to_sound),
                fl(z.check_for_corpse_timer),
                z.idle_tick_count,
                z.path.goal.name(),
                fl(z.path.target_x),
                fl(z.path.target_y),
                fl(z.path.target_z)
            );
        }
        prev = draws;
    }
    let _ = writeln!(sb, "END ticks={ticks} draws={}", world.rand.id());
    Ok(())
}

fn hex(b: &[u8]) -> String {
    let mut s = String::with_capacity(b.len() * 2);
    for x in b {
        let _ = write!(s, "{x:02x}");
    }
    s
}

/// The save's `map_t.bin` through `GameTime.load` as a single-player game reads it, into a new game time and climate;
/// what the load sets, in GameTimeProbe.java's format.
pub fn gametime(save: &Path) -> Result<String, String> {
    use save_dump::{dh, fh, table};
    use zomboid_engine::zombie::game_time::{CorePoisonous, GameTime};
    use zomboid_engine::zombie::iso::map_meta::MetaSide;
    use zomboid_engine::zombie::iso::weather::climate_color_info::ClimateColorInfo;
    use zomboid_engine::zombie::iso::weather::climate_manager::{AirFront, ClimateManager};
    let p = save.join("map_t.bin");
    let data = std::fs::read(&p).map_err(|e| format!("{}: {e}", p.display()))?;
    let mut gt = GameTime::default();
    let mut core = CorePoisonous::default();
    let mut cm = ClimateManager::default();
    let mut log = Vec::new();
    gt.load(
        &data,
        -1,
        MetaSide::SinglePlayer,
        &mut core,
        &mut cm,
        &mut log,
    )?;
    for l in &log {
        eprintln!("{l}");
    }
    let b = |v: bool| u8::from(v);
    let null = |s: &Option<String>| s.clone().unwrap_or_else(|| "null".to_string());
    let front = |f: &AirFront| {
        format!(
            "{} {} {} {} {} {} {} {}",
            fh(f.days),
            fh(f.max_noise),
            fh(f.total_noise),
            f.type_,
            fh(f.strength),
            fh(f.front_wind_angle_degrees),
            fh(f.noise_cache[0]),
            fh(f.noise_cache[1])
        )
    };
    let color = |c: &ClimateColorInfo| {
        c.interior
            .iter()
            .chain(c.exterior.iter())
            .map(|v| format!(" {}", fh(*v)))
            .collect::<String>()
    };
    let idx = |i: Option<usize>| i.map_or(-1, |i| i as i64);
    let mut sb = String::new();
    let _ = writeln!(
        sb,
        "GT {} {} {} {} {} {} {} {} {} {} {} {} {} {} {}",
        fh(gt.multiplier),
        gt.nights_survived,
        gt.target_zombies,
        fh(gt.last_time_of_day),
        fh(gt.time_of_day),
        gt.day,
        gt.month,
        gt.year,
        gt.helicopter_day1,
        gt.helicopter_time1_start,
        gt.helicopter_time1_end,
        gt.minutes_stamp,
        gt.get_hour(),
        gt.get_minutes(),
        dh(gt.get_world_age_hours())
    );
    let _ = writeln!(
        sb,
        "GTT {}",
        gt.table.as_ref().map_or("null".to_string(), table)
    );
    let _ = writeln!(
        sb,
        "CORE {} {}",
        null(&core.poisonous_berry),
        null(&core.poisonous_mushroom)
    );
    let _ = writeln!(
        sb,
        "CM {} {} {} {} {} {} {} {} {}",
        dh(cm.simplex_offset_a),
        dh(cm.simplex_offset_b),
        dh(cm.simplex_offset_c),
        dh(cm.simplex_offset_d),
        fh(cm.snow_frac_now),
        fh(cm.snow_strength),
        b(cm.can_do_winter_sprites),
        b(cm.day_do_fog),
        fh(cm.day_fog_strength)
    );
    let _ = writeln!(sb, "CF {}", front(&cm.current_front));
    let wp = &cm.weather_period;
    let _ = writeln!(sb, "WP {}", b(wp.is_running));
    if wp.is_running {
        let _ = writeln!(
            sb,
            "WPR {} {} {} {} {} {} {} {} {} {}",
            wp.weather_stage_index,
            fh(wp.current_strength),
            fh(wp.rain_threshold),
            b(wp.is_thunder_storm),
            b(wp.is_tropical_storm),
            b(wp.is_blizzard),
            fh(wp.wind_angle_dir_mod),
            dh(wp.duration),
            wp.weather_stages.len(),
            idx(wp.current_stage)
        );
        let _ = writeln!(sb, "WPF {}", front(&wp.front_cache));
        for (i, s) in wp.weather_stages.iter().enumerate() {
            let lerp = |v: Option<_>| {
                v.map_or(
                    "null",
                    |v: zomboid_engine::zombie::iso::weather::weather_period::StrLerpVal| v.name(),
                )
            };
            let _ = writeln!(
                sb,
                "WS {i} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {}",
                s.stage_id,
                dh(s.stage_start),
                dh(s.stage_end),
                dh(s.stage_duration),
                fh(s.entry_strength),
                fh(s.exit_strength),
                fh(s.target_strength),
                lerp(s.lerp_mid_val),
                lerp(s.lerp_end_val),
                b(s.has_started_cloud),
                null(&s.mod_id),
                fh(s.fog_strength),
                idx(s.previous_stage),
                idx(s.next_stage),
                b(s.creation_finished)
            );
        }
        let _ = writeln!(sb, "WPC{}", color(wp.cloud_color()));
    }
    for (i, c) in cm.thunder_storm.clouds.iter().enumerate() {
        if c.is_running {
            let _ = writeln!(
                sb,
                "TS {i} 1 {} {} {} {} {} {} {} {} {} {} {} {}",
                c.start_x,
                c.start_y,
                c.end_x,
                c.end_y,
                fh(c.radius),
                fh(c.angle),
                fh(c.strength),
                fh(c.thunder_ratio),
                dh(c.start_time),
                dh(c.end_time),
                dh(c.duration),
                fh(c.percentage_offset)
            );
        } else {
            let _ = writeln!(sb, "TS {i} 0");
        }
    }
    let _ = writeln!(
        sb,
        "CMT {}",
        cm.mod_data_table.as_ref().map_or("null".to_string(), table)
    );
    Ok(sb)
}

/// The save's `global_mod_data.bin` through `GlobalModData.load` and `iTrack.bin` through `InstanceTracker.load`;
/// the tables in `collectTableNames` order, the groups in their HashMap order, each exported as text and CSV with keys
/// sorted (pairs with ModDataProbe.java).
pub fn moddata(save: &Path) -> Result<String, String> {
    use zomboid_engine::java::hash_map_order;
    use zomboid_engine::zombie::iso::instance_tracker::{Format, InstanceTracker, Sort};
    use zomboid_engine::zombie::world::moddata::global_mod_data::GlobalModData;
    let read = |name: &str| {
        let p = save.join(name);
        std::fs::read(&p).map_err(|e| format!("{}: {e}", p.display()))
    };
    let gmd_data = read("global_mod_data.bin")?;
    let it_data = read("iTrack.bin")?;
    let mut sb = String::new();
    let mut gmd = GlobalModData::default();
    gmd.load(Some(&gmd_data))?;
    let tags = gmd.collect_table_names();
    let _ = writeln!(sb, "GMD {}", tags.len());
    for t in &tags {
        let table = gmd.get(t).map_or("null".to_string(), save_dump::table);
        let _ = writeln!(sb, "GMT {t} {table}");
    }
    let mut it = InstanceTracker::default();
    it.load(Some(&it_data))?;
    let _ = writeln!(sb, "IG {}", it.instance_groups.len());
    let names: Vec<&str> = it.instance_groups.iter().map(|(g, _)| g.as_str()).collect();
    for i in hash_map_order(&names) {
        let g = names[i];
        let _ = writeln!(sb, "IGN {g}");
        sb.push_str(&it.export_group(g, Format::Text, Sort::Key, "\n"));
        sb.push_str(&it.export_group(g, Format::Csv, Sort::Key, "\n"));
    }
    Ok(sb)
}

/// The outfits of `media/clothing/clothing.xml` (OutfitManager.init) sorted by name, and PersistentOutfits built from
/// them with the save's `z_outfits.bin` loaded (pairs with OutfitsProbe.java).
pub fn outfits(root: &Path, save: &Path) -> Result<String, String> {
    use save_dump::fh;
    use zomboid_engine::java::{equals_ignore_case, string_compare_to};
    use zomboid_engine::zombie::core::skinnedmodel::population::clothing_item_reference::ClothingItemReference;
    use zomboid_engine::zombie::core::skinnedmodel::population::outfit::Outfit;
    use zomboid_engine::zombie::core::skinnedmodel::population::outfit_manager::OutfitManager;
    use zomboid_engine::zombie::persistent_outfits::{PersistentOutfits, FEMALE_BIT, NO_HAT_BIT};
    fn item(sb: &mut String, r: &ClothingItemReference, depth: usize) {
        let _ = writeln!(
            sb,
            "I {depth} {} {} {} {} {}",
            fh(r.probability),
            r.item_guid.as_deref().unwrap_or("null"),
            u8::from(r.randomized),
            u8::from(r.immutable),
            r.sub_items.len()
        );
        for s in &r.sub_items {
            item(sb, s, depth + 1);
        }
    }
    let om = OutfitManager::init(root)?.ok_or("no media/clothing/clothing.xml")?;
    let mut sb = String::new();
    let mut names: Vec<String> = Vec::new();
    for female in [true, false] {
        let mut l: Vec<&Outfit> = if female {
            om.female_outfits.iter().collect()
        } else {
            om.male_outfits.iter().collect()
        };
        l.sort_by(|a, b| string_compare_to(&a.name, &b.name));
        let map_len = if female {
            om.female_outfit_map_len()
        } else {
            om.male_outfit_map_len()
        };
        let _ = writeln!(
            sb,
            "OM {} {} {map_len}",
            if female { "F" } else { "M" },
            l.len()
        );
        for o in l {
            if !names.iter().any(|n| equals_ignore_case(n, &o.name)) {
                names.push(o.name.clone());
            }
            let same = om.get_specific_outfit(female, &o.name);
            let upper = om.get_specific_outfit(female, &o.name.to_uppercase());
            let _ = writeln!(
                sb,
                "O {} {} {} {} {} {} {} {} {} [{}] [{}] {} {} {}",
                o.name,
                u8::from(o.top),
                u8::from(o.pants),
                u8::from(o.allow_pants_hue),
                u8::from(o.allow_pants_tint),
                u8::from(o.allow_top_tint),
                u8::from(o.allow_tshirt_decal),
                o.mod_id.as_deref().unwrap_or("null"),
                u8::from(o.immutable),
                o.top_textures.join(","),
                o.pants_textures.join(","),
                o.items.len(),
                u8::from(same.is_some_and(|s| std::ptr::eq(s, o))),
                u8::from(upper.is_some_and(|s| std::ptr::eq(s, o)))
            );
            for r in &o.items {
                item(&mut sb, r, 0);
            }
        }
    }
    let p = save.join("z_outfits.bin");
    let data = std::fs::read(&p).map_err(|e| format!("{}: {e}", p.display()))?;
    let mut po = PersistentOutfits::default();
    po.init(&om, false, &mut |_| 0);
    po.load(&data)?;
    for (i, s) in po.seeds.iter().enumerate() {
        let _ = writeln!(sb, "PS {i} {s}");
    }
    let k = names.len();
    let _ = writeln!(sb, "PA {k}");
    for i in 0..k {
        let d = &po.all[i];
        let _ = writeln!(
            sb,
            "PD {i} {} {} {} {}",
            d.index,
            d.outfit_name,
            u8::from(d.use_seed),
            po.outfit_names[i]
        );
    }
    for (sex, l, n) in [
        ("F", &po.female, om.female_outfits.len()),
        ("M", &po.male, om.male_outfits.len()),
    ] {
        let _ = writeln!(sb, "PL {sex} {n}");
        let line: Vec<String> = l[..n]
            .iter()
            .map(|&i| po.all[i].index.to_string())
            .collect();
        let _ = writeln!(sb, "{}", line.join(" "));
    }
    let mut no_draw = |_: i32| -> i32 { panic!("getOutfit drew a random variant") };
    for i in [0i32, 1, k as i32 - 1, 32767] {
        for v in [1, 250, 500] {
            for bits in [0, FEMALE_BIT, NO_HAT_BIT] {
                let id = bits | (i << 16) | v;
                let _ = writeln!(sb, "GO {id} {}", po.get_outfit(id, &mut no_draw));
            }
        }
    }
    let _ = writeln!(sb, "GO 0 {}", po.get_outfit(0, &mut no_draw));
    Ok(sb)
}

/// The file guid table (`media/fileGuidTable.xml`) with its lookups in other letter case, then
/// `OutfitManager.getClothingItem` for every guid in table order (pairs with ClothingItemsProbe.java).
pub fn clothingitems(root: &Path) -> Result<String, String> {
    use zomboid_engine::zombie::core::skinnedmodel::population::outfit_manager::OutfitManager;
    use zomboid_engine::zombie::file_guid_table::FileGuidTable;
    let t = FileGuidTable::load_game(root)?;
    let mut om = OutfitManager::init(root)?.ok_or("no media/clothing/clothing.xml")?;
    let b = |v: bool| u8::from(v);
    let s = |v: &Option<String>| v.clone().unwrap_or_else(|| "null".to_string());
    let l = |v: &[String]| format!("[{}]", v.join(","));
    let mut sb = String::new();
    let _ = writeln!(sb, "FGT {}", t.files.len());
    for p in &t.files {
        let (path, guid) = (s(&p.path), s(&p.guid));
        let _ = writeln!(
            sb,
            "FG {path} {guid} {} {}",
            b(t.get_file_path_from_guid(&guid.to_uppercase()) == Some(path.as_str())),
            b(t.get_guid_from_file_path(&path.to_lowercase()) == Some(guid.as_str()))
        );
    }
    for (i, p) in t.files.iter().enumerate() {
        let Some(ci) = om.get_clothing_item(&s(&p.guid), &t, root) else {
            let _ = writeln!(sb, "CI {i} null");
            continue;
        };
        let masks: Vec<String> = ci.masks.iter().map(|m| m.to_string()).collect();
        let _ = writeln!(
            sb,
            "CI {i} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {}",
            s(&ci.guid),
            s(&ci.mame),
            b(ci.failed),
            s(&ci.male_model),
            s(&ci.female_model),
            s(&ci.alt_male_model),
            s(&ci.alt_female_model),
            b(ci.is_static),
            l(&ci.base_textures),
            s(&ci.attach_bone),
            l(&masks),
            s(&ci.masks_folder),
            s(&ci.underlay_masks_folder),
            l(&ci.texture_choices),
            b(ci.allow_random_hue),
            b(ci.allow_random_tint),
            s(&ci.decal_group),
            s(&ci.shader),
            s(&ci.hat_category),
            l(&ci.spawn_with),
            b(ci.is_hat()),
            b(ci.is_mask()),
            b(ci.has_model())
        );
    }
    Ok(sb)
}

/// Seeded draws that dress a character, in the format of `tests/original_probe/OutfitRandomProbe.java`.
pub fn outfitrandom(root: &Path) -> Result<String, String> {
    use save_dump::fh;
    use zomboid_engine::java::string_compare_to;
    use zomboid_engine::zombie::core::color::Color;
    use zomboid_engine::zombie::core::immutable_color::ImmutableColor;
    use zomboid_engine::zombie::core::skinnedmodel::population::clothing_decals::ClothingDecals;
    use zomboid_engine::zombie::core::skinnedmodel::population::clothing_item_reference::{
        ClothingItemReference, RandomizeContext,
    };
    use zomboid_engine::zombie::core::skinnedmodel::population::outfit::Outfit;
    use zomboid_engine::zombie::core::skinnedmodel::population::outfit_manager::OutfitManager;
    use zomboid_engine::zombie::core::skinnedmodel::population::outfit_rng::OutfitRng;
    use zomboid_engine::zombie::file_guid_table::FileGuidTable;
    use zomboid_engine::zombie::util::location_rng::LocationRng;
    let b = |v: bool| u8::from(v);
    let c = |c: &ImmutableColor| format!("{},{},{},{}", fh(c.r), fh(c.g), fh(c.b), fh(c.a));
    let s = |v: &Option<String>| v.clone().unwrap_or_else(|| "null".to_string());
    fn item(sb: &mut String, r: &ClothingItemReference, depth: usize) {
        let d = &r.random_data;
        let picked = match &d.picked_item_ref {
            Some(path) => r
                .picked(path)
                .and_then(|p| p.item_guid.as_deref())
                .unwrap_or("null"),
            None => "null",
        };
        let t = &d.tint;
        let _ = writeln!(
            sb,
            "R {depth} {} {} {} {} {},{},{},{} {} {picked} {} {}",
            r.item_guid.as_deref().unwrap_or("null"),
            u8::from(r.randomized),
            u8::from(d.active),
            fh(d.hue),
            fh(t.r),
            fh(t.g),
            fh(t.b),
            fh(t.a),
            d.base_texture.as_deref().unwrap_or("null"),
            d.texture_choice.as_deref().unwrap_or("null"),
            d.decal.as_deref().unwrap_or("null"),
        );
        for sub in &r.sub_items {
            item(sb, sub, depth + 1);
        }
    }
    let mut sb = String::new();
    let seeds: [i64; 7] = [
        0,
        1,
        -1,
        12345,
        i64::MIN,
        i64::MAX,
        0x9E37_79B9_7F4A_7C15_u64 as i64,
    ];
    let bounds: [i32; 9] = [1, 2, 7, 100, 1000, 65536, i32::MAX, 0, -5];
    let mut rng = LocationRng::default();
    for seed in seeds {
        rng.set_seed(seed);
        let _ = write!(sb, "LR {seed} {}", rng.get_seed());
        for _ in 0..4 {
            let _ = write!(sb, " {}", fh(rng.next_float()));
        }
        for n in bounds {
            let _ = write!(sb, " {}", rng.next_int(n));
        }
        sb.push('\n');
    }
    for [x, y, z] in [[0, 0, 0], [10500, 9800, 0], [-3, 7, 1], [300000, 200000, 7]] {
        let v = rng.next_int_at(1000, x, y, z);
        let _ = writeln!(sb, "LRS {x} {y} {z} {v} {}", rng.get_seed());
    }
    let hs: [f32; 12] = [
        -1.25,
        -0.5,
        -1.0e-9,
        0.0,
        0.1,
        0.166_666_67,
        0.25,
        0.5,
        0.75,
        0.9999,
        1.0,
        2.3,
    ];
    let ss: [f32; 4] = [0.0, 0.3, 1.0, 1.5];
    let vs: [f32; 4] = [0.0, 0.5, 1.0, -0.2];
    for h in hs {
        for sat in ss {
            for v in vs {
                let col = Color::hsb_to_rgb(h, sat, v);
                let _ = writeln!(
                    sb,
                    "HSB {} {} {} {},{},{},{}",
                    fh(h),
                    fh(sat),
                    fh(v),
                    fh(col.r),
                    fh(col.g),
                    fh(col.b),
                    fh(col.a)
                );
            }
        }
    }
    let mut orng = OutfitRng::default();
    for nbc in [false, true] {
        for seed in 0..20i64 {
            orng.set_seed(seed);
            let first = orng.random_immutable_color(nbc, false);
            let second = orng.random_immutable_color(nbc, true);
            let _ = writeln!(sb, "COL {} {seed} {} {}", b(nbc), c(&first), c(&second));
        }
    }
    let decals = ClothingDecals::init(root).ok_or("no media/clothing/clothingDecals.xml")?;
    let _ = writeln!(sb, "DECALS {}", decals.groups.len());
    for (gi, g) in decals.groups.iter().enumerate() {
        let name = s(&g.name);
        let same = decals
            .find_group(&name.to_uppercase())?
            .is_some_and(|f| std::ptr::eq(f, &decals.groups[gi]));
        let _ = write!(sb, "DG {name} {}", b(same));
        for seed in 0..6i64 {
            orng.set_seed(seed * 31 + 7);
            let _ = write!(sb, " {}", s(&decals.get_random_decal(&name, &mut orng)?));
        }
        sb.push('\n');
    }
    let _ = writeln!(
        sb,
        "DGX {} {}",
        s(&decals.get_random_decal("NoSuchGroup", &mut orng)?),
        s(&decals.get_random_decal(" ", &mut orng)?)
    );
    let table = FileGuidTable::load_game(root)?;
    let mut om = OutfitManager::init(root)?.ok_or("no media/clothing/clothing.xml")?;
    for nbc in [true, false] {
        for female in [true, false] {
            let mut l: Vec<Outfit> = if female {
                om.female_outfits.clone()
            } else {
                om.male_outfits.clone()
            };
            l.sort_by(|a, b| string_compare_to(&a.name, &b.name));
            for (i, base) in l.iter().enumerate() {
                let mut o = base.clone_outfit();
                orng.set_seed(
                    i as i64 * 7919 + if female { 1 } else { 2 } + if nbc { 0 } else { 1_000_003 },
                );
                let mut cx = RandomizeContext {
                    rng: &mut orng,
                    outfits: &mut om,
                    table: &table,
                    game_root: root,
                    decals: Some(&decals),
                    no_black_clothes: nbc,
                };
                for r in &mut o.items {
                    r.randomize(&mut cx)?;
                }
                let after = orng.next(1_000_000);
                let _ = writeln!(
                    sb,
                    "O {} {} {i} {} {} {after}",
                    b(nbc),
                    if female { "F" } else { "M" },
                    o.name,
                    o.items.len()
                );
                for r in &o.items {
                    item(&mut sb, r, 0);
                }
            }
        }
    }
    Ok(sb)
}

/// The clips of the human skeleton as one shared table.
fn animation_clips(
    fs: &zomboid_engine::zombie::zomboid_file_system::ZomboidFileSystem,
) -> Result<
    std::rc::Rc<
        zomboid_engine::zombie::core::skinnedmodel::animation::animation_clips::AnimationClips,
    >,
    String,
> {
    use zomboid_engine::zombie::core::skinnedmodel::animation::animation_clips::AnimationClips;
    let (clips, _dups) = AnimationClips::load(fs, "Human")?;
    Ok(std::rc::Rc::new(clips))
}

/// ActionContextProbe.dumpAnimator for the port: per layer (the root, then the sub layers in the animator's order) its
/// depth, state and live nodes, then the tracks of the animation player's multi track.
fn anim_dump(
    sb: &mut String,
    aa: &zomboid_engine::zombie::core::skinnedmodel::advancedanimation::advanced_animator::AdvancedAnimator,
) {
    use zomboid_engine::java::float_to_string as fl;
    let b = |v: bool| u8::from(v);
    let mut layers = vec![aa.root_layer().id];
    layers.extend(aa.sub_layer_ids());
    for id in layers {
        let Some(l) = aa.layer(id) else { continue };
        let _ = writeln!(
            sb,
            " L {} state={} nodes={}",
            aa.get_depth(id),
            aa.state_name_of_layer(id)
                .unwrap_or_else(|| "null".to_string()),
            l.get_live_anim_nodes().len()
        );
        for n in l.get_live_anim_nodes() {
            let _ = writeln!(
                sb,
                "  N {} anim={} w={} raw={} act={} in={} out={} tout={} new={} bf={} t={} tin={} tw={} from={} mt={} flags={}",
                aa.node_name(n),
                aa.node_anim_name(n),
                fl(n.get_weight()),
                fl(n.get_raw_weight()),
                b(n.is_active()),
                b(n.is_blending_in()),
                b(n.is_blending_out()),
                b(n.transitioning_out),
                b(n.is_new()),
                b(n.is_blend_field),
                fl(n.node_anim_time),
                b(n.is_transitioning_in()),
                fl(n.get_transition_in_weight()),
                n.get_transition_from().unwrap_or("null"),
                n.main_tracks().len(),
                n.while_alive_flag_count()
            );
        }
    }
    for (i, t) in aa.tracks().iter().enumerate() {
        let name = t
            .current_clip
            .as_ref()
            .map_or("!Empty!", |c| c.name.as_str());
        let _ = writeln!(
            sb,
            " R {i} {name} time={} speed={} bw={} fw={} play={} loop={} prim={} prio={}",
            fl(t.get_current_time_value()),
            fl(t.get_speed_delta()),
            fl(t.get_blend_weight()),
            fl(t.get_blend_field_weight()),
            b(t.is_playing),
            b(t.is_looping()),
            b(t.is_primary),
            t.priority
        );
    }
}

/// The OPTS line of ActionContextProbe: the debug options the animator reads, as the port assumes them (a build with
/// `Core.debug` false: every one at its default).
fn anim_options_line(sb: &mut String) {
    let _ = writeln!(
        sb,
        "OPTS debug=false earlyOut=1 noBlends=0 physicsHit=1 logStates=0 override=0"
    );
}

/// ActionContextProbe.animScenario "anim:<set>:<seed>:<ticks>": the animator of the set on the scripted character.
fn anim_scenario(
    sb: &mut String,
    fs: &zomboid_engine::zombie::zomboid_file_system::ZomboidFileSystem,
    clips: &std::rc::Rc<
        zomboid_engine::zombie::core::skinnedmodel::animation::animation_clips::AnimationClips,
    >,
    spec: &str,
) -> Result<(), String> {
    use std::collections::{BTreeMap, BTreeSet};
    use zomboid_engine::java::{self, float_to_string as fl, Random};
    use zomboid_engine::zombie::core::random::rand_abstract::RandAbstract;
    use zomboid_engine::zombie::core::skinnedmodel::advancedanimation::advanced_animator::{
        AdvancedAnimator, AnimatorOwner, Env,
    };
    use zomboid_engine::zombie::core::skinnedmodel::advancedanimation::anim_condition::Type;
    use zomboid_engine::zombie::core::skinnedmodel::advancedanimation::anim_event::AnimEvent;
    use zomboid_engine::zombie::core::skinnedmodel::advancedanimation::anim_node::float_compare;
    use zomboid_engine::zombie::core::skinnedmodel::advancedanimation::animation_set::AnimationSets;
    use zomboid_engine::zombie::core::skinnedmodel::advancedanimation::animation_variable_reference::AnimationVariableReference;
    use zomboid_engine::zombie::core::skinnedmodel::advancedanimation::animation_variable_source::{
        AnimationVariableHandlePool, AnimationVariableSlot, AnimationVariableSource,
    };
    use zomboid_engine::zombie::core::skinnedmodel::advancedanimation::i_animation_variable_source::IAnimationVariableSource;
    use zomboid_engine::zombie::core::skinnedmodel::animation::animation_track::AnimationTrack;

    /// The scripted character: its variables, and the log of the events the animator raised.
    struct AnimScript {
        vars: AnimationVariableSource,
        pool: AnimationVariableHandlePool,
        events: String,
    }
    impl IAnimationVariableSource for AnimScript {
        fn get_variable(&self, key: &str) -> Option<AnimationVariableSlot> {
            self.vars.get_variable(key).cloned()
        }
    }
    impl AnimatorOwner for AnimScript {
        fn is_player_ready(&self) -> bool {
            true
        }
        fn on_anim_event(
            &mut self,
            layer_depth: i32,
            layer_state_name: Option<&str>,
            track: Option<&AnimationTrack>,
            event: &AnimEvent,
            _rand: &mut RandAbstract<Random>,
        ) -> Result<(), String> {
            let track_name = track.map_or("null", |t| {
                t.current_clip
                    .as_ref()
                    .map_or("!Empty!", |c| c.name.as_str())
            });
            let _ = writeln!(
                self.events,
                " EV {} {} {} {} {}",
                layer_depth,
                layer_state_name.unwrap_or("null"),
                event.event_name.as_deref().unwrap_or("null"),
                event.parameter_value.as_deref().unwrap_or("null"),
                track_name
            );
            Ok(())
        }
        fn set_variable_bool(
            &mut self,
            reference: &AnimationVariableReference,
            value: bool,
        ) -> Result<(), String> {
            // AnimationVariableReference.setVariable(owner, boolean): a blank name or a sub variable source (the
            // character has none: read-only) does nothing; else the character's `setVariable(name, value)`
            if reference.name.chars().all(java::is_whitespace)
                || reference.sub_variable_source_name.is_some()
            {
                return Ok(());
            }
            self.vars
                .set_variable_bool(&mut self.pool, &reference.name, value);
            Ok(())
        }
        fn set_variable_float(&mut self, key: &str, value: f32) -> Result<(), String> {
            if let Some(s) = self.vars.get_variable_mut(key) {
                s.set_float(value);
            }
            Ok(())
        }
    }
    enum Opt {
        Bool(bool),
        Float(f32),
        Str(String),
    }

    let p: Vec<&str> = spec.split(':').collect();
    if p.len() != 4 && p.len() != 5 {
        return Err(format!("bad scenario {spec}"));
    }
    let set_name = p[1];
    let seed: i64 = p[2].parse().map_err(|e| format!("{spec}: {e}"))?;
    let ticks: usize = p[3].parse().map_err(|e| format!("{spec}: {e}"))?;
    let mut sets = AnimationSets::default();
    let set = sets.get_animation_set_rc(fs, set_name, false)?;
    // the domain: the variables the conditions of every node and transition read, the states that need no ragdoll
    // or node blend curve
    let mut strs: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut nums: BTreeMap<String, Vec<f32>> = BTreeMap::new();
    let mut names: BTreeSet<String> = BTreeSet::new();
    let mut state_keys: BTreeSet<String> = BTreeSet::new();
    let mut keys: Vec<&String> = set.states.keys().collect();
    keys.sort();
    for key in &keys {
        let st = &set.states[*key];
        let mut ok = true;
        for n in &st.nodes {
            if n.is_ragdoll || n.blend_curve.is_some() {
                ok = false;
            }
            let mut conds: Vec<&_> = n.conditions.iter().collect();
            for t in &n.transitions {
                conds.extend(t.conditions.iter());
            }
            for c in conds {
                let Some(ty) = c.type_ else { continue };
                if ty == Type::Or {
                    continue;
                }
                let r = AnimationVariableReference::from_raw_variable_name(&c.name);
                if r.sub_variable_source_name.is_some() || r.name.chars().all(java::is_whitespace) {
                    continue;
                }
                names.insert(c.name.clone());
                match ty {
                    Type::String | Type::Strneq => {
                        strs.entry(c.name.clone())
                            .or_default()
                            .insert(c.string_value.clone());
                    }
                    Type::Bool => {}
                    _ => {
                        let v = nums.entry(c.name.clone()).or_default();
                        if !v.iter().any(|x| float_compare(*x, c.float_value).is_eq()) {
                            v.push(c.float_value);
                        }
                    }
                }
            }
        }
        if ok {
            state_keys.insert((*key).clone());
        }
    }
    let mut vars: Vec<(String, Vec<Opt>)> = Vec::new();
    for name in &names {
        let options = if let Some(o) = strs.get(name) {
            let mut o = o.clone();
            o.insert(String::new());
            o.insert("x".to_string());
            o.into_iter().map(Opt::Str).collect()
        } else if let Some(o) = nums.get(name) {
            let mut o = o.clone();
            for f in o.clone() {
                let g = f + 1.0;
                if !o.iter().any(|x| float_compare(*x, g).is_eq()) {
                    o.push(g);
                }
            }
            for g in [0.0f32, 1.0] {
                if !o.iter().any(|x| float_compare(*x, g).is_eq()) {
                    o.push(g);
                }
            }
            o.sort_by(|a, b| float_compare(*a, *b));
            o.into_iter().map(Opt::Float).collect()
        } else {
            vec![Opt::Bool(true), Opt::Bool(false)]
        };
        vars.push((name.clone(), options));
    }
    let mut state_list: Vec<String> = state_keys.into_iter().collect();
    if let Some(only) = p.get(4) {
        // "...:state,state": only these states (lower case, as the set keys them) are chosen
        let only: Vec<&str> = only.split(',').collect();
        state_list.retain(|k| only.contains(&k.as_str()));
    }
    let _ = writeln!(sb, "SCENARIO {spec}");
    let _ = writeln!(
        sb,
        "SET {set_name} states={} usable={} vars={}",
        keys.len(),
        state_list.len(),
        vars.len()
    );
    anim_options_line(sb);
    let mut animator = AdvancedAnimator::init(Some(set.clone()), clips.clone());
    let mut owner = AnimScript {
        vars: AnimationVariableSource::default(),
        pool: AnimationVariableHandlePool::default(),
        events: String::new(),
    };
    if seed % 2 == 0 {
        owner
            .vars
            .set_variable_float(&mut owner.pool, "maxTwist", 0.0);
    }
    let mut rand = RandAbstract::new(Random::new(seed));
    let mut r = Random::new(seed);
    let dts: [f32; 8] = [
        1.0 / 60.0,
        1.0 / 30.0,
        0.083333336,
        0.05,
        0.016666668,
        0.2,
        0.5,
        0.033333335,
    ];
    let nv = vars.len() as i32;
    let ns = state_list.len() as i32;
    let mut prev = 0;
    for t in 1..=ticks {
        let changes = 1 + r.next_int_bound(3);
        for _ in 0..changes {
            if nv <= 0 {
                break;
            }
            let (name, options) = &vars[r.next_int_bound(nv) as usize];
            let o = &options[r.next_int_bound(options.len() as i32) as usize];
            match o {
                Opt::Bool(b) => owner.vars.set_variable_bool(&mut owner.pool, name, *b),
                Opt::Float(f) => owner.vars.set_variable_float(&mut owner.pool, name, *f),
                Opt::Str(s) => owner.vars.set_variable_string(&mut owner.pool, name, s),
            }
        }
        if nv > 0 && r.next_int_bound(16) == 0 {
            let name = vars[r.next_int_bound(nv) as usize].0.clone();
            owner.vars.clear_variable(&name);
        }
        let mut did = "-".to_string();
        if ns > 0 && r.next_int_bound(5) == 0 {
            let root = if r.next_int_bound(25) == 0 {
                "bogus".to_string()
            } else {
                state_list[r.next_int_bound(ns) as usize].clone()
            };
            let nsub = r.next_int_bound(3);
            let mut subs: Vec<String> = Vec::new();
            for _ in 0..nsub {
                subs.push(if r.next_int_bound(25) == 0 {
                    "bogus".to_string()
                } else {
                    state_list[r.next_int_bound(ns) as usize].clone()
                });
            }
            animator.set_state_names(&root, &subs)?;
            did = format!("{root}[{}]", subs.join(", "));
        }
        if r.next_int_bound(97) == 0 {
            let mut env = Env {
                owner: &mut owner,
                rand: &mut rand,
            };
            animator.reset(&mut env)?;
            did = "reset".to_string();
        }
        let dt = dts[r.next_int_bound(dts.len() as i32) as usize];
        owner.events.clear();
        {
            let mut env = Env {
                owner: &mut owner,
                rand: &mut rand,
            };
            animator.update(dt, &mut env)?;
            animator.update_animation_player(dt, &mut env)?;
        }
        let draws = rand.id();
        let _ = write!(
            sb,
            "T {t} dt={} draws={} set={did} state={}",
            fl(dt),
            draws - prev,
            animator
                .get_current_state_name()
                .unwrap_or_else(|| "null".to_string())
        );
        if let Some(s) = owner.vars.get_variable("maxTwist") {
            let _ = write!(sb, " maxTwist={}", fl(s.get_value_float()));
        }
        sb.push('\n');
        prev = draws;
        sb.push_str(&owner.events);
        anim_dump(sb, &animator);
    }
    let _ = writeln!(sb, "END {spec} draws={}", rand.id());
    Ok(())
}

/// ActionContextProbe.zombieAnimScenario "zanim:<seed>:<ticks>": the ZombieProbe zombie of the seed ticked alone on
/// the square (8280, 11670, 0); per tick the animation time delta, the draws, the action context and the animator.
fn zombie_anim_scenario(sb: &mut String, root: &Path, spec: &str) -> Result<(), String> {
    use zomboid_engine::java::{float_to_string as fl, Random};
    use zomboid_engine::zombie::characters::iso_zombie::{IsoZombie, Lore, ZombieWorld};
    use zomboid_engine::zombie::core::skinnedmodel::population::outfit_rng::OutfitRng;

    let p: Vec<&str> = spec.split(':').collect();
    if p.len() != 3 {
        return Err(format!("bad scenario {spec}"));
    }
    let seed: i64 = p[1].parse().map_err(|e| format!("{spec}: {e}"))?;
    let ticks: usize = p[2].parse().map_err(|e| format!("{spec}: {e}"))?;
    let survivors = zombie_survivors(root)?;
    let sandbox = zombie_sandbox(None)?;
    let lore = Lore::from_sandbox(&sandbox)?;
    let assets = zombie_anim_assets(root, false)?;
    let mut outfit_rng = OutfitRng::default();
    outfit_rng.set_seed(seed);
    let group_name = assets.action_group.name.clone();
    let set_name = assets.anim_set.name.clone();
    let mut world = ZombieWorld {
        rand: RandAbstract::new(Random::new(seed)),
        outfit_rng,
        lore,
        survivors,
        survivor_id_count: 0,
        multiplier: 0.8,
        raining: false,
        anim_assets: Some(assets),
        sp: None,
    };
    let _ = writeln!(sb, "SCENARIO {spec}");
    anim_options_line(sb);
    let mut z = IsoZombie::new(&mut world)?;
    let _ = writeln!(
        sb,
        "ZOMBIE draws={} group={group_name} animSet={set_name}",
        world.rand.id()
    );
    let chunks = zombie_chunks(root)?;
    let grid = ZombieGrid::new(&chunks);
    z.place(8280.5, 11670.5, 0.0);
    let mut prev = world.rand.id();
    for n in 1..=ticks {
        z.tick(&mut world, &grid)?;
        let draws = world.rand.id();
        let anim = z.anim.as_ref().ok_or("the zombie has no animator")?;
        let _ = writeln!(
            sb,
            "T {n} dt={} upd={} draws={} ctx={} child=[{}] ai={} vars={}",
            fl(world.time_delta() * anim.animation_time_scale),
            u8::from(anim.animation_updating_this_frame),
            draws - prev,
            anim.action_context.get_current_state_name(),
            anim.action_context.get_child_state_names().join(","),
            z.current_state.map_or("null", |s| s.name()),
            anim.game_variables.get_game_variables().count()
        );
        anim_dump(sb, &anim.animator);
        prev = draws;
    }
    let _ = writeln!(sb, "END {spec} draws={}", world.rand.id());
    Ok(())
}

/// The `actioncontext` case: `ActionContextProbe`'s scenarios, run by the port. `ctx:<group>:<seed>:<ticks>` is the
/// ActionContext of an action group with a scripted owner (see the probe for the script).
pub fn actioncontext(root: &Path, scenarios: &[String]) -> Result<String, String> {
    use std::collections::{BTreeMap, BTreeSet};
    use zomboid_engine::java::Random;
    use zomboid_engine::zombie::characters::action::action_context::ActionContext;
    use zomboid_engine::zombie::characters::action::action_group::ActionGroups;
    use zomboid_engine::zombie::characters::action::conditions::character_variable_condition::Value;
    use zomboid_engine::zombie::characters::action::i_action_condition::IActionCondition;
    use zomboid_engine::zombie::core::skinnedmodel::advancedanimation::anim_node::float_compare;
    use zomboid_engine::zombie::core::skinnedmodel::advancedanimation::animation_variable_source::{
        AnimationVariableHandlePool, AnimationVariableSlot, AnimationVariableSource,
    };
    use zomboid_engine::zombie::core::skinnedmodel::advancedanimation::i_animation_variable_source::IAnimationVariableSource;
    use zomboid_engine::zombie::zomboid_file_system::ZomboidFileSystem;

    struct Scripted {
        vars: AnimationVariableSource,
    }
    impl IAnimationVariableSource for Scripted {
        fn get_variable(&self, key: &str) -> Option<AnimationVariableSlot> {
            self.vars.get_variable(key).cloned()
        }
    }
    enum Opt {
        Bool(bool),
        Float(f32),
        Str(String),
    }
    struct Var {
        name: String,
        options: Vec<Opt>,
    }

    let fs = ZomboidFileSystem::init(root)?;
    let mut groups = ActionGroups::default();
    let mut sb = String::new();
    for spec in scenarios {
        if spec == "clips" {
            use zomboid_engine::zombie::core::skinnedmodel::animation::animation_clips::AnimationClips;
            let (clips, _dups) = AnimationClips::load(&fs, "Human")?;
            let mut names: Vec<&String> = clips.clips.keys().collect();
            names.sort();
            let _ = writeln!(sb, "SCENARIO clips");
            let _ = writeln!(sb, "READY true");
            let _ = writeln!(sb, "CLIPS {}", names.len());
            for n in names {
                let c = &clips.clips[n];
                let _ = writeln!(
                    sb,
                    "CLIP {} {} {}",
                    n,
                    c.name,
                    zomboid_engine::java::float_to_string(c.duration)
                );
            }
            let _ = writeln!(sb, "END clips");
            continue;
        }
        if spec.starts_with("anim:") {
            let clips = animation_clips(&fs)?;
            anim_scenario(&mut sb, &fs, &clips, spec)?;
            continue;
        }
        if spec.starts_with("zanim:") {
            zombie_anim_scenario(&mut sb, root, spec)?;
            continue;
        }
        let p: Vec<&str> = spec.split(':').collect();
        if p.len() != 4 || p[0] != "ctx" {
            return Err(format!("unknown scenario {spec}"));
        }
        let group_name = p[1];
        let seed: i64 = p[2].parse().map_err(|e| format!("{spec}: {e}"))?;
        let ticks: usize = p[3].parse().map_err(|e| format!("{spec}: {e}"))?;
        let group = groups.get_action_group_rc(&fs, group_name);
        // the domain, from the group's conditions
        let mut state_names: Vec<String> = Vec::new();
        let mut events: BTreeSet<String> = BTreeSet::new();
        let mut names: BTreeSet<String> = BTreeSet::new();
        let mut strs: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        let mut nums: BTreeMap<String, Vec<f32>> = BTreeMap::new();
        let mut bools: BTreeSet<String> = BTreeSet::new();
        for st in &group.states {
            state_names.push(st.name.clone());
            for t in &st.transitions {
                for c in &t.conditions {
                    match c {
                        IActionCondition::EventOccurred(e)
                        | IActionCondition::EventNotOccurred(e) => {
                            events.insert(e.clone());
                        }
                        IActionCondition::CharacterVariable(c) => {
                            let Some(Value::Lookup(r)) = &c.lhs_value else {
                                continue;
                            };
                            if r.sub_variable_source_name.is_some() {
                                continue;
                            }
                            let name = r.name.clone();
                            names.insert(name.clone());
                            match &c.rhs_value {
                                Some(Value::Boolean(_)) => {
                                    bools.insert(name);
                                }
                                Some(Value::Integer(i)) => {
                                    let v = nums.entry(name).or_default();
                                    let f = *i as f32;
                                    if !v.iter().any(|x| float_compare(*x, f).is_eq()) {
                                        v.push(f);
                                    }
                                }
                                Some(Value::Float(f)) => {
                                    let v = nums.entry(name).or_default();
                                    if !v.iter().any(|x| float_compare(*x, *f).is_eq()) {
                                        v.push(*f);
                                    }
                                }
                                Some(Value::String(s)) => {
                                    strs.entry(name).or_default().insert(s.clone());
                                }
                                _ => {}
                            }
                        }
                        IActionCondition::LuaCall => {}
                    }
                }
            }
        }
        let mut vars: Vec<Var> = Vec::new();
        let mut kinds: Vec<(String, &str)> = Vec::new();
        for name in &names {
            let (kind, options) = if let Some(o) = strs.get(name) {
                let mut o = o.clone();
                o.insert(String::new());
                o.insert("x".to_string());
                ("string", o.into_iter().map(Opt::Str).collect())
            } else if let Some(o) = nums.get(name) {
                let mut o = o.clone();
                for f in o.clone() {
                    let g = f + 1.0;
                    if !o.iter().any(|x| float_compare(*x, g).is_eq()) {
                        o.push(g);
                    }
                }
                for g in [0.0f32, 1.0] {
                    if !o.iter().any(|x| float_compare(*x, g).is_eq()) {
                        o.push(g);
                    }
                }
                o.sort_by(|a, b| float_compare(*a, *b));
                ("float", o.into_iter().map(Opt::Float).collect())
            } else {
                let _ = bools.contains(name);
                ("bool", vec![Opt::Bool(true), Opt::Bool(false)])
            };
            kinds.push((name.clone(), kind));
            vars.push(Var {
                name: name.clone(),
                options,
            });
        }
        let _ = writeln!(sb, "SCENARIO {spec}");
        let _ = writeln!(
            sb,
            "GROUP {group_name} states={} vars={} events={}",
            state_names.len(),
            vars.len(),
            events.len()
        );
        let _ = write!(sb, "VARS");
        for (v, (_, kind)) in vars.iter().zip(&kinds) {
            let _ = write!(sb, " {}:{}/{}", v.name, kind, v.options.len());
        }
        sb.push('\n');
        let _ = writeln!(
            sb,
            "EVENTS [{}]",
            events.iter().cloned().collect::<Vec<_>>().join(", ")
        );
        let mut pool = AnimationVariableHandlePool::default();
        let mut owner = Scripted {
            vars: AnimationVariableSource::default(),
        };
        let mut ctx = ActionContext::new();
        ctx.set_group(Rc::clone(&group));
        let _ = writeln!(
            sb,
            "INIT root={} prev={}",
            ctx.get_current_state_name(),
            ctx.peek_previous_state_name()
        );
        let mut r = Random::new(seed);
        let nv = vars.len() as i32;
        let ne = events.len() as i32;
        let events: Vec<String> = events.into_iter().collect();
        for t in 1..=ticks {
            let changes = 1 + r.next_int_bound(2);
            for _ in 0..changes {
                if nv <= 0 {
                    break;
                }
                let v = &vars[r.next_int_bound(nv) as usize];
                let o = &v.options[r.next_int_bound(v.options.len() as i32) as usize];
                match o {
                    Opt::Bool(b) => owner.vars.set_variable_bool(&mut pool, &v.name, *b),
                    Opt::Float(f) => owner.vars.set_variable_float(&mut pool, &v.name, *f),
                    Opt::Str(s) => owner.vars.set_variable_string(&mut pool, &v.name, s),
                }
            }
            if ne > 0 && r.next_int_bound(4) == 0 {
                let e = &events[r.next_int_bound(ne) as usize];
                if r.next_int_bound(3) == 0 {
                    let cur = ctx.get_current_state_name();
                    ctx.report_event_for_state(Some(&cur), e);
                } else {
                    ctx.report_event(e);
                }
            }
            if r.next_int_bound(64) == 0 {
                let forced = &state_names[r.next_int_bound(state_names.len() as i32) as usize];
                let sr = ctx.find_state_ref(forced);
                ctx.set_current_state(sr);
            }
            let can = &state_names[r.next_int_bound(state_names.len() as i32) as usize];
            let changed = u8::from(ctx.update(&owner));
            let next = ctx.peek_next_state(&owner).map(|s| s.name.clone());
            let _ = writeln!(
                sb,
                "T {t} root={} child=[{}] prev={} next={} changed={changed} can({can})={}{} ev={}",
                ctx.get_current_state_name(),
                ctx.get_child_state_names().join(","),
                ctx.peek_previous_state_name(),
                next.as_deref().unwrap_or("-"),
                u8::from(ctx.can_transition_to_state(&owner, can, true)),
                u8::from(ctx.can_transition_to_state(&owner, can, false)),
                if ne > 0 {
                    u8::from(ctx.has_event_occurred(&events[t % ne as usize]))
                } else {
                    0
                }
            );
        }
        let _ = writeln!(sb, "END {spec}");
    }
    Ok(sb)
}

/// `ZombieAiProbe.squareMatrices`: the matrices the chunk loading left on every square of the four chunks.
fn zombie_square_matrices(sb: &mut String, chunks: &[IsoChunk]) {
    let _ = writeln!(sb, "SCENARIO zsq");
    for ch in chunks {
        let _ = writeln!(
            sb,
            "CHUNK {} {} levels={}..{}",
            ch.wx,
            ch.wy,
            ch.get_min_level(),
            ch.get_max_level()
        );
        for z in ch.get_min_level()..=ch.get_max_level() {
            for y in 0..8 {
                for x in 0..8 {
                    if let Some(sq) = ch.get_grid_square(x, y, z) {
                        let _ = writeln!(
                            sb,
                            "SQ {},{},{} c={:x} p={:x} v={:x}",
                            sq.x, sq.y, sq.z, sq.collide_matrix, sq.path_matrix, sq.vision_matrix
                        );
                    }
                }
            }
        }
    }
    let _ = writeln!(sb, "END zsq");
}

/// The `zombieai` case: `ZombieAiProbe`'s scenarios `zai:<seed>:<ticks>:<dx>:<dy>:<force|none>`, run by the port.
pub fn zombieai(root: &Path, scenarios: &[String]) -> Result<String, String> {
    let mut sb = String::new();
    // the files are read once; every scenario gets its own copies of the mutable parts
    let survivors = zombie_survivors(root)?;
    let assets = zombie_anim_assets(root, true)?;
    let (chunks, alias, sprites) = zombie_chunks_alias(root)?;
    let alias = std::rc::Rc::new(alias);
    let sprites = std::rc::Rc::new(sprites);
    // the item scripts, only when a scenario hits the zombie with a weapon of them
    let scripts = if scenarios.iter().any(|s| s.contains(":hit")) {
        Some(zomboid_engine::zombie::scripting::script_manager::ScriptManager::load(root)?)
    } else {
        None
    };
    for spec in scenarios {
        if spec == "zsq" {
            zombie_square_matrices(&mut sb, &chunks);
        } else {
            zombie_ai_scenario(
                &mut sb,
                spec,
                &survivors,
                &assets,
                &chunks,
                &alias,
                &sprites,
                scripts.as_ref(),
            )?;
        }
    }
    Ok(sb)
}

/// ZombieAiProbe.scenario: the zombie of a seed on the square (8280, 11670, 0) and a player stand-in (8280.5 + dx,
/// 11670.5 + dy) in the single-player world, ticked as the world ticks its objects (`tick_sp`). `force` sets the
/// zombie's target to the player (and `timeSinceSeenFlesh` to 0) before every tick.
#[allow(clippy::too_many_arguments)]
fn zombie_ai_scenario(
    sb: &mut String,
    spec: &str,
    survivors: &zomboid_engine::zombie::characters::survivor_factory::SurvivorFactory,
    assets: &zomboid_engine::zombie::characters::iso_zombie::ZombieAnimAssets,
    chunks: &[IsoChunk],
    alias: &std::rc::Rc<
        zomboid_engine::zombie::core::tile_property_alias_map::TilePropertyAliasMap,
    >,
    sprites: &std::rc::Rc<
        zomboid_engine::zombie::iso::sprite::iso_sprite_manager::IsoSpriteManager,
    >,
    scripts: Option<&zomboid_engine::zombie::scripting::script_manager::ScriptManager>,
) -> Result<(), String> {
    use zomboid_engine::java::{float_to_string as fl, Random};
    use zomboid_engine::zombie::characters::iso_zombie::{IsoZombie, Lore, ZombieWorld};
    use zomboid_engine::zombie::characters::iso_zombie_hit::HitWeapon;
    use zomboid_engine::zombie::characters::iso_zombie_sp::{
        PlayerStandIn, SinglePlayerWorld, SpRead,
    };
    use zomboid_engine::zombie::core::skinnedmodel::population::outfit_rng::OutfitRng;

    const VARS: [&str; 33] = [
        "bLunge",
        "bMoving",
        "bPathfind",
        "bHasTarget",
        "bCanSeeTarget",
        "bAttack",
        "isFacingTarget",
        "alerted",
        "bThump",
        "bKnockedDown",
        "bOnFloor",
        "bStaggerBack",
        "hashitreaction",
        "AttackType",
        "AttackOutcome",
        "AttackDidDamage",
        "ZombieBiteDone",
        "angle",
        "animAngle",
        "twist",
        "targetTwist",
        "shoulderTwist",
        "excessTwist",
        "angleStepDelta",
        "angleTwistDelta",
        "isTurning",
        "isTurning90",
        "isTurningAround",
        "previousState",
        "bStopLunging",
        "lungeTimer",
        "distanceToTarget",
        "stateEventDelayTimer",
    ];
    const RAG: [&str; 7] = [
        "canRagdoll",
        "isUpright",
        "isOnBack",
        "anyGrappleAnim",
        "grappleAnim",
        "numTwistBones",
        "useRagdollVehicleCollision",
    ];
    let p: Vec<&str> = spec.split(':').collect();
    if p.len() != 6 || p[0] != "zai" {
        return Err(format!("bad scenario {spec}"));
    }
    let seed: i64 = p[1].parse().map_err(|e| format!("{spec}: {e}"))?;
    let ticks: usize = p[2].parse().map_err(|e| format!("{spec}: {e}"))?;
    let dx: f32 = p[3].parse().map_err(|e| format!("{spec}: {e}"))?;
    let dy: f32 = p[4].parse().map_err(|e| format!("{spec}: {e}"))?;
    // `force`, or `hit<N>[,<M>...][@<weapon>[@<damageSplit>[@<flags>]]]`: like force, and at those ticks the player
    // stand-in hits the zombie (default weapon Base.BaseballBat, split 1.0; flag `c` = critical hit)
    let hit_spec: Vec<&str> = p[5].split('@').collect();
    let hit_ticks: Vec<usize> = if hit_spec[0].starts_with("hit") {
        hit_spec[0][3..]
            .split(',')
            .map(|t| t.parse().map_err(|e| format!("{spec}: {e}")))
            .collect::<Result<_, _>>()?
    } else {
        Vec::new()
    };
    let force = p[5] == "force" || !hit_ticks.is_empty();
    let hit_weapon_name = hit_spec.get(1).copied().unwrap_or("Base.BaseballBat");
    let hit_split: f32 = match hit_spec.get(2) {
        Some(t) => zomboid_engine::java::parse_float(t).ok_or_else(|| format!("{spec}: split"))?,
        None => 1.0,
    };
    let hit_critical = hit_spec.get(3).is_some_and(|f| f.contains('c'));
    let hit_weapon = if hit_ticks.is_empty() {
        None
    } else {
        let item = scripts
            .ok_or("zombieai: the item scripts are not loaded")?
            .items
            .iter()
            .find(|i| i.full_name() == hit_weapon_name)
            .ok_or_else(|| format!("{spec}: no item {hit_weapon_name}"))?;
        Some(HitWeapon::from_item(item)?)
    };
    let sandbox = zombie_sandbox(None)?;
    let lore = Lore::from_sandbox(&sandbox)?;
    let grid = ZombieGrid::new(chunks);
    let spw = SinglePlayerWorld {
        player: PlayerStandIn {
            x: 8280.5f32 + dx,
            y: 11670.5f32 + dy,
            z: 0.0,
            width: 0.24,
            health: 1.0,
            critical_hit: false,
        },
        loaded_chunks: vec![(1034, 1458), (1034, 1459), (1035, 1458), (1035, 1459)],
        alias: alias.clone(),
        sprites: sprites.clone(),
        zombie_object_id: 1,
        game_sounds: scripts.map(|m| std::rc::Rc::new(m.game_sounds.clone())),
    };
    let mut outfit_rng = OutfitRng::default();
    outfit_rng.set_seed(seed);
    let mut world = ZombieWorld {
        rand: RandAbstract::new(Random::new(seed)),
        outfit_rng,
        lore,
        survivors: survivors.clone(),
        survivor_id_count: 0,
        multiplier: 0.8,
        raining: false,
        anim_assets: Some(assets.clone()),
        sp: Some(spw.clone()),
    };
    let _ = writeln!(sb, "SCENARIO {spec}");
    anim_options_line(sb);
    let mut z = IsoZombie::new(&mut world)?;
    let rd = SpRead::new(&grid, &spw);
    let _ = writeln!(
        sb,
        "ZOMBIE draws={} vars={} id={} zombieId={}",
        world.rand.id(),
        z.sp_variable_count(),
        spw.zombie_object_id,
        z.zombie_id
    );
    for l in z.sp_variable_dump(&rd) {
        let _ = writeln!(sb, "{l}");
    }
    rd.check()?;
    z.place(8280.5, 11670.5, 0.0);
    // `z.setCurrent(cell.getGridSquare(8280, 11670, 0))`
    z.sp.current = Some((8280, 11670, 0));
    let mut prev = world.rand.id();
    for n in 1..=ticks {
        if force {
            z.sp.target = true;
            z.time_since_seen_flesh = 0.0;
        }
        if let (true, Some(weapon)) = (hit_ticks.contains(&n), hit_weapon.as_ref()) {
            weapon.create_item_draws(&mut world);
            if let Some(sp) = world.sp.as_mut() {
                sp.player.critical_hit = hit_critical;
            }
            let damage = z.hit_sp(&mut world, &grid, weapon, hit_split)?;
            let _ = writeln!(
                sb,
                "HIT damage={} hp={} dead={} knockedDown={} staggerBack={} hitReaction={}",
                fl(damage),
                fl(z.character.health),
                z.is_dead(),
                z.sp.knocked_down,
                z.sp.stagger_back,
                z.sp.hit_reaction
            );
        }
        z.tick_sp(&mut world, &grid)?;
        let draws = world.rand.id();
        let anim = z.anim.as_ref().ok_or("the zombie has no animator")?;
        let mut vars = String::new();
        for name in VARS {
            let slot = anim
                .action_context
                .get_variable(name)
                .or_else(|| z.sp_get_variable(name, &rd));
            vars.push_str(name);
            vars.push('=');
            match slot {
                None => vars.push('-'),
                Some(s) => vars.push_str(&s.get_value_string().unwrap_or_else(|| "null".into())),
            }
            vars.push(' ');
        }
        let made = z.sp.anim_player.is_some();
        for name in RAG {
            let text = if !made {
                "?".to_string()
            } else {
                match anim
                    .action_context
                    .get_variable(name)
                    .or_else(|| z.sp_get_variable(name, &rd))
                {
                    None => "-".to_string(),
                    // numTwistBones is an Int slot
                    Some(s) if name == "numTwistBones" => format!("{}", s.get_value_float() as i32),
                    Some(s) => s.get_value_string().unwrap_or_else(|| "null".into()),
                }
            };
            vars.push_str(&format!("{name}={text} "));
        }
        rd.check()?;
        let _ = writeln!(
            sb,
            "T {n} draws={} ctx={} child=[{}] ai={} x={} y={} dir={} fwd={},{} look={} hp={} target={} php={} goal={} {}",
            draws - prev,
            anim.action_context.get_current_state_name(),
            anim.action_context.get_child_state_names().join(","),
            z.current_state.map_or("null", |s| s.name()),
            fl(z.character.moving.x),
            fl(z.character.moving.y),
            z.character.forward_iso_direction().name(),
            fl(z.character.forward_direction.x),
            fl(z.character.forward_direction.y),
            fl(z.look_angle_radians()),
            fl(z.character.health),
            if z.sp.target { "player" } else { "-" },
            fl(spw.player.health),
            z.path.goal.name(),
            vars.trim_end()
        );
        prev = draws;
    }
    let _ = writeln!(sb, "END {spec} draws={}", world.rand.id());
    Ok(())
}

/// Every animation set of `media/AnimSets` and every action group of `media/actiongroups`, in the format of
/// `tests/original_probe/AnimSetsProbe.java`.
pub fn animsets(root: &Path) -> Result<String, String> {
    use zomboid_engine::java::{float_to_string, string_compare_to};
    use zomboid_engine::zombie::characters::action::action_group::ActionGroups;
    use zomboid_engine::zombie::characters::action::conditions::character_variable_condition::Value;
    use zomboid_engine::zombie::characters::action::i_action_condition::IActionCondition;
    use zomboid_engine::zombie::core::math::i_interpolator::IInterpolator;
    use zomboid_engine::zombie::core::skinnedmodel::advancedanimation::anim_condition::AnimCondition;
    use zomboid_engine::zombie::core::skinnedmodel::advancedanimation::anim_event::AnimEventKind;
    use zomboid_engine::zombie::core::skinnedmodel::advancedanimation::anim_node::AnimNode;
    use zomboid_engine::zombie::core::skinnedmodel::advancedanimation::animation_set::AnimationSets;
    use zomboid_engine::zombie::core::skinnedmodel::advancedanimation::animation_variable_reference::AnimationVariableReference;
    use zomboid_engine::zombie::zomboid_file_system::{list_files, ZomboidFileSystem};
    fn q(s: Option<&str>) -> String {
        let Some(s) = s else {
            return "null".into();
        };
        let mut b = String::from("\"");
        for c in s.encode_utf16() {
            if (0x20..0x7f).contains(&c) && c != u16::from(b'"') && c != u16::from(b'\\') {
                b.push(char::from(c as u8));
            } else {
                let _ = write!(b, "\\u{c:04x}");
            }
        }
        b.push('"');
        b
    }
    let f = float_to_string;
    let b = |v: bool| u8::from(v);
    fn e(v: Option<&str>) -> &str {
        v.unwrap_or("null")
    }
    fn reference(r: Option<&AnimationVariableReference>) -> String {
        match r {
            None => "null".into(),
            Some(r) => format!(
                "({},{})",
                q(r.sub_variable_source_name.as_deref()),
                q(Some(&r.name))
            ),
        }
    }
    fn idx(v: Option<usize>) -> String {
        v.map_or("null".into(), |i| i.to_string())
    }
    fn cond(sb: &mut String, indent: &str, tag: &str, i: usize, c: &AnimCondition) {
        let _ = writeln!(
            sb,
            "{indent}{tag} {i} name={} type={} value={} f={} b={} s={} ref={}",
            q(Some(&c.name)),
            e(c.type_.map(|t| t.name())),
            q(Some(&c.value)),
            float_to_string(c.float_value),
            u8::from(c.bool_value),
            q(Some(&c.string_value)),
            reference(c.variable_reference.as_ref())
        );
    }
    let node = |sb: &mut String, tag: &str, n: &AnimNode| {
        let _ = writeln!(
            sb,
            "  {tag} {} idle={} priority={} conditionPriority={} abstract={} parent=1",
            q(Some(&n.name)),
            b(n.is_idle_anim),
            n.priority,
            n.condition_priority,
            b(n.is_abstract())
        );
        let alternates: Vec<String> = n.alternate_anims.iter().map(|s| q(Some(s))).collect();
        let _ = writeln!(
            sb,
            "   anim={} alternates=[{}]",
            q(Some(&n.anim_name)),
            alternates.join(",")
        );
        let _ = writeln!(
            sb,
            "   grapple matching={} fwd={} yaw={} tweenIn={} behaviour={}",
            q(Some(&n.matching_grappled_anim_node)),
            f(n.grapple_offset_forward),
            f(n.grapple_offset_yaw),
            f(n.grapple_tween_in_time),
            e(n.grappler_offset_behaviour.map(|v| v.name()))
        );
        let _ = writeln!(
            sb,
            "   ragdoll={} chance={} startMin={} startMax={} maxTime={}",
            b(n.is_ragdoll),
            f(n.chance_to_ragdoll),
            f(n.ragdoll_start_time_min),
            f(n.ragdoll_start_time_max),
            f(n.ragdoll_max_time)
        );
        let _ = writeln!(
            sb,
            "   deferred bone={} axis={} rotation={} movement={} rotationScale={}",
            q(Some(&n.deferred_bone_name)),
            e(n.deferred_bone_axis.map(|v| v.name())),
            b(n.use_defered_rotation),
            b(n.use_deferred_movement),
            f(n.deferred_rotation_scale)
        );
        let _ = writeln!(
            sb,
            "   looped={} blendTime={} blendOutTime={} stopOnExit={} earlyTransitionOut={}",
            b(n.is_looped),
            f(n.blend_time),
            f(n.blend_out_time),
            b(n.stop_anim_on_exit),
            b(n.early_transition_out)
        );
        let _ = writeln!(
            sb,
            "   speedScale={} variable={} randomMin={} randomMax={} parsed={}",
            q(Some(&n.speed_scale)),
            q(n.speed_scale_variable.as_deref()),
            f(n.speed_scale_random_multiplier_min),
            f(n.speed_scale_random_multiplier_max),
            f(n.speed_scale_f)
        );
        let _ = writeln!(
            sb,
            "   randomAdvance={} maxTorsoTwist={} scalar={} scalar2={} reverse={} sync={} trackTimeTo={}",
            f(n.random_advance_fraction),
            f(n.max_torso_twist),
            q(Some(&n.scalar)),
            q(Some(&n.scalar2)),
            b(n.is_anim_reverse),
            b(n.sync_tracking_enabled),
            q(n.track_time_to_variable.as_deref())
        );
        sb.push_str("   blendCurve ");
        match n.blend_curve.as_ref().and_then(|s| s.interpolator.as_ref()) {
            None => sb.push_str("null"),
            Some(i) => {
                sb.push_str(i.simple_name());
                match i {
                    IInterpolator::Sequence(s) => {
                        let raw: Vec<String> = s
                            .sequence_raw
                            .iter()
                            .map(|p| {
                                format!(
                                    "{}:{}:{}",
                                    f(p.x),
                                    f(p.y),
                                    e(p.lerp_type.map(|t| t.name()))
                                )
                            })
                            .collect();
                        let sorted: Vec<String> = s
                            .sequence_entries
                            .iter()
                            .flatten()
                            .map(usize::to_string)
                            .collect();
                        let _ = write!(
                            sb,
                            " n={} min={} max={} raw=[{}] sorted=[{}]",
                            s.num_entries,
                            f(s.min_x),
                            f(s.max_x),
                            raw.join(","),
                            sorted.join(",")
                        );
                    }
                    IInterpolator::Polyline(p) => {
                        let raw: Vec<String> = p
                            .points_raw
                            .iter()
                            .map(|p| format!("{}:{}", f(p.x), f(p.y)))
                            .collect();
                        let pts: Vec<String> = p
                            .points
                            .iter()
                            .flatten()
                            .map(|p| format!("{}:{}", f(p[0]), f(p[1])))
                            .collect();
                        let _ = write!(
                            sb,
                            " n={} min={} max={} raw=[{}] points=[{}]",
                            p.num_points,
                            f(p.min_x),
                            f(p.max_x),
                            raw.join(","),
                            pts.join(",")
                        );
                    }
                    IInterpolator::Linear(l) => {
                        let _ = write!(sb, " y0={} y1={}", f(l.start_value), f(l.end_value));
                    }
                    IInterpolator::EaseInQuad(l) => {
                        let _ = write!(sb, " y0={} y1={}", f(l.start_value), f(l.end_value));
                    }
                    IInterpolator::EaseOutInQuad(l) => {
                        let _ = write!(sb, " y0={} y1={}", f(l.start_value), f(l.end_value));
                    }
                    IInterpolator::EaseOutQuad(l) => {
                        let _ = write!(sb, " y0={} y1={}", f(l.start_value), f(l.end_value));
                    }
                    IInterpolator::One | IInterpolator::Zero => {}
                }
            }
        }
        sb.push('\n');
        if n.blend_curve
            .as_ref()
            .is_some_and(|s| s.interpolator.is_none())
        {
            sb.push_str("   blendCurveSlot empty\n");
        }
        for (i, bl) in n.blends2d.iter().enumerate() {
            let _ = writeln!(
                sb,
                "   blend {i} anim={} x={} y={} speed={} id={}",
                q(Some(&bl.anim_name)),
                f(bl.pos_x),
                f(bl.pos_y),
                f(bl.speed_scale),
                q(bl.reference_id.as_deref())
            );
        }
        for (i, c) in n.conditions.iter().enumerate() {
            cond(sb, "   ", "cond", i, c);
        }
        for (i, ev) in n.events.iter().enumerate() {
            let class = match ev.kind {
                AnimEventKind::Plain => "AnimEvent",
                AnimEventKind::SetVariable { .. } => "AnimEventSetVariable",
                AnimEventKind::FlagWhileAlive { .. } => "AnimEventFlagWhileAlive",
            };
            let _ = write!(
                sb,
                "   event {i} {class} name={} time={} pc={} param={} parent=1",
                q(ev.event_name.as_deref()),
                e(ev.time.map(|t| t.name())),
                f(ev.time_pc),
                q(ev.parameter_value.as_deref())
            );
            match &ev.kind {
                AnimEventKind::Plain => {}
                AnimEventKind::SetVariable {
                    variable_reference,
                    set_variable_value,
                } => {
                    let _ = write!(
                        sb,
                        " ref={} set={}",
                        reference(Some(variable_reference)),
                        q(Some(set_variable_value))
                    );
                }
                AnimEventKind::FlagWhileAlive {
                    variable_reference,
                    flag_value,
                } => {
                    let _ = write!(
                        sb,
                        " ref={} flag={}",
                        reference(Some(variable_reference)),
                        b(*flag_value)
                    );
                }
            }
            sb.push('\n');
        }
        for (i, t) in n.blend_tris.iter().enumerate() {
            let _ = writeln!(
                sb,
                "   tri {i} {} {} {}",
                idx(t.node1),
                idx(t.node2),
                idx(t.node3)
            );
        }
        match &n.blend2d_picker {
            None => sb.push_str("   picker null\n"),
            Some(p) => {
                let mut hull = p.hull.clone();
                hull.sort_unstable();
                let hull: Vec<String> = hull.iter().map(usize::to_string).collect();
                let _ = writeln!(sb, "   picker tris=1 hull=[{}]", hull.join(","));
            }
        }
        for (i, t) in n.transitions.iter().enumerate() {
            let _ = writeln!(
                sb,
                "   trans {i} source={} target={} anim={} bone={} axis={} rotation={} movement={} rotationScale={} syncAdjust={} in={} out={} speed={} parsed={}",
                q(t.source.as_deref()),
                q(t.target.as_deref()),
                q(t.anim_name.as_deref()),
                q(t.deferred_bone_name.as_deref()),
                e(t.deferred_bone_axis.map(|v| v.name())),
                b(t.use_defered_rotation),
                b(t.use_deferred_movement),
                f(t.deferred_rotation_scale),
                f(t.sync_adjust_time),
                f(t.blend_in_time),
                f(t.blend_out_time),
                f(t.speed_scale),
                b(t.is_parsed)
            );
            for (j, c) in t.conditions.iter().enumerate() {
                cond(sb, "    ", "tcond", j, c);
            }
        }
        for (i, w) in n.sub_state_bone_weights.iter().enumerate() {
            let _ = writeln!(
                sb,
                "   bone {i} {} weight={} descendants={}",
                q(w.bone_name.as_deref()),
                f(w.weight),
                b(w.include_descendants)
            );
        }
        let _ = writeln!(sb, "   transitionOut={}", idx(n.transition_out));
    };

    let fs = ZomboidFileSystem::init(root)?;
    let mut sb = String::new();
    let dir_names = |dir: &Path| -> Vec<String> {
        let mut names: Vec<String> = list_files(dir)
            .unwrap_or_default()
            .into_iter()
            .filter(|p| p.is_dir())
            .filter_map(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
            .collect();
        names.sort_by(|a, b| string_compare_to(a, b));
        names
    };
    let mut sets = AnimationSets::default();
    for name in dir_names(&fs.base().join("media/AnimSets")) {
        if let Err(err) = sets.get_animation_set(&fs, &name, false) {
            let _ = writeln!(sb, "SETERROR {} {err}", q(Some(&name)));
        }
    }
    let mut set_keys: Vec<&String> = sets.set_map.keys().collect();
    set_keys.sort_by(|a, b| string_compare_to(a, b));
    for key in set_keys {
        let set = &sets.set_map[key];
        let _ = writeln!(
            sb,
            "ANIMSET {} name={} states={}",
            q(Some(key)),
            q(Some(&set.name)),
            set.states.len()
        );
        let mut state_keys: Vec<&String> = set.states.keys().collect();
        state_keys.sort_by(|a, b| string_compare_to(a, b));
        for sk in state_keys {
            let st = &set.states[sk];
            let _ = writeln!(
                sb,
                " STATE {} name={} set=1 default={} nodes={} abstract={}",
                q(Some(sk)),
                q(Some(&st.name)),
                st.default_index,
                st.nodes.len(),
                st.abstract_nodes.len()
            );
            for (i, n) in st.nodes.iter().enumerate() {
                node(&mut sb, &format!("NODE {i}"), n);
            }
            for (i, n) in st.abstract_nodes.iter().enumerate() {
                node(&mut sb, &format!("ABSTRACT {i}"), n);
            }
        }
    }
    let base = format!("{}/", fs.base().to_string_lossy());
    let mut failed: Vec<String> = sets
        .asset_manager
        .assets
        .iter()
        .filter(|(_, n)| n.is_none())
        .map(|(p, _)| format!("{} failed", p.strip_prefix(&base).unwrap_or(p)))
        .collect();
    failed.sort_by(|a, b| string_compare_to(a, b));
    let _ = writeln!(sb, "ASSETS {}", sets.asset_manager.assets.len());
    for s in failed {
        let _ = writeln!(sb, "ASSET {}", q(Some(&s)));
    }
    let mut groups = ActionGroups::default();
    for name in dir_names(&fs.base().join("media/actiongroups")) {
        groups.get_action_group(&fs, &name);
    }
    fn strings(v: Option<&Vec<String>>) -> String {
        match v {
            None => "null".into(),
            Some(v) => {
                let s: Vec<String> = v.iter().map(|s| q(Some(s))).collect();
                format!("[{}]", s.join(","))
            }
        }
    }
    fn value(v: Option<&Value>) -> String {
        match v {
            None => "null".into(),
            Some(Value::String(s)) => format!("s{}", q(Some(s))),
            Some(Value::Integer(i)) => format!("i{i}"),
            Some(Value::Float(x)) => format!("f{}", float_to_string(*x)),
            Some(Value::Boolean(x)) => format!("b{x}"),
            Some(Value::Lookup(r)) => format!("lookup{}", reference(Some(r))),
        }
    }
    let mut group_keys: Vec<&String> = groups.action_group_map.keys().collect();
    group_keys.sort_by(|a, b| string_compare_to(a, b));
    for key in group_keys {
        let g = &groups.action_group_map[key];
        let _ = writeln!(
            sb,
            "ACTIONGROUP {} name={} initial={} initialState={} states={}",
            q(Some(key)),
            q(Some(&g.name)),
            q(g.initial_state_name.as_deref()),
            g.get_initial_state()
                .map_or("null".into(), |s| q(Some(&s.name))),
            g.states.len()
        );
        for (i, s) in g.states.iter().enumerate() {
            let vars: Vec<_> = s.state_variables.get_game_variables().collect();
            let _ = writeln!(
                sb,
                " ASTATE {i} {} group=1 grappler={} tags={} childTags={} vars={} transitions={}",
                q(Some(&s.name)),
                b(s.is_grappler_state),
                strings(s.tags.as_ref()),
                strings(s.child_tags.as_ref()),
                vars.len(),
                s.transitions.len()
            );
            for slot in vars {
                let _ = writeln!(
                    sb,
                    "  var {} {} {} value={} readOnly={}",
                    q(Some(&slot.key)),
                    slot.class_name(),
                    slot.type_name(),
                    q(slot.get_value_string().as_deref()),
                    b(slot.read_only)
                );
            }
            for (j, t) in s.transitions.iter().enumerate() {
                let _ = writeln!(
                    sb,
                    "  atrans {j} to={} asSubstate={} transitionOut={} forceParent={} priority={} conditions={}",
                    q(t.transition_to.as_deref()),
                    b(t.as_substate),
                    b(t.transition_out),
                    b(t.force_parent),
                    t.condition_priority,
                    t.conditions.len()
                );
                for c in &t.conditions {
                    sb.push_str("    acond ");
                    match c {
                        IActionCondition::CharacterVariable(cv) => {
                            let _ = write!(
                                sb,
                                "CharacterVariableCondition op={} lhs={} rhs={}",
                                e(cv.op.map(|o| o.name())),
                                value(cv.lhs_value.as_ref()),
                                value(cv.rhs_value.as_ref())
                            );
                        }
                        IActionCondition::EventOccurred(n) => {
                            let _ = write!(sb, "EventOccurred {}", q(Some(n)));
                        }
                        IActionCondition::EventNotOccurred(n) => {
                            let _ = write!(sb, "EventNotOccurred {}", q(Some(n)));
                        }
                        IActionCondition::LuaCall => sb.push_str("LuaCall"),
                    }
                    sb.push('\n');
                }
            }
        }
        let mut lookup: Vec<(&String, &usize)> = g.state_lookup.iter().collect();
        lookup.sort_by(|a, b| string_compare_to(a.0, b.0));
        for (k, i) in lookup {
            let _ = writeln!(sb, " LOOKUP {} {i}", q(Some(k)));
        }
        let mut names: Vec<(&i32, &String)> = g.state_name_lookup.iter().collect();
        names.sort_by_key(|(h, _)| **h);
        for (h, n) in names {
            let _ = writeln!(sb, " NAMELOOKUP {h} {}", q(Some(n)));
        }
    }
    Ok(sb)
}

/// The Kahlua compiler over every `media/lua/**/*.lua` and the cases file, in the format of
/// `tests/original_probe/LuaCompileProbe.java`.
pub fn luacompile(root: &Path, cases: &Path) -> Result<String, String> {
    use zomboid_engine::java::{string_compare_to, utf8_decode_jdk};
    use zomboid_engine::org::luaj::kahluafork::compiler::lex_state::compile;
    use zomboid_engine::se::krka::kahlua::vm::prototype::{Constant, Prototype};
    fn esc(sb: &mut String, s: Option<&str>) {
        let Some(s) = s else {
            sb.push_str("null");
            return;
        };
        sb.push('"');
        for c in s.encode_utf16() {
            if (0x20..0x7f).contains(&c) && c != u16::from(b'"') && c != u16::from(b'\\') {
                sb.push(char::from(c as u8));
            } else {
                let _ = write!(sb, "\\u{c:04x}");
            }
        }
        sb.push('"');
    }
    fn proto(sb: &mut String, p: &Prototype, depth: usize) {
        let _ = write!(sb, "P {depth} ");
        esc(sb, p.name.as_deref());
        sb.push(' ');
        esc(sb, p.file.as_deref());
        sb.push(' ');
        esc(sb, p.filename.as_deref());
        let _ = writeln!(
            sb,
            " {} {} {} {} {} {} {} {}",
            p.num_params,
            u8::from(p.is_vararg),
            p.max_stacksize,
            p.num_upvalues,
            p.code.len(),
            p.constants.len(),
            p.prototypes.len(),
            p.locvars.len()
        );
        sb.push('C');
        for c in &p.code {
            let _ = write!(sb, " {c}");
        }
        sb.push_str("\nL");
        for l in &p.lines {
            let _ = write!(sb, " {l}");
        }
        sb.push_str("\nK");
        for k in &p.constants {
            sb.push(' ');
            match k {
                Constant::Nil => sb.push_str("nil"),
                Constant::Boolean(b) => sb.push(if *b { 'T' } else { 'F' }),
                Constant::Number(d) => {
                    let _ = write!(sb, "d{:x}", d.to_bits());
                }
                Constant::String(s) => {
                    sb.push('s');
                    esc(sb, Some(s));
                }
            }
        }
        sb.push_str("\nV");
        for v in &p.locvars {
            sb.push(' ');
            esc(sb, Some(v));
        }
        sb.push('\n');
        for c in &p.prototypes {
            proto(sb, c, depth + 1);
        }
    }
    fn run(sb: &mut String, chars: Vec<u16>, name: &str, file: &str, full: &str) {
        match compile(chars, Some(name), None, Some(file), Some(full)) {
            Ok(p) => proto(sb, &p, 0),
            Err(e) => {
                sb.push_str("ERR ");
                let msg = if e.starts_with("java.") {
                    e
                } else {
                    format!("se.krka.kahlua.vm.KahluaException: {e}")
                };
                esc(sb, Some(&msg));
                sb.push('\n');
            }
        }
    }
    /// The bytes as the original's UTF-8 InputStreamReader yields them (valid UTF-8 exactly; malformed input with
    /// the String decoder's replacement rules, which is not measured).
    fn chars(bytes: &[u8]) -> Vec<u16> {
        match std::str::from_utf8(bytes) {
            Ok(s) => s.encode_utf16().collect(),
            Err(_) => utf8_decode_jdk(bytes),
        }
    }
    fn gen(kind: &str, n: usize) -> String {
        let mut sb = String::new();
        match kind {
            "bigtable" => {
                sb.push_str("local t = {");
                for i in 0..n {
                    let _ = write!(sb, "{},", i % 7);
                }
                sb.push_str("}\n");
            }
            "locals" => {
                for i in 0..n {
                    let _ = writeln!(sb, "local a{i} = {i}");
                }
            }
            "nest" => {
                sb.push_str("x = ");
                sb.push_str(&"(".repeat(n));
                sb.push('1');
                sb.push_str(&")".repeat(n));
                sb.push('\n');
            }
            "upvals" => {
                sb.push_str("local function f()\n");
                for i in 0..n {
                    let _ = writeln!(sb, "local u{i} = {i}");
                }
                sb.push_str("return function() return ");
                for i in 0..n {
                    let _ = write!(sb, "{}u{i}", if i == 0 { "" } else { " + " });
                }
                sb.push_str(" end\nend\n");
            }
            "jump" => {
                sb.push_str("while x do\n");
                for i in 0..n {
                    let _ = writeln!(sb, "y = y + {i}");
                }
                sb.push_str("end\n");
            }
            _ => sb.push_str("error('unknown generator')\n"),
        }
        sb
    }
    fn walk(dir: &Path, out: &mut Vec<std::path::PathBuf>) -> Result<(), String> {
        let rd = std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        for entry in rd {
            let p = entry.map_err(|e| e.to_string())?.path();
            if p.is_dir() {
                walk(&p, out)?;
            } else if p.extension().is_some_and(|x| x == "lua") {
                out.push(p);
            }
        }
        Ok(())
    }
    let base = root.join("media/lua");
    let mut paths = Vec::new();
    walk(&base, &mut paths)?;
    let mut files: Vec<String> = paths
        .iter()
        .filter_map(|p| p.strip_prefix(&base).ok())
        .map(|r| format!("media/lua/{}", r.to_string_lossy().replace('\\', "/")))
        .collect();
    files.sort_by(|a, b| string_compare_to(a, b));
    let mut sb = String::new();
    for rel in &files {
        let _ = writeln!(sb, "F {rel}");
        let name = rel.rsplit('/').next().unwrap_or(rel);
        let bytes = std::fs::read(root.join(rel)).map_err(|e| format!("{rel}: {e}"))?;
        run(&mut sb, chars(&bytes), name, name, rel);
    }
    let all = std::fs::read_to_string(cases).map_err(|e| format!("{}: {e}", cases.display()))?;
    for (i, case) in all.split("\n--@@case\n").enumerate().skip(1) {
        let src = match case.strip_prefix("--@@gen ") {
            Some(g) => {
                let mut it = g.split_whitespace();
                let kind = it.next().unwrap_or("");
                let n = it.next().and_then(|x| x.parse().ok()).unwrap_or(0);
                gen(kind, n)
            }
            None => case.to_string(),
        };
        let _ = writeln!(sb, "F case{i}");
        run(
            &mut sb,
            src.encode_utf16().collect(),
            &format!("case{i}"),
            "cases",
            "tests/lua_compile_cases.txt",
        );
    }
    Ok(sb)
}

/// The Kahlua VM over each case of the cases file, in the format of `tests/original_probe/LuaVmProbe.java`.
pub fn luavm(root: &Path, cases: &Path) -> Result<String, String> {
    use zomboid_engine::se::krka::kahlua::j2se::j2se_platform::J2SEPlatform;
    use zomboid_engine::se::krka::kahlua::luaj::compiler::lua_compiler::{
        loadstring, CompileFiles,
    };
    use zomboid_engine::se::krka::kahlua::vm::kahlua_thread::KahluaThread;
    use zomboid_engine::se::krka::kahlua::vm::object::{Object, Throwable};
    fn esc(sb: &mut String, s: &str) {
        sb.push('"');
        for c in s.encode_utf16() {
            if (0x20..0x7f).contains(&c) && c != u16::from(b'"') && c != u16::from(b'\\') {
                sb.push(char::from(c as u8));
            } else {
                let _ = write!(sb, "\\u{c:04x}");
            }
        }
        sb.push('"');
    }
    fn esc_units(sb: &mut String, s: &[u16]) {
        sb.push('"');
        for &c in s {
            if (0x20..0x7f).contains(&c) && c != u16::from(b'"') && c != u16::from(b'\\') {
                sb.push(char::from(c as u8));
            } else {
                let _ = write!(sb, "\\u{c:04x}");
            }
        }
        sb.push('"');
    }
    fn render(sb: &mut String, o: &Object) {
        match o {
            Object::Nil => sb.push_str("nil"),
            Object::Boolean(b) => sb.push_str(if *b { "true" } else { "false" }),
            Object::Number(d) => {
                let _ = write!(sb, "d{:x}", d.to_bits());
            }
            Object::String(s) => {
                sb.push('s');
                esc_units(sb, s);
            }
            Object::Table(_) => sb.push_str("table"),
            Object::Closure(_) => sb.push_str("closure"),
            Object::Function(_) => sb.push_str("jfunction"),
            Object::Coroutine(_) => sb.push_str("coroutine"),
            Object::Userdata(u) => {
                let any = u.as_any();
                match any
                    .downcast_ref::<zomboid_engine::se::krka::kahlua::vm::object::ThrowableObject>()
                {
                    Some(t) => {
                        let _ = write!(sb, "exc:{}", t.0.class_name());
                    }
                    None => {
                        let _ = write!(sb, "userdata:{}", u.class().name);
                    }
                }
            }
        }
    }
    fn exc_text(e: &Throwable) -> String {
        format!(
            "{}: {}",
            e.class_name(),
            e.get_message().unwrap_or_else(|| "null".into())
        )
    }
    let all = std::fs::read_to_string(cases).map_err(|e| format!("{}: {e}", cases.display()))?;
    let mut sb = String::new();
    for (i, case) in all.split("\n--@@case\n").enumerate().skip(1) {
        let _ = writeln!(sb, "F case{i}");
        let platform = J2SEPlatform::new();
        let env = platform
            .new_environment(root)
            .map_err(|e| format!("environment: {}", exc_text(&e)))?;
        let mut thread = KahluaThread::new(platform.clone(), env.clone());
        let units: Vec<u16> = case.encode_utf16().collect();
        let closure = match loadstring(
            &units,
            Some(&format!("case{i}")),
            env,
            &CompileFiles::default(),
        ) {
            Ok(c) => c,
            Err(e) => {
                sb.push_str("ERR ");
                esc(&mut sb, &exc_text(&e));
                sb.push('\n');
                continue;
            }
        };
        let r = match thread.pcall_args(&Object::Closure(closure), &[]) {
            Ok(r) => r,
            Err(e) => {
                sb.push_str("THROW ");
                esc(&mut sb, &exc_text(&e));
                sb.push('\n');
                continue;
            }
        };
        for line in thread.out.split('\n') {
            if line.is_empty() {
                continue;
            }
            sb.push_str("O ");
            esc(&mut sb, line);
            sb.push('\n');
        }
        let _ = write!(sb, "R {}", r.len());
        for o in &r {
            sb.push(' ');
            render(&mut sb, o);
        }
        sb.push('\n');
    }
    Ok(sb)
}

/// `pz-dump javachars <root>`: java.lang.Character.isDigit/isLowerCase/isUpperCase of every UTF-16 unit, as
/// `JavaCharsProbe` prints them.
pub fn javachars() -> Result<String, String> {
    use std::fmt::Write as _;
    use zomboid_engine::java_chars::{is_digit, is_lower_case, is_upper_case};
    let mut out = String::new();
    for c in 0..=u16::MAX {
        let f = u8::from(is_digit(c))
            | (u8::from(is_lower_case(c)) << 1)
            | (u8::from(is_upper_case(c)) << 2);
        if f != 0 {
            let _ = writeln!(out, "{c:x} {f}");
        }
    }
    Ok(out)
}

/// `tests/original_probe/WorldGenSimplexProbe.java` on the port: the 3D SimplexNoise over a 16x16x16 grid, then
/// WorldGenSimplexGenerator noise/selector for six seeds over an 8x8 grid, doubles in Double.toString form.
pub fn wgsimplex() -> Result<String, String> {
    use std::fmt::Write as _;
    use zomboid_engine::java;
    use zomboid_engine::zombie::iso::weather::simplex_noise;
    use zomboid_engine::zombie::iso::worldgen::world_gen_simplex_generator::WorldGenSimplexGenerator;
    let mut out = String::new();
    const C: [f64; 16] = [
        -1048576.5, -300.25, -128.0, -1.75, -0.5, 0.0, 0.25, 1.0, 1.5, 63.75, 64.0, 127.999, 128.0,
        300.0, 4096.5, 1048576.0,
    ];
    for x in C {
        for y in C {
            for z in C {
                let n = simplex_noise::noise3(x, y, z);
                let _ = writeln!(
                    out,
                    "SIMPLEX3 {},{},{}={}",
                    java::double_to_string(x),
                    java::double_to_string(y),
                    java::double_to_string(z),
                    java::double_to_string(n)
                );
            }
        }
    }
    const SEEDS: [i64; 6] = [1, 42, -1, 2147483647, i64::MIN, 0x1234567890ABCDEF];
    const W: [f64; 8] = [-300.25, -1.75, 0.0, 0.25, 1.0, 63.75, 128.0, 300.0];
    for seed in SEEDS {
        let g = WorldGenSimplexGenerator::new(seed);
        for x in W {
            for y in W {
                let n = g.noise(x, y);
                let _ = writeln!(
                    out,
                    "WGSG {seed},{},{} landscape={} plant={} bush={} temperature={} hygrometry={} ore={}",
                    java::double_to_string(x),
                    java::double_to_string(y),
                    java::double_to_string(n.landscape),
                    java::double_to_string(n.plant),
                    java::double_to_string(n.bush),
                    java::double_to_string(n.temperature),
                    java::double_to_string(n.hygrometry),
                    java::double_to_string(n.ore)
                );
                let _ = writeln!(
                    out,
                    "WGSEL {seed},{},{}={}",
                    java::double_to_string(x),
                    java::double_to_string(y),
                    java::double_to_string(g.selector(x, y))
                );
            }
        }
    }
    Ok(out)
}

/// `tests/original_probe/WorldGenBiomesProbe.java` on the port: the biome registry the game's own Lua
/// defines - the client's startup and the Lua state as the loot case builds them, LoadDirBase of shared and
/// server (the dedicated server's Lua, where `media/lua/server/WorldGen/*.lua` fills the `worldgen` table),
/// then WorldGenReader over it: both biome tables with every field, the six selection threshold lists, the
/// tile priorities, and BiomeRegistry's picks over the stage-1 simplex for a grid of seeds and coordinates
/// (the maps WorldGenChunk's constructor keeps: loadBiomes filtered by `generate`).
pub fn wgbiomes(root: &std::path::Path, cache_dir: &str) -> Result<String, String> {
    use std::fmt::Write as _;
    use std::rc::Rc;
    use zomboid_engine::java::JavaHashMap;
    use zomboid_engine::se::krka::kahlua::j2se::kahlua_table_impl::KahluaTableImpl;
    use zomboid_engine::se::krka::kahlua::vm::object::Object;
    use zomboid_engine::zombie::game_window::ClientStartup;
    use zomboid_engine::zombie::iso::worldgen::biomes::biome::Biome;
    use zomboid_engine::zombie::iso::worldgen::biomes::biome_registry::{BiomeRegistry, Selection};
    use zomboid_engine::zombie::iso::worldgen::biomes::biome_type::{
        BiomeEnum, Bush, EnumSet, Hygrometry, Landscape, OreLevel, Plant, Temperature,
    };
    use zomboid_engine::zombie::iso::worldgen::biomes::feature::Feature;
    use zomboid_engine::zombie::iso::worldgen::biomes::feature_type::FeatureType;
    use zomboid_engine::zombie::iso::worldgen::biomes::probability::Probability;
    use zomboid_engine::zombie::iso::worldgen::world_gen_reader::WorldGenReader;
    use zomboid_engine::zombie::iso::worldgen::world_gen_simplex_generator::WorldGenSimplexGenerator;
    use zomboid_engine::zombie::lua::exposed_methods::sandbox_options_instance;
    use zomboid_engine::zombie::lua::lua_manager::LuaManager;

    fn esc(s: &str) -> String {
        let mut b = String::from("\"");
        for c in s.encode_utf16() {
            if (0x20..0x7f).contains(&c) && c != u16::from(b'"') && c != u16::from(b'\\') {
                b.push(char::from(c as u8));
            } else {
                let _ = write!(b, "\\u{c:04x}");
            }
        }
        b.push('"');
        b
    }
    fn d_hex(d: f64) -> String {
        format!("d{:016x}", d.to_bits())
    }
    fn f_hex(f: f32) -> String {
        format!("f{:08x}", f.to_bits())
    }
    fn names(list: &[String]) -> String {
        format!(
            "[{}]",
            list.iter().map(|s| esc(s)).collect::<Vec<_>>().join(",")
        )
    }
    fn doubles(list: &[f64]) -> String {
        format!(
            "[{}]",
            list.iter().map(|d| d_hex(*d)).collect::<Vec<_>>().join(",")
        )
    }
    fn sets<T: BiomeEnum>(set: &EnumSet<T>) -> String {
        format!(
            "[{}]",
            set.iter().map(|t| t.name()).collect::<Vec<_>>().join(",")
        )
    }
    fn p_str(p: &Probability) -> String {
        // the probe prints "P " + (ProbaString ? "s" : "") + f%08x: `s` marks the sandbox-option kind, the
        // plain double gets NO marker of its own - the leading `f` comes from f_hex itself
        match p {
            Probability::Double(d) => format!("P {}", f_hex(*d as f32)),
            Probability::Sandbox(f) => format!("P s{}", f_hex(*f)),
        }
    }
    fn feature_lines(out: &mut String, features: &[Feature], f_prefix: &str, g_prefix: &str) {
        for (i, f) in features.iter().enumerate() {
            let _ = writeln!(
                out,
                "{f_prefix} {} MIN {} MAX {} {}",
                i + 1,
                f.min_size,
                f.max_size,
                p_str(&f.probability)
            );
            for (j, g) in f.tile_groups.iter().enumerate() {
                let _ = writeln!(
                    out,
                    "{g_prefix} {} {} {}x{} {}",
                    i + 1,
                    j + 1,
                    g.sx,
                    g.sy,
                    names(&g.tiles)
                );
            }
        }
    }
    // one biome's block: the same lines the probe prints, `id` the biome's name or its sub-biome path
    fn biome_block(out: &mut String, id: &str, b: &Biome, depth: usize) {
        let _ = writeln!(
            out,
            "B {id} PARENT {} GEN {} ZOMBIES {}",
            match &b.parent {
                Some(p) => esc(p),
                None => "nil".to_string(),
            },
            i32::from(b.generate),
            f_hex(b.zombies)
        );
        match &b.grass {
            Some(g) => {
                let _ = writeln!(
                    out,
                    "B {id} GRASS fern={} div={} noGrass={} grass={}",
                    f_hex(g.fern_chance),
                    f_hex(g.no_grass_div),
                    doubles(&g.no_grass_stages),
                    doubles(&g.grass_stages)
                );
            }
            None => {
                let _ = writeln!(out, "B {id} GRASS nil");
            }
        }
        let _ = writeln!(
            out,
            "B {id} SETS landscape={} plant={} bush={} temperature={} hygrometry={} ore={}",
            sets(&b.landscape),
            sets(&b.plant),
            sets(&b.bush),
            sets(&b.temperature),
            sets(&b.hygrometry),
            sets(&b.ore_level)
        );
        match &b.features {
            None => {
                let _ = writeln!(out, "B {id} FEATURES nil");
            }
            Some(features) => {
                let present = FeatureType::ALL
                    .iter()
                    .filter(|t| features.get(**t).is_some())
                    .count();
                let _ = writeln!(out, "B {id} FEATURES {present}");
                for t in FeatureType::ALL {
                    if let Some(list) = features.get(*t) {
                        let _ = writeln!(out, "B {id} FEAT {} {}", t.name(), list.len());
                        feature_lines(
                            out,
                            list,
                            &format!("F {id} {}", t.name()),
                            &format!("GF {id} {}", t.name()),
                        );
                    }
                }
            }
        }
        match &b.replacements {
            None => {
                let _ = writeln!(out, "B {id} REPLACEMENTS nil");
            }
            Some(replacements) => {
                // the shared map lives behind Rc<RefCell<..>>: one borrow serves the size and the walk
                let replacements = replacements.borrow();
                let _ = writeln!(out, "B {id} REPLACEMENTS {}", replacements.len());
                for (key, features) in replacements.iter() {
                    let _ = writeln!(out, "R {id} {} {}", esc(key), features.len());
                    feature_lines(
                        out,
                        features,
                        &format!("RF {id} {}", esc(key)),
                        &format!("GRF {id} {}", esc(key)),
                    );
                }
            }
        }
        match &b.placements {
            None => {
                let _ = writeln!(out, "B {id} PLACEMENTS nil");
            }
            Some(placements) => {
                let _ = writeln!(out, "B {id} PLACEMENTS 5");
                for t in FeatureType::ALL {
                    let _ = writeln!(out, "PL {id} {} {}", t.name(), names(placements.get(*t)));
                }
            }
        }
        let sub_count = FeatureType::ALL
            .iter()
            .filter(|t| b.sub_biomes.get(**t).is_some())
            .count();
        let _ = writeln!(out, "B {id} SUBBIOMES {sub_count}");
        if depth < 8 {
            for t in FeatureType::ALL {
                let Some(inner) = b.sub_biomes.get(*t) else {
                    continue;
                };
                for st in FeatureType::ALL {
                    let Some(list) = inner.get(*st) else {
                        continue;
                    };
                    let _ = writeln!(out, "SB {id} {} {} {}", t.name(), st.name(), list.len());
                    for (k, sub) in list.iter().enumerate() {
                        let path = format!("{id}/{}/{}/{}", t.name(), st.name(), k + 1);
                        let _ = writeln!(out, "BB {path} {}", esc(&sub.name));
                        biome_block(out, &path, sub, depth + 1);
                    }
                }
            }
        }
        let _ = writeln!(out, "B {id} PROTECTED {}", names(&b.protected_list));
    }
    fn dump_map(out: &mut String, header: &str, raw: &JavaHashMap<Biome>) -> JavaHashMap<Biome> {
        let _ = writeln!(out, "{header} {}", raw.len());
        for (name, b) in raw.iter() {
            biome_block(out, name, b, 0);
        }
        // the map WorldGenChunk's constructor keeps: loadBiomes filtered by generate(), Collectors.toMap
        // into a fresh HashMap (inserted in this map's iteration order)
        let mut kept: JavaHashMap<Biome> = JavaHashMap::default();
        for (name, b) in raw.iter() {
            if b.generate {
                kept.put(name.to_string(), b.clone());
            }
        }
        let kept_names: Vec<String> = kept.iter().map(|(k, _)| k.to_string()).collect();
        let _ = writeln!(out, "{header}GEN {} {}", kept.len(), names(&kept_names));
        kept
    }
    fn selection_lines<T: BiomeEnum>(out: &mut String, sel: &Selection<T>) {
        let _ = writeln!(out, "SEL {} {}", T::KEY, sel.iter().count());
        for (member, list) in sel.iter() {
            let _ = writeln!(out, "S {} {} {}", T::KEY, member.name(), doubles(list));
        }
    }

    let startup = ClientStartup::init(root).map_err(|e| format!("startup: {e}"))?;
    let mut lm =
        LuaManager::init_after(root, Some(Rc::new(startup))).map_err(|e| format!("init: {e:?}"))?;
    *lm.statics.cache_dir.borrow_mut() = Some(cache_dir.to_string());
    zomboid_engine::zombie::lua::exposed_game::init_shared(&lm.statics)
        .map_err(|e| format!("initShared: {e}"))?;
    for dir in ["shared", "server"] {
        lm.load_dir_base(dir)
            .map_err(|e| format!("LoadDirBase {dir}: {e:?}"))?;
    }
    let worldgen = match KahluaTableImpl::rawget(&lm.env, &Object::str("worldgen"))
        .map_err(|e| format!("worldgen: {e:?}"))?
    {
        Object::Table(t) => t,
        Object::Nil => return Err("worldgen table missing from the Lua environment".into()),
        o => {
            return Err(format!(
                "ClassCastException: worldgen is not a table: {}",
                o.java_to_string()
            ))
        }
    };
    // SandboxOptions.instance, what ProbaString reads (resolved here; nothing between this and the parse
    // changes the options)
    let so = sandbox_options_instance(&lm.statics, &mut lm.thread)
        .map_err(|e| format!("SandboxOptions.instance: {e:?}"))?;
    let so_borrowed = so.0.borrow();
    let reader = WorldGenReader {
        sandbox: &so_borrowed,
    };
    let mut out = String::new();

    // WorldGenChunk.<init>'s table keys: the _override entries when a WorldGenOverride.lua set them
    let has_override = |key: &str| -> Result<bool, String> {
        Ok(!KahluaTableImpl::rawget(&worldgen, &Object::str(key))
            .map_err(|e| format!("{e:?}"))?
            .is_nil())
    };
    let biomes_key = if has_override("biomes_override")? {
        "biomes_override"
    } else {
        "biomes"
    };
    let biomes_map_key = "biomes_map"; // the constructor passes the literal key, no _override fallback
    let selection_key = if has_override("selection_override")? {
        "selection_override"
    } else {
        "selection"
    };
    let _ = writeln!(
        out,
        "KEYS biomes={biomes_key} biomes_map={biomes_map_key} selection={selection_key}"
    );

    let raw = reader
        .load_biomes(&worldgen, biomes_key)
        .map_err(|e| format!("loadBiomes: {e:?}"))?;
    let biomes = dump_map(&mut out, "BIOMES", &raw);
    let raw_map = reader
        .load_biomes(&worldgen, biomes_map_key)
        .map_err(|e| format!("loadBiomes(biomes_map): {e:?}"))?;
    let biomes_map = dump_map(&mut out, "BIOMESMAP", &raw_map);

    let landscape: Selection<Landscape> = reader
        .load_selection(&worldgen, selection_key)
        .map_err(|e| format!("loadSelection landscape: {e:?}"))?;
    selection_lines(&mut out, &landscape);
    let plant: Selection<Plant> = reader
        .load_selection(&worldgen, selection_key)
        .map_err(|e| format!("loadSelection plant: {e:?}"))?;
    selection_lines(&mut out, &plant);
    let bush: Selection<Bush> = reader
        .load_selection(&worldgen, selection_key)
        .map_err(|e| format!("loadSelection bush: {e:?}"))?;
    selection_lines(&mut out, &bush);
    let temperature: Selection<Temperature> = reader
        .load_selection(&worldgen, selection_key)
        .map_err(|e| format!("loadSelection temperature: {e:?}"))?;
    selection_lines(&mut out, &temperature);
    let hygrometry: Selection<Hygrometry> = reader
        .load_selection(&worldgen, selection_key)
        .map_err(|e| format!("loadSelection hygrometry: {e:?}"))?;
    selection_lines(&mut out, &hygrometry);
    let ore_level: Selection<OreLevel> = reader
        .load_selection(&worldgen, selection_key)
        .map_err(|e| format!("loadSelection ore_level: {e:?}"))?;
    selection_lines(&mut out, &ore_level);

    let priorities = reader
        .load_priorities(&worldgen, "priorities")
        .map_err(|e| format!("loadPriorities: {e:?}"))?;
    let _ = writeln!(out, "PRIORITIES {}", priorities.len());
    for (name, value) in priorities.iter() {
        let _ = writeln!(out, "P {} {}", esc(name), d_hex(*value));
    }

    // BiomeRegistry, as WorldGenChunk.getBiome calls it
    let mut registry = BiomeRegistry::default();
    const SEEDS: [i64; 6] = [1, 42, -1, 2147483647, i64::MIN, 0x1234567890ABCDEF];
    const CO: [i64; 8] = [0, 1, 7, 8, 63, 300, 2047, -1];
    for seed in SEEDS {
        let generator = WorldGenSimplexGenerator::new(seed);
        for x in CO {
            for y in CO {
                let pick = registry
                    .get(
                        &biomes,
                        generator.noise(x as f64, y as f64),
                        generator.selector(x as f64, y as f64),
                        &landscape,
                        &plant,
                        &bush,
                        &temperature,
                        &hygrometry,
                        &ore_level,
                    )
                    .map_err(|e| format!("BiomeRegistry.get: {e}"))?;
                let _ = writeln!(out, "GETBIOME {seed} {x} {y} {}", pick.name());
            }
        }
    }
    let map_filters: Vec<String> = biomes_map.iter().map(|(k, _)| k.to_string()).collect();
    for filter in &map_filters {
        for seed in SEEDS.iter().take(2) {
            let generator = WorldGenSimplexGenerator::new(*seed);
            for x in CO.iter().step_by(2) {
                for y in CO.iter().step_by(2) {
                    let pick = registry
                        .get_filtered(
                            &biomes_map,
                            Some(filter),
                            generator.noise(*x as f64, *y as f64),
                            generator.selector(*x as f64, *y as f64),
                            &bush,
                            &ore_level,
                        )
                        .map_err(|e| format!("BiomeRegistry.get(filtered): {e}"))?;
                    let name = match pick {
                        Some(p) => p.name().to_string(),
                        None => "null".to_string(),
                    };
                    let _ = writeln!(out, "GETMAPBIOME {} {seed} {x} {y} {name}", esc(filter));
                }
            }
        }
    }
    Ok(out)
}

/// `tests/original_probe/FdLibmProbe.java` on the port: java.lang.Math / StrictMath results as raw bits.
pub fn fdlibm() -> Result<String, String> {
    use std::fmt::Write as _;
    use zomboid_engine::java::Random;
    use zomboid_engine::java_fdlibm as m;
    const SPECIAL: [f64; 40] = [
        0.0,
        -0.0,
        1.0,
        -1.0,
        0.5,
        -0.5,
        2.0,
        -2.0,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NAN,
        5e-324,
        -5e-324,
        f64::MAX,
        -f64::MAX,
        f64::MIN_POSITIVE,
        std::f64::consts::PI,
        std::f64::consts::PI / 2.0,
        std::f64::consts::PI / 4.0,
        -std::f64::consts::PI / 2.0,
        3.0,
        10.0,
        0.1,
        709.782712893384,
        -745.1332191019411,
        710.4758600739439,
        22.0,
        1e-300,
        1e300,
        1e22,
        1e-8,
        0.9999995,
        1.0000009536743162,
        2.1474856959999995E9,
        1e-320,
        100.0,
        -100.0,
        1e6,
        7.0,
        -3.5,
    ];
    /// `Double.doubleToLongBits`: every NaN is 0x7ff8000000000000.
    fn bits(d: f64) -> u64 {
        if d.is_nan() {
            0x7ff8_0000_0000_0000
        } else {
            d.to_bits()
        }
    }
    fn input(r: &mut Random, i: usize) -> f64 {
        if i < SPECIAL.len() {
            return SPECIAL[i];
        }
        match i % 6 {
            0 => f64::from_bits(r.next_long() as u64),
            1 => r.next_double() * 2.0 - 1.0,
            2 => (r.next_double() * 2.0 - 1.0) * 10.0,
            3 => (r.next_double() * 2.0 - 1.0) * 1000.0,
            4 => (r.next_double() * 2.0 - 1.0) * 1e6,
            _ => {
                let d = r.next_double() * 2.0 - 1.0;
                d * m::scalb(1.0, r.next_int_bound(2100) - 1075)
            }
        }
    }
    fn one(f: &str, x: f64, n: i32) -> f64 {
        match f {
            "tan" => m::tan(x),
            "asin" => m::asin(x),
            "acos" => m::acos(x),
            "atan" => m::atan(x),
            "exp" => m::exp(x),
            "log" => m::log(x),
            "log10" => m::log10(x),
            "sinh" => m::sinh(x),
            "cosh" => m::cosh(x),
            "tanh" => m::tanh(x),
            "sin" => m::sin(x),
            "cos" => m::cos(x),
            "expm1" => m::expm1(x),
            _ => m::scalb(x, n),
        }
    }
    let mut out = String::new();
    let mut r = Random::new(42);
    for f in [
        "tan", "asin", "acos", "atan", "exp", "log", "log10", "sinh", "cosh", "tanh", "sin", "cos",
        "expm1", "scalb",
    ] {
        let mut h: i64 = 0;
        for i in 0..204_000 {
            let x = input(&mut r, i);
            let n = r.next_int_bound(4300) - 2150;
            let b = bits(one(f, x, n));
            if i < 4000 {
                let _ = write!(out, "{f} {:x}", bits(x));
                if f == "scalb" {
                    let _ = write!(out, " {n}");
                }
                let _ = writeln!(out, " {b:x}");
            } else {
                h = h.wrapping_mul(31).wrapping_add(b as i64);
            }
        }
        let _ = writeln!(out, "{f} hash {:x}", h as u64);
    }
    for f in ["pow", "atan2"] {
        let mut h: i64 = 0;
        for i in 0..204_000 {
            let x = input(&mut r, i % 1600);
            let mut y = input(&mut r, i / 40 % 40 + if i >= 1600 { 40 } else { 0 });
            if f == "pow" && i % 3 == 0 {
                y = (y * 4.0).round_ties_even() / 4.0;
            }
            let b = bits(if f == "pow" {
                m::pow(x, y)
            } else {
                m::atan2(x, y)
            });
            if i < 4000 {
                let _ = writeln!(out, "{f} {:x} {:x} {b:x}", bits(x), bits(y));
            } else {
                h = h.wrapping_mul(31).wrapping_add(b as i64);
            }
        }
        let _ = writeln!(out, "{f} hash {:x}", h as u64);
    }
    Ok(out)
}

/// `tests/original_probe/LuaLoadEnvProbe.java` on the port: the game's own Lua loaded by the ported LuaManager.
/// LuaLoadEnvProbe (`boot` false) and LuaBootProbe (`boot` true) on the port.
pub fn luaload(
    root: &Path,
    cache_dir: &str,
    dirs: &[String],
    boot: bool,
) -> Result<String, String> {
    luaload_after(root, cache_dir, dirs, boot, None)
}

/// `SandboxOptions.instance.<name>.getValue()` for a boolean option, resolved from the Lua state the wg*
/// probes' shared setup leaves (the option at its loaded value). `None`: not a boolean option (or missing).
fn sandbox_option_bool(
    lm: &mut zomboid_engine::zombie::lua::lua_manager::LuaManager,
    name: &str,
) -> Option<bool> {
    use zomboid_engine::zombie::lua::exposed_methods::sandbox_options_instance;
    use zomboid_engine::zombie::sandbox_options::Value;
    let so = sandbox_options_instance(&lm.statics, &mut lm.thread).ok()?;
    let so = so.0.borrow();
    match so.get(name) {
        Some(Value::Boolean(b)) => Some(*b),
        _ => None,
    }
}

/// WgUtilsProbe on the port: the worldgen utility classes - the coordinate records, Direction, CacheMap,
/// AreaSquareVar, the enums and the Delaunay triangulation family - over the same fixed inputs, no game state.
pub fn wgutils() -> Result<String, String> {
    use std::fmt::Write as _;
    use std::rc::Rc;
    use zomboid_engine::zombie::iso::worldgen::enums::tile_lookup::TileLookup;
    use zomboid_engine::zombie::iso::worldgen::enums::tile_replacement::TileReplacement;
    use zomboid_engine::zombie::iso::worldgen::utils::area_square_var::AreaSquareVar;
    use zomboid_engine::zombie::iso::worldgen::utils::cache_map::CacheMap;
    use zomboid_engine::zombie::iso::worldgen::utils::cell_coord::CellCoord;
    use zomboid_engine::zombie::iso::worldgen::utils::chunk_coord::ChunkCoord;
    use zomboid_engine::zombie::iso::worldgen::utils::direction::Direction;
    use zomboid_engine::zombie::iso::worldgen::utils::square_coord::SquareCoord;
    use zomboid_engine::zombie::iso::worldgen::utils::triangulation::delaunay_triangulator::DelaunayTriangulator;
    use zomboid_engine::zombie::iso::worldgen::utils::triangulation::edge_distance_pack::EdgeDistancePack;
    use zomboid_engine::zombie::iso::worldgen::utils::triangulation::triangle2d::Triangle2D;
    use zomboid_engine::zombie::iso::worldgen::utils::triangulation::vector2d::Vector2D;

    fn esc(s: &str) -> String {
        let mut b = String::from("\"");
        for c in s.encode_utf16() {
            if (0x20..0x7f).contains(&c) && c != u16::from(b'"') && c != u16::from(b'\\') {
                b.push(char::from(c as u8));
            } else {
                let _ = write!(b, "\\u{c:04x}");
            }
        }
        b.push('"');
        b
    }
    fn d_hex(d: f64) -> String {
        format!("d{:016x}", d.to_bits())
    }
    fn pt(v: &Vector2D) -> String {
        format!("({} {})", d_hex(v.x), d_hex(v.y))
    }
    /// java.lang.Double.compare (NaN largest, -0.0 < +0.0): the probe's Comparator.comparingDouble.
    fn jcmp(a: f64, b: f64) -> std::cmp::Ordering {
        match (a.is_nan(), b.is_nan()) {
            (true, true) => std::cmp::Ordering::Equal,
            (true, false) => std::cmp::Ordering::Greater,
            (false, true) => std::cmp::Ordering::Less,
            (false, false) => {
                if a < b {
                    std::cmp::Ordering::Less
                } else if a > b {
                    std::cmp::Ordering::Greater
                } else if a == 0.0 {
                    match (a.is_sign_negative(), b.is_sign_negative()) {
                        (false, true) => std::cmp::Ordering::Greater,
                        (true, false) => std::cmp::Ordering::Less,
                        _ => std::cmp::Ordering::Equal,
                    }
                } else {
                    std::cmp::Ordering::Equal
                }
            }
        }
    }
    // one triangulation case: the sizes, then the triangles and edges sorted (the identity-hash set orders
    // behind them are not reproducible), exactly as the probe prints them
    let triangulation_case = |out: &mut String, id: i32, points: &[Rc<Vector2D>]| {
        let _ = writeln!(out, "DSET {id} PTS {}", points.len());
        let mut t = DelaunayTriangulator::new(points.to_vec());
        match t.triangulate() {
            Err(e) => {
                if points.len() < 3 {
                    // WgUtilsProbe prints getMessage() for the NotEnoughPointsException branch.
                    let message = e
                        .strip_prefix(
                            "zombie.iso.worldgen.utils.triangulation.NotEnoughPointsException: ",
                        )
                        .unwrap_or(&e);
                    let _ = writeln!(out, "DSET {id} NEE {}", esc(message));
                } else {
                    // WgUtilsProbe prints Exception.toString() for triangulation failures.
                    let _ = writeln!(out, "DSET {id} ERR {}", esc(&e));
                }
                return;
            }
            Ok(()) => {}
        }
        let mut tris: Vec<&Rc<Triangle2D>> = t.get_triangles().iter().collect();
        tris.sort_by(|a, b| {
            jcmp(a.a.x, b.a.x)
                .then_with(|| jcmp(a.a.y, b.a.y))
                .then_with(|| jcmp(a.b.x, b.b.x))
                .then_with(|| jcmp(a.b.y, b.b.y))
                .then_with(|| jcmp(a.c.x, b.c.x))
                .then_with(|| jcmp(a.c.y, b.c.y))
        });
        let _ = writeln!(out, "DSET {id} TRI {}", tris.len());
        for tr in &tris {
            let _ = writeln!(out, "DTRI {id} {} {} {}", pt(&tr.a), pt(&tr.b), pt(&tr.c));
        }
        let mut edges = t.get_edges();
        edges.sort_by(|a, b| {
            jcmp(a.a.x, b.a.x)
                .then_with(|| jcmp(a.a.y, b.a.y))
                .then_with(|| jcmp(a.b.x, b.b.x))
                .then_with(|| jcmp(a.b.y, b.b.y))
        });
        let _ = writeln!(out, "DSET {id} EDGE {}", edges.len());
        for e in &edges {
            let _ = writeln!(out, "DEDGE {id} {} {} {}", pt(&e.a), pt(&e.b), d_hex(e.mag_sqrt()));
        }
        if id == 1 {
            let mut t2 = DelaunayTriangulator::new(points.to_vec());
            t2.shuffle_permutation(&[3, 1, 4, 0, 2, 6, 5]);
            match t2.triangulate() {
                Err(e) => {
                    let _ = writeln!(out, "DSHUF {id} ERR {}", esc(&e));
                }
                Ok(()) => {
                    let mut edges2 = t2.get_edges();
                    edges2.sort_by(|a, b| {
                        jcmp(a.a.x, b.a.x)
                            .then_with(|| jcmp(a.a.y, b.a.y))
                            .then_with(|| jcmp(a.b.x, b.b.x))
                            .then_with(|| jcmp(a.b.y, b.b.y))
                    });
                    let _ = writeln!(out, "DSHUF {id} EDGE {}", edges2.len());
                    for e in &edges2 {
                        let _ = writeln!(
                            out,
                            "DEDGE {id} {} {} {}",
                            pt(&e.a),
                            pt(&e.b),
                            d_hex(e.mag_sqrt())
                        );
                    }
                }
            }
        }
    };

    let mut out = String::new();
    // the record hash codes
    let _ = writeln!(out, "HASH start");
    let coords: [(i32, i32); 7] = [
        (0, 0),
        (1, 0),
        (0, 1),
        (-1, -1),
        (255, 255),
        (123456, -654321),
        (i32::MAX, i32::MIN),
    ];
    for (x, y) in coords {
        let _ = writeln!(
            out,
            "HASH CC {x} {y} {} {}",
            CellCoord::new(x, y).java_hash_code(),
            ChunkCoord::new(x, y).java_hash_code()
        );
    }
    for (x, y) in coords {
        let _ = writeln!(
            out,
            "HASH SC {x} {y} {}",
            SquareCoord::new(x, y, (x + y) & 3).java_hash_code()
        );
    }

    // SquareCoord save/load roundtrip over a 64-byte buffer
    {
        use zomboid_engine::java_nio::{ByteBuffer, ByteBufferWriter};
        let sc = SquareCoord::new(17, -34, 8);
        let mut w = ByteBufferWriter::new();
        sc.save(&mut w);
        let written = w.bytes().to_vec();
        let mut bb = ByteBuffer::wrap(&written);
        let sc2 = SquareCoord::load(&mut bb).map_err(|e| format!("SQSAVE: {e}"))?;
        // Java: allocate(64), 12 written, flip (limit 12), 12 read: remaining() = 0
        let _ = writeln!(
            out,
            "SQSAVE {} {} {} eq={} rem={}",
            sc2.x,
            sc2.y,
            sc2.z,
            sc == sc2,
            0
        );
    }

    // Direction
    let _ = writeln!(out, "DIR start");
    for dir in Direction::ALL {
        let _ = writeln!(
            out,
            "DIR {} {} {} {} {} {} {}",
            dir.index(),
            dir.x(),
            dir.y(),
            dir.name(),
            dir.opposite().name(),
            dir.prev().name(),
            dir.next().name()
        );
    }
    // WgUtilsProbe leaves its StringBuilders without a closing bracket.
    let names = |dirs: &[Direction]| -> String {
        format!(
            "[{}",
            dirs.iter().map(|d| d.name()).collect::<Vec<_>>().join(",")
        )
    };
    let _ = writeln!(
        out,
        "DIR cardinals={} diagonals={} rose={}",
        names(Direction::cardinals()),
        names(Direction::diagonals()),
        names(Direction::rose())
    );

    // CacheMap(3): five puts, then gets
    {
        let mut cm: CacheMap<String, i32> = CacheMap::new(3);
        for i in 1..=5 {
            cm.put(format!("k{i}"), i);
            let _ = writeln!(out, "CACHE put k{i} size={}", cm.len());
        }
        let _ = writeln!(
            out,
            "CACHE get k1={} k5={} has2={}",
            cm.get(&String::from("k1")).map(|v| v.to_string()).unwrap_or("null".into()),
            cm.get(&String::from("k5")).map(|v| v.to_string()).unwrap_or("null".into()),
            cm.contains_key(&String::from("k2"))
        );
    }

    // AreaSquareVar
    {
        let mut asv = AreaSquareVar::new();
        let _ = writeln!(
            out,
            "ASV defaults {} {} {} {}",
            asv.get_xmin(),
            asv.get_xmax(),
            asv.get_ymin(),
            asv.get_ymax()
        );
        asv.set_xmin(-5);
        asv.set_xmax(30);
        asv.set_ymin(-9);
        asv.set_ymax(41);
        let _ = writeln!(out, "ASV set {}", asv.to_string());
    }

    // the enums
    let _ = writeln!(
        out,
        "ENUM TileLookup [{}",
        TileLookup::ALL.iter().map(|v| v.name()).collect::<Vec<_>>().join(",")
    );
    let _ = writeln!(
        out,
        "ENUM TileReplacement [{}",
        TileReplacement::ALL
            .iter()
            .map(|v| v.name())
            .collect::<Vec<_>>()
            .join(",")
    );

    // Vector2D algebra
    {
        let a = Vector2D::new(3.5, -2.25);
        let b = Vector2D::new(-1.0, 7.75);
        let _ = writeln!(
            out,
            "V {} {} sub {} add {} mult {}",
            d_hex(a.x),
            d_hex(a.y),
            pt(&a.sub(&b)),
            pt(&a.add(&b)),
            pt(&a.mult(2.5))
        );
        let _ = writeln!(
            out,
            "V mag {} magSqrt {} dot {} cross {}",
            d_hex(a.mag()),
            d_hex(a.mag_sqrt()),
            d_hex(a.dot(&b)),
            d_hex(a.cross(&b))
        );
        let z = Vector2D::new(0.0, -0.0);
        let _ = writeln!(out, "V zero mag {} magSqrt {}", d_hex(z.mag()), d_hex(z.mag_sqrt()));
        let _ = writeln!(
            out,
            "VSTR {} {} {}",
            esc(&a.to_string()),
            esc(&Vector2D::new(1.0, 2.0).to_string()),
            esc(&Vector2D::new(-0.5, 1.0e-3).to_string())
        );
    }

    // EdgeDistancePack.compareTo
    {
        let ds = [1.0, -0.0, 0.0, f64::NAN, f64::INFINITY, -1.0];
        for (i, di) in ds.iter().enumerate() {
            for (j, dj) in ds.iter().enumerate() {
                let p1 = EdgeDistancePack::new_null(*di);
                let p2 = EdgeDistancePack::new_null(*dj);
                let _ = writeln!(
                    out,
                    "EDP {i} {j} {}",
                    match p1.compare_to(&p2) {
                        std::cmp::Ordering::Less => -1,
                        std::cmp::Ordering::Equal => 0,
                        std::cmp::Ordering::Greater => 1,
                    }
                );
            }
        }
    }

    // the triangulation cases
    {
        let set1: Vec<Rc<Vector2D>> = [
            (0.0, 0.0),
            (10.0, 0.0),
            (0.0, 10.0),
            (10.0, 10.0),
            (5.0, 5.0),
            (5.0, 0.5),
            (2.5, 7.5),
        ]
        .iter()
        .map(|&(x, y)| Rc::new(Vector2D::new(x, y)))
        .collect();
        triangulation_case(&mut out, 1, &set1);

        let mut rnd = zomboid_engine::java::Random::new(42);
        let set2: Vec<Rc<Vector2D>> = (0..12)
            .map(|_| {
                Rc::new(Vector2D::new(
                    rnd.next_double() * 100.0,
                    rnd.next_double() * 100.0,
                ))
            })
            .collect();
        triangulation_case(&mut out, 2, &set2);

        let set3: Vec<Rc<Vector2D>> = (0..5)
            .flat_map(|x| (0..5).map(move |y| (x, y)))
            .map(|(x, y)| Rc::new(Vector2D::new(f64::from(x) * 20.0, f64::from(y) * 20.0)))
            .collect();
        triangulation_case(&mut out, 3, &set3);

        let set4: Vec<Rc<Vector2D>> = [(1.0, 1.0), (1.0, 1.0), (9.0, 1.0), (5.0, 9.0), (-3.0, 4.0)]
            .iter()
            .map(|&(x, y)| Rc::new(Vector2D::new(x, y)))
            .collect();
        triangulation_case(&mut out, 4, &set4);

        let set5: Vec<Rc<Vector2D>> = [(1.0, 1.0), (2.0, 2.0)]
            .iter()
            .map(|&(x, y)| Rc::new(Vector2D::new(x, y)))
            .collect();
        triangulation_case(&mut out, 5, &set5);
    }

    Ok(out)
}

/// WgConfigsProbe on the port: the WorldGenReader loaders wgbiomes left out - loadStaticModules (with the
/// inline loadBiome and loadPrefab), loadVeinsConfig, loadRoadConfig, loadAnimalsPath, loadBiomeMapConfig -
/// and the zombie_voronoi entries, over the same Lua state as wgbiomes.
pub fn wgconfigs(root: &std::path::Path, cache_dir: &str) -> Result<String, String> {
    use std::fmt::Write as _;
    use std::rc::Rc;
    use zomboid_engine::se::krka::kahlua::j2se::kahlua_table_impl::KahluaTableImpl;
    use zomboid_engine::se::krka::kahlua::vm::object::Object;
    use zomboid_engine::zombie::game_window::ClientStartup;
    use zomboid_engine::zombie::iso::worldgen::maps::biome_map_entry::BiomeMapEntry;
    use zomboid_engine::zombie::iso::worldgen::world_gen_reader::WorldGenReader;
    use zomboid_engine::zombie::lua::exposed_game;
    use zomboid_engine::zombie::lua::exposed_methods::sandbox_options_instance;
    use zomboid_engine::zombie::lua::lua_manager::LuaManager;

    fn esc(s: &str) -> String {
        let mut b = String::from("\"");
        for c in s.encode_utf16() {
            if (0x20..0x7f).contains(&c) && c != u16::from(b'"') && c != u16::from(b'\\') {
                b.push(char::from(c as u8));
            } else {
                let _ = write!(b, "\\u{c:04x}");
            }
        }
        b.push('"');
        b
    }
    fn d_hex(d: f64) -> String {
        format!("d{:016x}", d.to_bits())
    }
    fn f_hex(f: f32) -> String {
        format!("f{:08x}", f.to_bits())
    }
    fn names(list: &[String]) -> String {
        format!(
            "[{}]",
            list.iter().map(|s| esc(s)).collect::<Vec<_>>().join(",")
        )
    }
    fn tiles(list: &[zomboid_engine::zombie::iso::worldgen::biomes::tile_group::TileGroup]) -> String {
        let mut b = String::from("[");
        for (i, g) in list.iter().enumerate() {
            if i > 0 {
                b.push(',');
            }
            let _ = write!(b, "{}x{} {}", g.sx, g.sy, names(&g.tiles));
        }
        b.push(']');
        b
    }
    fn ints(list: &[i32]) -> String {
        format!("[{}]", list.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(","))
    }
    fn rows(rows: &[Vec<i32>]) -> String {
        format!(
            "[{}]",
            rows.iter().map(|r| ints(r)).collect::<Vec<_>>().join(",")
        )
    }
    fn rawget_is_nil(table: &zomboid_engine::se::krka::kahlua::vm::object::TableRef, key: &str) -> Result<bool, String> {
        Ok(KahluaTableImpl::rawget(table, &Object::str(key))
            .map_err(|e| format!("{e:?}"))?
            .is_nil())
    }
    fn env_table<'a>(
        lm: &'a LuaManager,
        key: &str,
    ) -> Option<zomboid_engine::se::krka::kahlua::vm::object::TableRef> {
        match KahluaTableImpl::rawget(&lm.env, &Object::str(key)) {
            Ok(Object::Table(t)) => Some(t),
            _ => None,
        }
    }
    /// The KEYS line's check: the original prints "table" for any non-null rawget (it only checks `== null`).
    fn env_is_nil(lm: &LuaManager, key: &str) -> Result<bool, String> {
        Ok(KahluaTableImpl::rawget(&lm.env, &Object::str(key))
            .map_err(|e| format!("{e:?}"))?
            .is_nil())
    }

    let startup = ClientStartup::init(root).map_err(|e| format!("startup: {e}"))?;
    let mut lm =
        LuaManager::init_after(root, Some(Rc::new(startup))).map_err(|e| format!("init: {e:?}"))?;
    *lm.statics.cache_dir.borrow_mut() = Some(cache_dir.to_string());
    exposed_game::init_shared(&lm.statics).map_err(|e| format!("initShared: {e}"))?;
    for dir in ["shared", "server"] {
        lm.load_dir_base(dir)
            .map_err(|e| format!("LoadDirBase {dir}: {e:?}"))?;
    }
    let worldgen = match KahluaTableImpl::rawget(&lm.env, &Object::str("worldgen"))
        .map_err(|e| format!("worldgen: {e:?}"))?
    {
        Object::Table(t) => t,
        _ => return Err("worldgen table missing from the Lua environment".into()),
    };
    let so = sandbox_options_instance(&lm.statics, &mut lm.thread)
        .map_err(|e| format!("SandboxOptions.instance: {e:?}"))?;
    let so_borrowed = so.0.borrow();
    let reader = WorldGenReader {
        sandbox: &so_borrowed,
    };
    let mut out = String::new();

    let key = |nil: bool| -> &'static str {
        if nil {
            "nil"
        } else {
            "table"
        }
    };
    let _ = writeln!(
        out,
        "KEYS static_modules={} veins={} roads={} animals={} biome_map={} zombie_voronoi={}",
        key(rawget_is_nil(&worldgen, "static_modules")?),
        key(rawget_is_nil(&worldgen, "veins")?),
        key(rawget_is_nil(&worldgen, "roads")?),
        key(env_is_nil(&lm, "animals_path_config")?),
        key(env_is_nil(&lm, "biome_map_config")?),
        key(env_is_nil(&lm, "zombie_voronoi")?)
    );

    // static modules
    let modules = reader
        .load_static_modules(&worldgen, "static_modules")
        .map_err(|e| format!("loadStaticModules: {e:?}"))?;
    let _ = writeln!(out, "STATIC {}", modules.len());
    for (i, m) in modules.iter().enumerate() {
        let _ = writeln!(out, "SM {} {} {} {} {}", i + 1, m.xmin, m.xmax, m.ymin, m.ymax);
        match &m.biome {
            None => {
                let _ = writeln!(out, "SM {} BIOME nil", i + 1);
            }
            Some(b) => {
                let _ = writeln!(
                    out,
                    "SM {} BIOME parent={} gen={} zombies={}",
                    i + 1,
                    match &b.parent {
                        Some(p) => esc(p),
                        None => "nil".to_string(),
                    },
                    i32::from(b.generate),
                    f_hex(b.zombies)
                );
            }
        }
        match &m.prefab {
            None => {
                let _ = writeln!(out, "SM {} PREFAB nil", i + 1);
            }
            Some(p) => {
                let _ = writeln!(
                    out,
                    "SM {} PREFAB {}x{} tiles={} zombies={} cats=[{}]",
                    i + 1,
                    p.get_x(),
                    p.get_y(),
                    p.tiles_len(),
                    f_hex(p.get_zombies()),
                    p.get_categories().join(", ")
                );
                for (j, tile) in p.tiles_iter().enumerate() {
                    let _ = writeln!(out, "PT {} {} {}", i + 1, j + 1, esc(tile));
                }
                for (category, schematic_rows) in p.schematic_iter() {
                    let _ = writeln!(out, "PF {} {} {}", i + 1, esc(category), rows(schematic_rows));
                }
            }
        }
    }

    // veins
    let veins = reader
        .load_veins_config(&worldgen, "veins")
        .map_err(|e| format!("loadVeinsConfig: {e:?}"))?;
    let _ = writeln!(out, "VEINS {}", veins.len());
    for (name, c) in veins.iter() {
        let _ = writeln!(
            out,
            "V {} cr={} cp={} amin={} amax={} dmin={} dmax={} delta={} ap={} p={}",
            esc(name),
            c.center_radius,
            f_hex(c.center_prob),
            c.arms_amount_min,
            c.arms_amount_max,
            c.arms_dist_min,
            c.arms_dist_max,
            c.arms_delta_angle,
            f_hex(c.arms_prob),
            f_hex(c.probability)
        );
        let _ = writeln!(out, "VT {} {}", esc(name), tiles(&c.tiles));
    }

    // roads
    let roads = reader
        .load_road_config(&worldgen, "roads")
        .map_err(|e| format!("loadRoadConfig: {e:?}"))?;
    let _ = writeln!(out, "ROADS {}", roads.len());
    for (name, c) in roads.iter() {
        let _ = writeln!(
            out,
            "R {} proba={} p={} filter={}",
            esc(name),
            d_hex(c.proba_roads),
            d_hex(c.probability),
            d_hex(c.filter)
        );
        let _ = writeln!(out, "RT {} {}", esc(name), tiles(&c.tiles));
    }

    // animals paths
    match env_table(&lm, "animals_path_config") {
        None => {
            let _ = writeln!(out, "ANIMALS nil");
        }
        Some(table) => {
            let animals = reader
                .load_animals_path(&table)
                .map_err(|e| format!("loadAnimalsPath: {e:?}"))?;
            let _ = writeln!(out, "ANIMALS {}", animals.len());
            for (i, c) in animals.iter().enumerate() {
                let _ = writeln!(
                    out,
                    "A {} {} count={} chance={} points={} radius={} extension={} extchance={} hash={}",
                    i + 1,
                    esc(&c.animal_type),
                    c.count,
                    f_hex(c.chance),
                    ints(&c.points),
                    ints(&c.radius),
                    ints(&c.extension),
                    f_hex(c.extension_chance),
                    c.get_name_hash()
                );
            }
        }
    }

    // biome map
    match env_table(&lm, "biome_map_config") {
        None => {
            let _ = writeln!(out, "BIOME_MAP nil");
        }
        Some(table) => {
            let biome_map: zomboid_engine::java::JavaIntHashMap<BiomeMapEntry> = reader
                .load_biome_map_config(&table)
                .map_err(|e| format!("loadBiomeMapConfig: {e:?}"))?;
            let _ = writeln!(out, "BIOME_MAP {}", biome_map.len());
            for (pixel, m) in biome_map.iter() {
                let _ = writeln!(
                    out,
                    "M {} {} {} {}",
                    pixel,
                    m.biome.as_deref().map(esc).unwrap_or("nil".into()),
                    m.ore.as_deref().map(esc).unwrap_or("nil".into()),
                    m.zone.as_deref().map(esc).unwrap_or("nil".into())
                );
            }
        }
    }

    // zombie voronoi
    match env_table(&lm, "zombie_voronoi") {
        None => {
            let _ = writeln!(out, "ZVORONOI nil");
        }
        Some(table) => {
            let entries = zomboid_engine::zombie::iso::worldgen::zombie::zombie_voronoi_reader::get_entries(&table)
                .map_err(|e| format!("getEntries: {e:?}"))?;
            let _ = writeln!(out, "ZVORONOI {}", entries.len());
            for (i, e) in entries.iter().enumerate() {
                let _ = writeln!(
                    out,
                    "Z {} points={} closest={} scale={} cutoff={}",
                    i + 1,
                    e.number_points,
                    e.closest_point.name(),
                    d_hex(e.scale),
                    d_hex(e.cutoff)
                );
            }
        }
    }

    Ok(out)
}

/// WgVoronoiProbe on the port: the zombie-density Voronoi noise - getVoronois over the game's own
/// `zombie_voronoi` table, evaluateCellNoise / evaluateCellCutoff for the fixed seeds and cells, the cache,
/// releaseCell and the HashUtil values, over the wgbiomes Lua state.
pub fn wgvoronoi(root: &std::path::Path, cache_dir: &str) -> Result<String, String> {
    use std::fmt::Write as _;
    use std::rc::Rc;
    use zomboid_engine::se::krka::kahlua::j2se::kahlua_table_impl::KahluaTableImpl;
    use zomboid_engine::se::krka::kahlua::vm::object::Object;
    use zomboid_engine::zombie::game_window::ClientStartup;
    use zomboid_engine::zombie::iso::worldgen::zombie::hash_util::hash2d;
    use zomboid_engine::zombie::iso::worldgen::zombie::zombie_voronoi::ZombieVoronoi;
    use zomboid_engine::zombie::iso::worldgen::zombie::zombie_voronoi_reader::get_entries;
    use zomboid_engine::zombie::lua::exposed_game;
    use zomboid_engine::zombie::lua::lua_manager::LuaManager;

    fn d_hex(d: f64) -> String {
        format!("d{:016x}", d.to_bits())
    }
    fn row(noise: &[f64], y: usize) -> String {
        let mut b = String::new();
        for x in 0..32 {
            if x > 0 {
                b.push(',');
            }
            let _ = write!(b, "{}", d_hex(noise[x + y * 32]));
        }
        b
    }

    const SEEDS: [i64; 6] = [1, 42, -1, 2147483647, i64::MIN, 0x1234567890ABCDEF];
    const ROWS: [usize; 4] = [0, 10, 21, 31];
    const CELLS: [(i32, i32); 4] = [(0, 0), (1, -1), (31, 32), (-5, 7)];

    let startup = ClientStartup::init(root).map_err(|e| format!("startup: {e}"))?;
    let mut lm =
        LuaManager::init_after(root, Some(Rc::new(startup))).map_err(|e| format!("init: {e:?}"))?;
    *lm.statics.cache_dir.borrow_mut() = Some(cache_dir.to_string());
    exposed_game::init_shared(&lm.statics).map_err(|e| format!("initShared: {e}"))?;
    for dir in ["shared", "server"] {
        lm.load_dir_base(dir)
            .map_err(|e| format!("LoadDirBase {dir}: {e:?}"))?;
    }
    let table = match KahluaTableImpl::rawget(&lm.env, &Object::str("zombie_voronoi"))
        .map_err(|e| format!("zombie_voronoi: {e:?}"))?
    {
        Object::Table(t) => t,
        _ => return Err("zombie_voronoi table missing from the Lua environment".into()),
    };
    // SandboxOptions.instance.zombieVoronoiNoise.getValue() (the option exists in every table; its default
    // is true)
    let noise_option = match sandbox_option_bool(&mut lm, "ZombieVoronoiNoise") {
        Some(b) => b,
        None => true,
    };
    let entries =
        get_entries(&table).map_err(|e| format!("getEntries: {e:?}"))?;
    let mut out = String::new();
    let _ = writeln!(out, "NOISE {}", i32::from(noise_option));
    let _ = writeln!(
        out,
        "ENTRIES {}",
        ZombieVoronoi::get_voronois(0, &table)
            .map_err(|e| format!("getVoronois: {e:?}"))?
            .len()
    );

    for seed in SEEDS {
        let mut voronois = ZombieVoronoi::get_voronois(seed, &table)
            .map_err(|e| format!("getVoronois: {e:?}"))?;
        for (i, v) in voronois.iter_mut().enumerate() {
            let e = &entries[i];
            let _ = writeln!(
                out,
                "ZV {seed} {i} points={} closest={} scale={} cutoff={}",
                e.number_points,
                e.closest_point.name(),
                d_hex(e.scale),
                d_hex(e.cutoff)
            );
            for (cx, cy) in CELLS {
                // the first evaluation is kept as an owned copy - the later calls (releaseCell
                // included) need v mutably again, and the Java probe only reads its values
                let first: Vec<f64> = v
                    .evaluate_cell_noise(cx, cy)
                    .map_err(|e| format!("evaluateCellNoise: {e:?}"))?
                    .to_vec();
                let _ = v
                    .evaluate_cell_noise(cx, cy)
                    .map_err(|e| format!("evaluateCellNoise: {e:?}"))?;
                let _ = writeln!(out, "CELL {seed} {i} {cx} {cy} cached=1");
                for y in ROWS {
                    let _ = writeln!(out, "CN {seed} {i} {cx} {cy} {y} {}", row(&first, y));
                }
                let cutoff = v
                    .evaluate_cell_cutoff(cx, cy, noise_option)
                    .map_err(|e| format!("evaluateCellCutoff: {e:?}"))?;
                let _ = writeln!(
                    out,
                    "CUTOFF {seed} {i} {cx} {cy} 10 {}",
                    row(&cutoff, 10)
                );
                let cutoff_x = v
                    .evaluate_cell_cutoff_with(cx, cy, 0.15)
                    .map_err(|e| format!("evaluateCellCutoff: {e:?}"))?;
                let _ = writeln!(
                    out,
                    "CUTOFFX {seed} {i} {cx} {cy} c={} {}",
                    d_hex(0.15),
                    row(&cutoff_x, 10)
                );
                v.release_cell(cx, cy);
                let third = v
                    .evaluate_cell_noise(cx, cy)
                    .map_err(|e| format!("evaluateCellNoise: {e:?}"))?;
                // released-eq is the Java probe's Arrays.equals(first, third) - content equality
                let _ = writeln!(
                    out,
                    "CELL {seed} {i} {cx} {cy} released-eq={}",
                    i32::from(first[..] == *third)
                );
            }
        }
    }

    // the section hash, directly
    let _ = writeln!(out, "H2D start");
    let hseeds: [i64; 4] = [0, 1, -1, 1234567890123];
    let hxs: [i64; 6] = [0, 1, -1, 255, 1000000, -999999999];
    for hs in hseeds {
        for hx in hxs {
            let _ = writeln!(
                out,
                "H2D {hs} {hx} {} {}",
                hx.wrapping_mul(2),
                hash2d(hs, hx, hx.wrapping_mul(2))
            );
        }
    }
    for hx in hxs {
        let _ = writeln!(
            out,
            "H2D {} {hx} {} {}",
            i64::MIN,
            hx.wrapping_sub(7),
            hash2d(i64::MIN, hx, hx.wrapping_sub(7))
        );
    }

    Ok(out)
}

/// WgVeinsProbe on the port: the ore veins - Veins.get over the game's own `worldgen.veins` config with
/// WorldGenParams seeds set through setSeedString, the getRandom chain, OreVein's construction draws and
/// isValid, over the wgbiomes Lua state.
pub fn wgveins(root: &std::path::Path, cache_dir: &str) -> Result<String, String> {
    use std::fmt::Write as _;
    use std::rc::Rc;
    use zomboid_engine::java::Random;
    use zomboid_engine::se::krka::kahlua::j2se::kahlua_table_impl::KahluaTableImpl;
    use zomboid_engine::se::krka::kahlua::vm::object::Object;
    use zomboid_engine::zombie::game_window::ClientStartup;
    use zomboid_engine::zombie::iso::worldgen::veins::veins::Veins;
    use zomboid_engine::zombie::iso::worldgen::world_gen_params::WorldGenParams;
    use zomboid_engine::zombie::iso::worldgen::world_gen_reader::WorldGenReader;
    use zomboid_engine::zombie::lua::exposed_game;
    use zomboid_engine::zombie::lua::exposed_methods::sandbox_options_instance;
    use zomboid_engine::zombie::lua::lua_manager::LuaManager;

    fn esc(s: &str) -> String {
        let mut b = String::from("\"");
        for c in s.encode_utf16() {
            if (0x20..0x7f).contains(&c) && c != u16::from(b'"') && c != u16::from(b'\\') {
                b.push(char::from(c as u8));
            } else {
                let _ = write!(b, "\\u{c:04x}");
            }
        }
        b.push('"');
        b
    }
    fn f_hex(f: f32) -> String {
        format!("f{:08x}", f.to_bits())
    }
    fn cfg_tiles(list: &[zomboid_engine::zombie::iso::worldgen::biomes::tile_group::TileGroup]) -> String {
        let mut b = String::from("[");
        for (i, g) in list.iter().enumerate() {
            if i > 0 {
                b.push(',');
            }
            let _ = write!(b, "{}x{} {}", g.sx, g.sy, g.tiles.len());
        }
        b.push(']');
        b
    }

    const SEEDS: [&str; 3] = ["1", "42", "oracle"];
    const CELLS: [(i32, i32); 5] = [(0, 0), (0, 1), (1, 0), (-1, -1), (37, -11)];

    let startup = ClientStartup::init(root).map_err(|e| format!("startup: {e}"))?;
    let mut lm =
        LuaManager::init_after(root, Some(Rc::new(startup))).map_err(|e| format!("init: {e:?}"))?;
    *lm.statics.cache_dir.borrow_mut() = Some(cache_dir.to_string());
    exposed_game::init_shared(&lm.statics).map_err(|e| format!("initShared: {e}"))?;
    for dir in ["shared", "server"] {
        lm.load_dir_base(dir)
            .map_err(|e| format!("LoadDirBase {dir}: {e:?}"))?;
    }
    let worldgen = match KahluaTableImpl::rawget(&lm.env, &Object::str("worldgen"))
        .map_err(|e| format!("worldgen: {e:?}"))?
    {
        Object::Table(t) => t,
        _ => return Err("worldgen table missing from the Lua environment".into()),
    };
    let so = sandbox_options_instance(&lm.statics, &mut lm.thread)
        .map_err(|e| format!("SandboxOptions.instance: {e:?}"))?;
    let so_borrowed = so.0.borrow();
    let reader = WorldGenReader {
        sandbox: &so_borrowed,
    };
    let config = reader
        .load_veins_config(&worldgen, "veins")
        .map_err(|e| format!("loadVeinsConfig: {e:?}"))?;
    let mut out = String::new();
    let _ = writeln!(out, "CFG {}", config.len());
    for (name, c) in config.iter() {
        let _ = writeln!(
            out,
            "C {} cr={} cp={} amin={} amax={} dmin={} dmax={} delta={} ap={} p={} tiles={}",
            esc(name),
            c.center_radius,
            f_hex(c.center_prob),
            c.arms_amount_min,
            c.arms_amount_max,
            c.arms_dist_min,
            c.arms_dist_max,
            c.arms_delta_angle,
            f_hex(c.arms_prob),
            f_hex(c.probability),
            cfg_tiles(&c.tiles)
        );
    }

    // the getRandom chain, independent of the seeds below
    {
        let mut params = WorldGenParams::default();
        params.set_seed_string("chain");
        for (wx, wy) in [(3, -7), (0, 0), (-100, 100)] {
            for off in [0i64, 1000, -1] {
                let mut rnd = params.get_random_with_offset(wx, wy, off);
                let v1 = rnd.next_int_bound(100);
                let v2 = rnd.next_float();
                let v3 = rnd.next_int_bound(256);
                let v4 = rnd.next_long();
                let v5 = rnd.next_float();
                let v6 = rnd.next_int_bound(100);
                let _ = writeln!(
                    out,
                    "RND {wx} {wy} {off} {v1} {} {v3} {v4} {} {v6}",
                    f_hex(v2),
                    f_hex(v5)
                );
            }
        }
    }

    for seed in SEEDS {
        let mut params = WorldGenParams::default();
        params.set_seed_string(seed);
        let _ = writeln!(
            out,
            "SEED s={} hash={} seed={}",
            esc(seed),
            zomboid_engine::java::string_hash_code(seed),
            params.get_seed()
        );
        let mut veins = Veins::new(config.clone());
        for (cx, cy) in CELLS {
            let list = veins.get(cx, cy, &params);
            let _ = writeln!(out, "CELL {} {cx} {cy} {}", esc(seed), list.len());
            for (i, v) in list.iter().enumerate() {
                let _ = writeln!(
                    out,
                    "OV {} {cx} {cy} {i} {} {} {} {} sp={},{} amount={} cp={} ap={} cr={}",
                    esc(seed),
                    v.center_cell_x,
                    v.center_cell_y,
                    v.center_square_x,
                    v.center_square_y,
                    f_hex(v.start_point.x),
                    f_hex(v.start_point.y),
                    v.arms_amount,
                    f_hex(v.center_prob),
                    f_hex(v.arms_prob),
                    v.center_radius
                );
                for (j, _) in v.arms_orientation.iter().enumerate() {
                    let _ = writeln!(
                        out,
                        "ARM {} {cx} {cy} {i} {j} {} {} {} {}",
                        esc(seed),
                        f_hex(v.arms_orientation[j]),
                        f_hex(v.arms_distance[j]),
                        f_hex(v.end_points[j].x),
                        f_hex(v.end_points[j].y)
                    );
                }
                let _ = writeln!(
                    out,
                    "OVT {} {cx} {cy} {i} {}",
                    esc(seed),
                    cfg_tiles(&v.tile_groups)
                );
                let mut bits = String::new();
                for y in 0..8 {
                    for x in 0..8 {
                        let mut prnd = Random::new(1000 + (x + y * 8) as i64);
                        bits.push(if v.is_valid(cx * 256 + x * 32, cy * 256 + y * 32, &mut prnd) {
                            '1'
                        } else {
                            '0'
                        });
                    }
                }
                let _ = writeln!(out, "VAL {} {cx} {cy} {i} {bits}", esc(seed));
            }
            // the second get serves the cached list (the port returns the stored Vec by borrow)
            let _ = veins.get(cx, cy, &params);
            let _ = writeln!(out, "CACHE {} {cx} {cy} same=1", esc(seed));
        }
    }

    Ok(out)
}

/// WgRoadsProbe on the port: the road network generator - RoadGenerator over the game's own `worldgen.roads`
/// config, the meta grid bounds of `IsoWorld::init` and WorldGenParams seeds set through setSeedString, with
/// getRoads over the fixed sample chunks. The Lua state is wgbiomes' (the dedicated server the probe runs in
/// loads the same media/lua/server/WorldGen/*.lua).
pub fn wgroads(root: &std::path::Path) -> Result<String, String> {
    use std::fmt::Write as _;
    use std::rc::Rc;
    use zomboid_engine::se::krka::kahlua::j2se::kahlua_table_impl::KahluaTableImpl;
    use zomboid_engine::se::krka::kahlua::vm::object::Object;
    use zomboid_engine::zombie::game_window::ClientStartup;
    use zomboid_engine::zombie::iso::worldgen::roads::road_generator::RoadGenerator;
    use zomboid_engine::zombie::iso::worldgen::world_gen_params::WorldGenParams;
    use zomboid_engine::zombie::iso::worldgen::world_gen_reader::WorldGenReader;
    use zomboid_engine::zombie::lua::exposed_game;
    use zomboid_engine::zombie::lua::exposed_methods::sandbox_options_instance;
    use zomboid_engine::zombie::lua::lua_manager::LuaManager;
    // IsoWorld, AlarmSettings and RandAbstract come from the file's top-level imports.

    fn esc(s: &str) -> String {
        let mut b = String::from("\"");
        for c in s.encode_utf16() {
            if (0x20..0x7f).contains(&c) && c != u16::from(b'"') && c != u16::from(b'\\') {
                b.push(char::from(c as u8));
            } else {
                let _ = write!(b, "\\u{c:04x}");
            }
        }
        b.push('"');
        b
    }
    fn d_hex(d: f64) -> String {
        format!("d{:016x}", d.to_bits())
    }
    fn pt(x: f64, y: f64) -> String {
        format!("({} {})", d_hex(x), d_hex(y))
    }
    /// java.lang.Double.compare: the probe's Comparator.comparingDouble.
    fn jcmp(a: f64, b: f64) -> std::cmp::Ordering {
        match (a.is_nan(), b.is_nan()) {
            (true, true) => std::cmp::Ordering::Equal,
            (true, false) => std::cmp::Ordering::Greater,
            (false, true) => std::cmp::Ordering::Less,
            (false, false) => a.partial_cmp(&b).unwrap_or({
                if a == 0.0 && b == 0.0 && a.is_sign_negative() != b.is_sign_negative() {
                    if a.is_sign_negative() {
                        std::cmp::Ordering::Less
                    } else {
                        std::cmp::Ordering::Greater
                    }
                } else {
                    std::cmp::Ordering::Equal
                }
            }),
        }
    }

    // the meta grid of the real map, the way a dedicated server builds it (top-level imports for
    // IsoWorld/AlarmSettings/RandAbstract)
    let mut rand = RandAbstract::new(zomboid_engine::java::Random::new(12345));
    let alarm = AlarmSettings {
        last_stand: false,
        power_grid_exists: true,
        alarm: 1,
        alarm_decay: 1,
    };
    let world = IsoWorld::init(root, "Muldraugh, KY", &|_| false, &alarm, None, &mut rand)
        .map_err(|e| format!("IsoWorld.init: {e}"))?;
    let (meta_min_x, meta_max_x, meta_min_y, meta_max_y) = (
        world.meta_grid.min_x,
        world.meta_grid.max_x,
        world.meta_grid.min_y,
        world.meta_grid.max_y,
    );
    drop(world);

    // the Lua state the server's own worldgen Lua fills
    let startup = ClientStartup::init(root).map_err(|e| format!("startup: {e}"))?;
    let mut lm =
        LuaManager::init_after(root, Some(Rc::new(startup))).map_err(|e| format!("init: {e:?}"))?;
    exposed_game::init_shared(&lm.statics).map_err(|e| format!("initShared: {e}"))?;
    for dir in ["shared", "server"] {
        lm.load_dir_base(dir)
            .map_err(|e| format!("LoadDirBase {dir}: {e:?}"))?;
    }
    let worldgen = match KahluaTableImpl::rawget(&lm.env, &Object::str("worldgen"))
        .map_err(|e| format!("worldgen: {e:?}"))?
    {
        Object::Table(t) => t,
        _ => return Err("worldgen table missing from the Lua environment".into()),
    };
    let so = sandbox_options_instance(&lm.statics, &mut lm.thread)
        .map_err(|e| format!("SandboxOptions.instance: {e:?}"))?;
    let so_borrowed = so.0.borrow();
    let reader = WorldGenReader {
        sandbox: &so_borrowed,
    };
    let config = reader
        .load_road_config(&worldgen, "roads")
        .map_err(|e| format!("loadRoadConfig: {e:?}"))?;
    let mut out = String::new();
    let _ = writeln!(
        out,
        "META {meta_min_x} {meta_max_x} {meta_min_y} {meta_max_y}"
    );
    let _ = writeln!(out, "CFG {}", config.len());
    for (name, c) in config.iter() {
        let _ = writeln!(
            out,
            "C {} proba={} p={} filter={}",
            esc(name),
            d_hex(c.proba_roads),
            d_hex(c.probability),
            d_hex(c.filter)
        );
    }

    let sample_chunks: [i32; 5] = [0, 30, 1574, -1, 1236];
    for seed in ["1", "42", "DeadOxideOracle"] {
        let mut params = WorldGenParams::default();
        params.set_seed_string(seed);
        let mut offset: i64 = 1000;
        let entries: Vec<(String, _)> = config
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect();
        for (name, road_config) in entries {
            match RoadGenerator::new(
                params.get_seed(),
                road_config,
                offset,
                meta_min_x,
                meta_max_x,
                meta_min_y,
                meta_max_y,
                &params,
            ) {
                Err(e) => {
                    let message = e
                        .strip_prefix("java.lang.RuntimeException: ")
                        .unwrap_or(&e)
                        .to_string();
                    let _ = writeln!(
                        out,
                        "RG {} {offset} {} NEE {}",
                        esc(seed),
                        esc(&name),
                        esc(&message)
                    );
                }
                Ok(gen) => {
                    let mut pts: Vec<(f64, f64, usize)> = gen
                        .road_delaunay_points
                        .iter()
                        .enumerate()
                        .map(|(i, p)| (p.x, p.y, i))
                        .collect();
                    pts.sort_by(|a, b| {
                        jcmp(a.0, b.0).then_with(|| jcmp(a.1, b.1))
                    });
                    let mut fe: Vec<_> = gen.road_delaunay_edges_filtered.iter().collect();
                    fe.sort_by(|a, b| {
                        jcmp(a.a.x, b.a.x)
                            .then_with(|| jcmp(a.a.y, b.a.y))
                            .then_with(|| jcmp(a.b.x, b.b.x))
                            .then_with(|| jcmp(a.b.y, b.b.y))
                    });
                    let _ = writeln!(
                        out,
                        "RG {} {offset} {} pts={} edges={} filtered={} nexus={}",
                        esc(seed),
                        esc(&name),
                        gen.road_delaunay_points.len(),
                        gen.road_delaunay_edges.len(),
                        gen.road_delaunay_edges_filtered.len(),
                        gen.road_nexus.len()
                    );
                    for (x, y, _) in &pts {
                        let _ = writeln!(out, "PT {}", pt(*x, *y));
                    }
                    for e in &fe {
                        let _ = writeln!(
                            out,
                            "FE {} {} {}",
                            pt(e.a.x, e.a.y),
                            pt(e.b.x, e.b.y),
                            d_hex(e.mag_sqrt())
                        );
                    }
                    for (_, _, index) in &pts {
                        let n = &gen.road_nexus[*index];
                        let mut remotes: Vec<_> = n.delaunay_remotes.iter().collect();
                        remotes.sort_by_key(|r| (r.x, r.y));
                        let remotes_str = format!(
                            "[{}]",
                            remotes
                                .iter()
                                .map(|r| format!("({} {})", r.x, r.y))
                                .collect::<Vec<_>>()
                                .join(",")
                        );
                        let _ = writeln!(
                            out,
                            "NX {} {} remotes={}",
                            n.delaunay_point.x, n.delaunay_point.y, remotes_str
                        );
                    }
                    for cx in sample_chunks {
                        for cy in sample_chunks {
                            let mut roads: Vec<_> = gen.get_roads(cx, cy);
                            roads.sort_by(|a, b| {
                                (a.a.x, a.a.y, a.b.x, a.b.y).cmp(&(b.a.x, b.a.y, b.b.x, b.b.y))
                            });
                            for rd in roads {
                                let _ = writeln!(
                                    out,
                                    "RD {cx} {cy} ({} {})-({} {}) {} {} {} {} {}",
                                    rd.a.x,
                                    rd.a.y,
                                    rd.b.x,
                                    rd.b.y,
                                    rd.direction.name(),
                                    rd.ca.x,
                                    rd.ca.y,
                                    rd.cb.x,
                                    rd.cb.y
                                );
                            }
                        }
                    }
                }
            }
            offset += 1000;
        }
    }

    Ok(out)
}

/// LuaGameProbe on the port: the client's startup (zombie::game_window), then luaboot's LoadDirBase, boot events and
/// report, then the UI the scripts built.
pub fn luagame(root: &Path, cache_dir: &str, dirs: &[String]) -> Result<String, String> {
    use zomboid_engine::zombie::game_window::ClientStartup;
    let startup = ClientStartup::init(root).map_err(|e| format!("startup: {e}"))?;
    luaload_after(root, cache_dir, dirs, true, Some(std::rc::Rc::new(startup)))
}

/// LootProbe on the port: the client's startup and the Lua state as luagame builds them, LoadDirBase of shared and
/// server, IsoWorld.init's distribution merge events, `ItemPickerJava.Parse` and `InitSandboxLootSettings`, then the
/// tables printed as LootProbe prints them.
pub fn loot(root: &Path, cache_dir: &str, scenarios: &[String]) -> Result<String, String> {
    use std::rc::Rc;
    use zomboid_engine::se::krka::kahlua::j2se::kahlua_table_impl::KahluaTableImpl;
    use zomboid_engine::se::krka::kahlua::vm::object::Object;
    use zomboid_engine::zombie::game_window::ClientStartup;
    use zomboid_engine::zombie::inventory::item_picker_java::{
        ItemPickerContainer, ItemPickerJava, ItemPickerRoom,
    };
    use zomboid_engine::zombie::inventory::item_picker_roll::StashInfo;
    use zomboid_engine::zombie::lua::lua_manager::LuaManager;

    fn esc(s: Option<&str>) -> String {
        let Some(s) = s else {
            return "nil".into();
        };
        let mut b = String::from("\"");
        for c in s.encode_utf16() {
            if (0x20..0x7f).contains(&c) && c != u16::from(b'"') && c != u16::from(b'\\') {
                b.push(char::from(c as u8));
            } else {
                let _ = write!(b, "\\u{c:04x}");
            }
        }
        b.push('"');
        b
    }
    fn fbits(f: f32) -> String {
        format!("{:x}", f.to_bits())
    }
    fn list(l: &Option<Vec<String>>) -> String {
        match l {
            None => "nil".into(),
            Some(l) => format!(
                "[{}]",
                l.iter().map(|s| esc(Some(s))).collect::<Vec<_>>().join(",")
            ),
        }
    }
    fn flag(b: bool) -> &'static str {
        if b {
            "1"
        } else {
            "0"
        }
    }
    fn sorted<V>(m: &std::collections::HashMap<String, V>) -> Vec<(&String, &V)> {
        let mut v: Vec<_> = m.iter().collect();
        v.sort_by(|a, b| a.0.encode_utf16().cmp(b.0.encode_utf16()));
        v
    }
    fn container(out: &mut String, prefix: &str, label: &str, c: Option<&ItemPickerContainer>) {
        let Some(c) = c else {
            let _ = writeln!(out, "{prefix}{label} nil");
            return;
        };
        let _ = writeln!(
            out,
            "{prefix}{label} rolls={} flags={}{}{}{}{}{}{}{}{}{}{}{}{} fillRand={} maxMap={} stashChance={} items={}",
            fbits(c.rolls),
            flag(c.no_auto_age),
            flag(c.is_shop),
            flag(c.procedural),
            flag(c.dont_spawn_ammo),
            flag(c.gun_storage),
            flag(c.ignore_zombie_density),
            flag(c.cook_food),
            flag(c.can_burn),
            flag(c.is_trash),
            flag(c.is_worn),
            flag(c.is_rotten),
            flag(c.only_one),
            flag(c.default_inventory_loot),
            c.fill_rand,
            c.max_map,
            c.stash_chance,
            c.items.len()
        );
        let _ = write!(out, "{prefix} ITEMS");
        for i in &c.items {
            let _ = write!(out, " {}={}", esc(Some(&i.item_name)), fbits(i.chance));
        }
        out.push('\n');
        match &c.procedural_items {
            None => {
                let _ = writeln!(out, "{prefix} PROC nil");
            }
            Some(p) => {
                let _ = writeln!(out, "{prefix} PROC {}", p.len());
                for p in p {
                    let _ = writeln!(
                        out,
                        "{prefix}  {} min={} max={} weight={} items={} zones={} tiles={} rooms={}",
                        esc(p.name.as_deref()),
                        p.min,
                        p.max,
                        p.weight_chance,
                        list(&p.force_for_items),
                        list(&p.force_for_zones),
                        list(&p.force_for_tiles),
                        list(&p.force_for_rooms)
                    );
                }
            }
        }
        if let Some(j) = &c.junk {
            container(out, &format!("{prefix} "), "JUNK", Some(j));
        }
        if let Some(b) = &c.bags {
            container(out, &format!("{prefix} "), "BAGS", Some(b));
        }
    }
    fn room(out: &mut String, label: &str, r: &ItemPickerRoom) {
        let o = |s: &Option<String>| esc(s.as_deref());
        let _ = writeln!(
            out,
            "{label} fillRand={} isShop={} specificId={} professionChance={} professionChanceInt={} outfit={} outfitFemale={} outfitMale={} outfitChance={} vehicle={} vehicles={} vehicleChance={} vehicleDistribution={} vehicleSkin={} femaleChance={} roomTypes={} zoneRequires={} zoneDisallows={} containerChance={} femaleOdds={} bagType={} bagTable={} containers={}",
            r.fill_rand,
            flag(r.is_shop),
            o(&r.specific_id),
            r.profession_chance,
            r.profession_chance_int,
            o(&r.outfit),
            o(&r.outfit_female),
            o(&r.outfit_male),
            o(&r.outfit_chance),
            o(&r.vehicle),
            list(&r.vehicles),
            o(&r.vehicle_chance),
            o(&r.vehicle_distribution),
            r.vehicle_skin.map_or("null".to_string(), |v| v.to_string()),
            o(&r.female_chance),
            o(&r.room_types),
            o(&r.zone_requires),
            o(&r.zone_disallows),
            o(&r.container_chance),
            o(&r.female_odds),
            o(&r.bag_type),
            o(&r.bag_table),
            r.containers.len()
        );
        for (k, c) in sorted(&r.containers) {
            container(out, " ", &format!("RC {}", esc(Some(k))), Some(c));
        }
    }

    let startup = ClientStartup::init(root).map_err(|e| format!("startup: {e}"))?;
    let mut lm =
        LuaManager::init_after(root, Some(Rc::new(startup))).map_err(|e| format!("init: {e:?}"))?;
    *lm.statics.cache_dir.borrow_mut() = Some(cache_dir.to_string());
    zomboid_engine::zombie::lua::exposed_game::init_shared(&lm.statics)
        .map_err(|e| format!("initShared: {e}"))?;
    for dir in ["shared", "server"] {
        lm.load_dir_base(dir)
            .map_err(|e| format!("LoadDirBase {dir}: {e:?}"))?;
    }
    for e in [
        "OnPreDistributionMerge",
        "OnDistributionMerge",
        "OnPostDistributionMerge",
    ] {
        lm.events
            .trigger_event(e, &lm.env, &mut lm.thread, &[])
            .map_err(|e| format!("event: {e:?}"))?;
    }
    let startup = lm
        .statics
        .state
        .client
        .borrow()
        .clone()
        .ok_or("no client startup")?;
    let mut picker = ItemPickerJava::default();
    picker
        .parse(&lm.env, &startup)
        .map_err(|e| format!("ItemPickerJava.Parse: {e:?}"))?;
    let so = zomboid_engine::zombie::lua::exposed_methods::sandbox_options_instance(
        &lm.statics,
        &mut lm.thread,
    )
    .map_err(|e| format!("SandboxOptions.instance: {e:?}"))?;
    picker.init_sandbox_loot_settings(&so.0.borrow());

    let mut out = String::new();
    let m = &picker.modifiers;
    for (n, v) in [
        ("otherLootModifier", m.other),
        ("foodLootModifier", m.food),
        ("cannedFoodLootModifier", m.canned_food),
        ("weaponLootModifier", m.weapon),
        ("rangedWeaponLootModifier", m.ranged_weapon),
        ("ammoLootModifier", m.ammo),
        ("literatureLootModifier", m.literature),
        ("survivalGearsLootModifier", m.survival_gears),
        ("medicalLootModifier", m.medical),
        ("bagLootModifier", m.bag),
        ("mechanicsLootModifier", m.mechanics),
        ("clothingLootModifier", m.clothing),
        ("containerLootModifier", m.container),
        ("keyLootModifier", m.key),
        ("keyLootModifierD100", m.key_d100),
        ("mediaLootModifier", m.media),
        ("mementoLootModifier", m.memento),
        ("cookwareLootModifier", m.cookware),
        ("materialLootModifier", m.material),
        ("farmingLootModifier", m.farming),
        ("toolLootModifier", m.tool),
        ("skillBookLootModifier", m.skill_book),
        ("recipeResourceLootModifier", m.recipe_resource),
    ] {
        let _ = writeln!(out, "LOOTMOD {n} {}", fbits(v));
    }
    let _ = writeln!(
        out,
        "LOOTMOD zombieDensityCap {}",
        fbits(picker.zombie_density_cap)
    );
    let _ = write!(out, "NOFILL {}", picker.no_container_fill_rooms.len());
    for s in &picker.no_container_fill_rooms {
        let _ = write!(out, " {}", esc(Some(s)));
    }
    out.push('\n');
    for w in &picker.weapon_upgrades {
        let _ = writeln!(
            out,
            "UPGRADE {} {} {}",
            esc(Some(&w.name)),
            w.upgrades.len(),
            list(&Some(w.upgrades.clone()))
        );
    }
    let mut names: Vec<String> = picker.weapon_upgrade_map.keys().cloned().collect();
    names.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
    let _ = writeln!(
        out,
        "WEAPONUPGRADEMAP {} {}",
        names.len(),
        list(&Some(names))
    );
    let _ = writeln!(out, "ADDEDINVALID {}", picker.added_invalid_already.len());
    let _ = writeln!(out, "ROOMS {}", picker.rooms.len());
    for (k, r) in sorted(&picker.rooms) {
        room(&mut out, &format!("ROOM {}", esc(Some(k))), r);
    }
    let _ = writeln!(out, "CONTAINERS {}", picker.containers.len());
    for (k, c) in sorted(&picker.containers) {
        container(
            &mut out,
            "",
            &format!("CONTAINER {}", esc(Some(k))),
            Some(c),
        );
    }
    let _ = writeln!(out, "PROCDISTS {}", picker.procedural_distributions.len());
    for (k, c) in sorted(&picker.procedural_distributions) {
        container(&mut out, "", &format!("PROCDIST {}", esc(Some(k))), Some(c));
    }
    let _ = writeln!(out, "VEHDISTS {}", picker.vehicle_distributions.len());
    for (k, v) in sorted(&picker.vehicle_distributions) {
        let _ = writeln!(
            out,
            "VEHDIST {} specific={}",
            esc(Some(k)),
            v.specific.len()
        );
        if let Some(n) = &v.normal {
            room(&mut out, " VEHNORMAL", n);
        }
        for (i, r) in v.specific.iter().enumerate() {
            room(&mut out, &format!(" VEHSPECIFIC {i}"), r);
        }
    }
    let mut can: Vec<String> = picker
        .can_spawn_as_loot
        .iter()
        .filter_map(|i| startup.item(*i).map(|i| i.full_name()))
        .collect();
    can.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
    let _ = writeln!(out, "CANSPAWN {}", can.len());
    for s in &can {
        let _ = writeln!(out, "CANSPAWNITEM {s}");
    }

    // StashSystem.initAllStashes: one possible stash building per `StashDescriptions` entry
    let mut stash = StashInfo::default();
    if let Object::Table(t) = KahluaTableImpl::rawget(&lm.env, &Object::str("StashDescriptions"))
        .map_err(|e| format!("StashDescriptions: {e:?}"))?
    {
        for (_, v) in KahluaTableImpl::iterate_snapshot(&t) {
            if let Object::Table(d) = v {
                stash.possible += 1;
                if let Some(item) = KahluaTableImpl::rawget_str(&d, "item")
                    .map_err(|e| format!("stash item: {e:?}"))?
                {
                    stash.items.insert(item);
                }
            }
        }
    }
    let sandbox = so.0.borrow();
    eprintln!("LOOT sandbox MaximumDiminishedLoot={:?} DaysUntil={:?} Zombies={:?} AnnotatedMapChance={:?}", sandbox.get("MaximumDiminishedLoot"), sandbox.get("DaysUntilMaximumDiminishedLoot"), sandbox.get("Zombies"), sandbox.get("AnnotatedMapChance"));
    // MapDefinitions.definitions: the `LootMaps.Init` keys of the Lua environment (the client Lua defines them)
    let mut map_definitions: Vec<String> = Vec::new();
    if let Object::Table(lm_table) = KahluaTableImpl::rawget(&lm.env, &Object::str("LootMaps"))
        .map_err(|e| format!("LootMaps: {e:?}"))?
    {
        if let Object::Table(init) = KahluaTableImpl::rawget(&lm_table, &Object::str("Init"))
            .map_err(|e| format!("LootMaps.Init: {e:?}"))?
        {
            for (k, _) in KahluaTableImpl::iterate_snapshot(&init) {
                if let Object::String(_) = k {
                    map_definitions.push(k.java_to_string());
                }
            }
        }
    }
    stash.map_definitions = map_definitions;
    // the Moveable scripts instanced so far persist over the scenarios of one run (the original's scripts do)
    let mut moveables = std::collections::HashSet::new();
    for spec in scenarios {
        loot_scenario(
            &mut out,
            spec,
            &startup,
            &picker,
            &sandbox,
            &stash,
            &mut moveables,
        )?;
    }
    Ok(out)
}

/// One scenario `seed:room:container:times[:capacity[:same|fresh[:building rooms joined by +[:world]]]]` (see
/// LootProbe.java): `times` containers filled one after another from one seeded generator. `world` is a comma
/// separated list of `name=value`: `nights` (GameTime.nightsSurvived), `tod` (GameTime.timeOfDay) or a sandbox option.
fn loot_scenario(
    out: &mut String,
    spec: &str,
    startup: &zomboid_engine::zombie::game_window::ClientStartup,
    picker: &zomboid_engine::zombie::inventory::item_picker_java::ItemPickerJava,
    sandbox: &zomboid_engine::zombie::sandbox_options::SandboxOptions,
    stash: &zomboid_engine::zombie::inventory::item_picker_roll::StashInfo,
    moveables: &mut std::collections::HashSet<usize>,
) -> Result<(), String> {
    use zomboid_engine::zombie::inventory::item_picker_roll::{
        LootBuilding, LootContainer, LootRoller, LootRoom, LootSquare,
    };
    let p: Vec<&str> = spec.split(':').collect();
    if p.len() < 4 {
        return Err(format!("bad loot scenario {spec}"));
    }
    let seed: i64 = p[0].parse().map_err(|_| format!("seed of {spec}"))?;
    let (room, type_name) = (p[1], p[2]);
    let times: usize = p[3].parse().map_err(|_| format!("times of {spec}"))?;
    let capacity: i32 = p
        .get(4)
        .map_or(Ok(50), |c| c.parse())
        .map_err(|_| format!("capacity of {spec}"))?;
    let same = p.get(5) == Some(&"same");
    let bld: Vec<&str> = p.get(6).map_or(vec![room], |b| b.split('+').collect());
    let _ = writeln!(out, "SCENARIO {spec}");
    let mut world_sandbox = sandbox.clone();
    let mut game_time = zomboid_engine::zombie::game_time::GameTime::default();
    for kv in p.get(7).map_or(Vec::new(), |w| w.split(',').collect()) {
        let (k, v) = kv
            .split_once('=')
            .ok_or_else(|| format!("world setting {kv} of {spec}"))?;
        let num: f64 = v
            .parse()
            .map_err(|_| format!("world value {kv} of {spec}"))?;
        match k {
            "nights" => game_time.nights_survived = num as i32,
            "tod" => game_time.time_of_day = num as f32,
            _ => {
                if !world_sandbox.set_by_name(
                    k,
                    &zomboid_engine::se::krka::kahlua::vm::object::Object::Number(num),
                ) {
                    return Err(format!("unknown sandbox option {k} in {spec}"));
                }
            }
        }
    }
    let sandbox = &world_sandbox;
    let mut roller = LootRoller::new(
        startup,
        picker,
        sandbox,
        stash,
        RandAbstract::new(java::Random::new(seed)),
    );
    roller.moveables_instanced = std::mem::take(moveables);
    roller.game_time = game_time;
    let mut square: Option<LootSquare> = None;
    for t in 0..times {
        let mut c;
        if let Some(key) = room.strip_prefix("veh/") {
            c = LootContainer::new(type_name, capacity, None);
            let _ = key;
        } else {
            if square.is_none() || !same {
                let building = LootBuilding {
                    rooms: bld
                        .iter()
                        .map(|n| LootRoom {
                            name: (*n).to_string(),
                            procedural_spawned: std::collections::HashMap::new(),
                        })
                        .collect(),
                };
                let ri = bld.iter().position(|n| *n == room).unwrap_or(0);
                roller.buildings.push(building);
                square = Some(LootSquare {
                    building: roller.buildings.len() - 1,
                    room: ri,
                });
            }
            c = LootContainer::new(type_name, capacity, square);
            // `corpse/<outfit>`: the container of an IsoDeadBody with that outfit
            c.dead_body = room.strip_prefix("corpse/").map(str::to_string);
        }
        let before = roller.rand.id();
        let res = if let Some(key) = room.strip_prefix("veh/") {
            let rd = picker
                .vehicle_distributions
                .get(key)
                .and_then(|v| v.normal.as_ref());
            roller.fill_container_type(rd, &mut c, "all")
        } else {
            roller.fill_container(&mut c)
        };
        let draws = (roller.rand.id() - before + 10000) % 10000;
        // a state the lines cannot print exactly (a bag whose weight rests on a fluid amount) is as unported as a
        // fill that stopped
        let mut lines = String::new();
        // an `EXC <class>` result is a Java exception the original throws too (the fill ends there, its items stay)
        let exc = res
            .as_ref()
            .err()
            .and_then(|e| e.strip_prefix("EXC "))
            .map(str::to_string);
        let res = match &exc {
            Some(_) => roller.item_lines("", &c, &mut lines),
            None => res.and_then(|()| roller.item_lines("", &c, &mut lines)),
        };
        match res {
            Ok(()) if exc.is_some() => {
                let _ = write!(
                    out,
                    "FILL {t} draws={draws} n={} EXC {}",
                    c.items.len(),
                    exc.as_deref().unwrap_or("")
                );
            }
            Ok(()) => {
                let _ = write!(out, "FILL {t} draws={draws} n={}", c.items.len());
            }
            Err(e) => {
                let _ = write!(
                    out,
                    "FILL {t} draws={draws} n={} UNPORTED {e}",
                    c.items.len()
                );
            }
        }
        for i in &c.items {
            let _ = write!(out, " {}", i.full_type);
        }
        out.push('\n');
        out.push_str(&lines);
    }
    let _ = writeln!(out, "END {spec} rand={}", roller.rand.id());
    *moveables = std::mem::take(&mut roller.moveables_instanced);
    Ok(())
}

fn luaload_after(
    root: &Path,
    cache_dir: &str,
    dirs: &[String],
    boot: bool,
    startup: Option<std::rc::Rc<zomboid_engine::zombie::game_window::ClientStartup>>,
) -> Result<String, String> {
    use std::collections::{HashMap, HashSet};
    use std::rc::Rc;
    use zomboid_engine::se::krka::kahlua::j2se::kahlua_table_impl::KahluaTableImpl;
    use zomboid_engine::se::krka::kahlua::vm::object::{Object, TableRef};
    use zomboid_engine::zombie::lua::exposer::JavaEnum;
    use zomboid_engine::zombie::lua::lua_manager::LuaManager;
    fn esc(sb: &mut String, s: &[u16]) {
        sb.push('"');
        for &c in s {
            if (0x20..0x7f).contains(&c) && c != u16::from(b'"') && c != u16::from(b'\\') {
                sb.push(char::from(c as u8));
            } else {
                let _ = write!(sb, "\\u{c:04x}");
            }
        }
        sb.push('"');
    }
    fn key(k: &Object) -> String {
        if let Object::String(s) = k {
            let mut b = String::new();
            esc(&mut b, s);
            return b;
        }
        value(k)
    }
    fn value(o: &Object) -> String {
        match o {
            Object::Nil => "nil".into(),
            Object::Boolean(b) => b.to_string(),
            Object::Number(d) => format!("d{:x}", d.to_bits()),
            Object::String(s) => {
                let mut b = String::from("s");
                esc(&mut b, s);
                b
            }
            Object::Closure(c) => {
                let f = c.prototype.filename.as_deref().unwrap_or("null");
                let f = f.find("/media/lua/").map_or(f, |i| &f[i + 1..]);
                format!(
                    "closure {f}:{}",
                    c.prototype.lines.first().copied().unwrap_or(-1)
                )
            }
            Object::Function(f) => format!("jfunction {}", f.class().name),
            Object::Coroutine(_) => "coroutine".into(),
            Object::Table(_) => "table".into(),
            Object::Userdata(u) => match u.as_any().downcast_ref::<JavaEnum>() {
                Some(e) => format!("userdata {}.{}", e.class.name, e.name),
                None => format!("userdata {}", u.class().name),
            },
        }
    }
    struct Dumper {
        sb: String,
        seen: HashMap<usize, String>,
        init_tables: HashMap<usize, String>,
        /// The closures printed so far, in the order first printed, with where (LuaLoadEnvProbe.closures).
        closures: Vec<(
            Rc<zomboid_engine::se::krka::kahlua::vm::lua_closure::LuaClosure>,
            String,
        )>,
        closure_index: HashSet<usize>,
    }
    fn addr(t: &TableRef) -> usize {
        Rc::as_ptr(t) as usize
    }
    impl Dumper {
        fn dump(&mut self, path: &str, v: &Object) {
            if let Object::Table(t) = v {
                if let Some(init) = self.init_tables.get(&addr(t)) {
                    let _ = writeln!(self.sb, "{path} = init {init}");
                    return;
                }
                if let Some(before) = self.seen.get(&addr(t)) {
                    let _ = writeln!(self.sb, "{path} = ref {before}");
                    return;
                }
                self.seen.insert(addr(t), path.to_string());
                let _ = writeln!(self.sb, "{path} = table");
                for (k, v) in KahluaTableImpl::iterate_snapshot(t) {
                    self.dump(&format!("{path}[{}]", key(&k)), &v);
                }
            } else {
                let _ = writeln!(self.sb, "{path} = {}", value(v));
                if let Object::Closure(c) = v {
                    self.closure(c, path);
                }
            }
        }
        fn closure(
            &mut self,
            c: &Rc<zomboid_engine::se::krka::kahlua::vm::lua_closure::LuaClosure>,
            path: &str,
        ) {
            if self.closure_index.insert(Rc::as_ptr(c) as usize) {
                self.closures.push((c.clone(), path.to_string()));
            }
        }
        /// LuaGameProbe.element: `<prefix> x y width height (raw bits) visible anchors(T L R B) alwaysOnTop capture
        /// scrollChildren scrollWithParent yScroll scrollHeight consumeMouseEvents wantKeyEvents wantExtraMouseEvents
        /// forceCursorVisible table`, then the element's table (`UITABLE <path>`, a ref when printed before).
        fn ui_element(
            &mut self,
            prefix: &str,
            path: &str,
            u: &Rc<dyn zomboid_engine::se::krka::kahlua::vm::object::Userdata>,
        ) {
            use zomboid_engine::zombie::ui::ui_element::ui_element;
            let Some(obj) = ui_element(u) else {
                let _ = writeln!(self.sb, "{prefix} {}", u.class().name);
                return;
            };
            let table = {
                let e = obj.0.borrow();
                let flag = |b: bool| if b { "1" } else { "0" };
                let _ = write!(
                    self.sb,
                    "{prefix} {:x} {:x} {:x} {:x} {} {}{}{}{} {} {} {} {} {:x} {} {}{}{}{} ",
                    e.x.to_bits(),
                    e.y.to_bits(),
                    e.width.to_bits(),
                    e.height.to_bits(),
                    flag(e.visible),
                    flag(e.anchor_top),
                    flag(e.anchor_left),
                    flag(e.anchor_right),
                    flag(e.anchor_bottom),
                    flag(e.always_on_top),
                    flag(e.capture),
                    flag(e.scroll_children),
                    flag(e.scroll_with_parent),
                    e.y_scroll.to_bits(),
                    e.scroll_height,
                    flag(e.consume_mouse_events),
                    flag(e.want_key_events),
                    flag(e.want_extra_mouse_events),
                    flag(e.force_cursor_visible),
                );
                e.table.clone()
            };
            let table = table.map_or(Object::Nil, Object::Table);
            let _ = writeln!(self.sb, "{}", value(&table));
            self.dump(&format!("UITABLE {path}"), &table);
        }
        /// LuaGameProbe.tree: the controls of `u`, depth first, as `UICHILD <depth> <path>`.
        fn ui_tree(
            &mut self,
            path: &str,
            u: &Rc<dyn zomboid_engine::se::krka::kahlua::vm::object::Userdata>,
            depth: usize,
        ) {
            use zomboid_engine::zombie::ui::ui_element::ui_element;
            let Some(obj) = ui_element(u) else {
                return;
            };
            let controls = obj.0.borrow().controls.clone();
            for (i, c) in controls.iter().enumerate() {
                let p = format!("{path}.{i}");
                self.ui_element(&format!("UICHILD {depth} {p}"), &p, c);
                self.ui_tree(&p, c, depth + 1);
            }
        }
        /// LuaGameProbe.ui: each element of one of UIManager's lists, then its controls.
        fn ui_list(
            &mut self,
            list: &str,
            items: &[Rc<dyn zomboid_engine::se::krka::kahlua::vm::object::Userdata>],
        ) {
            for (i, u) in items.iter().enumerate() {
                let p = format!("{list}[{i}]");
                self.ui_element(&format!("UI {list} {i}"), &p, u);
                self.ui_tree(&p, u, 1);
            }
        }
    }
    let mut lm = LuaManager::init_after(root, startup).map_err(|e| format!("init: {e:?}"))?;
    // the probe's ZomboidFileSystem.instance.setCacheDir(<scratch>)
    *lm.statics.cache_dir.borrow_mut() = Some(cache_dir.to_string());
    let env = lm.env.clone();
    let mut init_globals: Vec<(Object, Object)> = Vec::new();
    let mut init_keys: HashMap<usize, HashSet<String>> = HashMap::new();
    let mut d = Dumper {
        sb: String::new(),
        seen: HashMap::new(),
        init_tables: HashMap::new(),
        closures: Vec::new(),
        closure_index: HashSet::new(),
    };
    for (k, v) in KahluaTableImpl::iterate_snapshot(&env) {
        if let (Object::Table(t), Object::String(name)) = (&v, &k) {
            if !Rc::ptr_eq(t, &env) {
                let keys = KahluaTableImpl::iterate_snapshot(t)
                    .iter()
                    .map(|(k, _)| key(k))
                    .collect();
                init_keys.insert(addr(t), keys);
                d.init_tables
                    .insert(addr(t), String::from_utf16_lossy(name));
            }
        }
        init_globals.push((k, v));
    }
    d.init_tables.insert(addr(&env), "_G".into());
    let init_collections: HashMap<String, String> = luaload_static_collections(&env)
        .into_values()
        .map(|(name, v)| (name, static_collection_text(&v, &value).unwrap_or_default()))
        .collect();
    // GameWindow.initShared, after LuaManager.init (a no-op without a client's startup)
    zomboid_engine::zombie::lua::exposed_game::init_shared(&lm.statics)
        .map_err(|e| format!("initShared: {e}"))?;
    for dir in dirs {
        lm.load_dir_base(dir)
            .map_err(|e| format!("LoadDirBase {dir}: {e:?}"))?;
    }
    let boot_state = if boot {
        Some(luaboot_events(&mut lm, &value).map_err(|e| format!("boot: {e:?}"))?)
    } else {
        None
    };
    for line in &lm.loader.borrow().log {
        eprintln!("LOG {line}");
    }
    for f in &lm.loader.borrow().loaded {
        let i = f.find("/media/lua/").map_or(0, |i| i + 1);
        let _ = writeln!(d.sb, "LOADED {}", &f[i..]);
    }
    let mut globals: Vec<(String, Object)> = KahluaTableImpl::iterate_snapshot(&env)
        .into_iter()
        .map(|(k, _)| (key(&k), k))
        .collect();
    globals.sort_by(|a, b| a.0.encode_utf16().cmp(b.0.encode_utf16()));
    let init_value = |k: &Object| -> Option<Object> {
        init_globals
            .iter()
            .find(|(ik, _)| ik.same(k) || key(ik) == key(k))
            .map(|(_, v)| v.clone())
    };
    for (gk, k) in &globals {
        let Some(Object::Table(t)) = init_value(k) else {
            continue;
        };
        let now = KahluaTableImpl::rawget(&env, k).map_err(|e| format!("{e:?}"))?;
        let Object::Table(now) = now else {
            continue;
        };
        if !Rc::ptr_eq(&now, &t) || !init_keys.contains_key(&addr(&t)) {
            continue;
        }
        let had = &init_keys[&addr(&t)];
        let added: Vec<(Object, Object)> = KahluaTableImpl::iterate_snapshot(&t)
            .into_iter()
            .filter(|(k, _)| !had.contains(&key(k)))
            .collect();
        let name = d.init_tables.remove(&addr(&t)).unwrap_or_default();
        for (k2, v2) in added {
            d.dump(&format!("ADDED G[{gk}][{}]", key(&k2)), &v2);
        }
        d.init_tables.insert(addr(&t), name);
    }
    for (gk, k) in &globals {
        let v = KahluaTableImpl::rawget(&env, k).map_err(|e| format!("{e:?}"))?;
        if let Some(iv) = init_value(k) {
            if iv.same(&v) {
                continue;
            }
        }
        d.dump(&format!("G[{gk}]"), &v);
    }
    for e in lm.events.event_list.borrow().iter() {
        let callbacks = e.callbacks.borrow();
        if callbacks.is_empty() {
            continue;
        }
        let _ = writeln!(d.sb, "EVENT {} {}", e.name, callbacks.len());
        for (i, c) in callbacks.iter().enumerate() {
            let _ = writeln!(d.sb, "  {}", value(&Object::Closure(c.clone())));
            d.closure(c, &format!("EVENT {} {i}", e.name));
        }
    }
    luaload_java_state(&lm, &init_collections, &mut d.sb, &value);
    if let Some((boot_sb, tos)) = boot_state {
        d.sb.push_str(&boot_sb);
        luaboot_java_state(&lm, &tos, &mut d.sb, &value);
        // LuaBootProbe's upvalue section: each closure printed with upvalues, in the order first printed
        let mut n = 0;
        while n < d.closures.len() {
            let (c, path) = d.closures[n].clone();
            if !c.upvalues.is_empty() {
                let _ = writeln!(d.sb, "CLOSURE U{n} {path} {}", c.upvalues.len());
                for (j, u) in c.upvalues.iter().enumerate() {
                    let v = u.borrow().get_value();
                    d.dump(&format!("U{n}[{j}]"), &v);
                }
            }
            n += 1;
        }
        // LuaGameProbe's UI section: UIManager's pending lists, then the UI list
        if lm.statics.state.client.borrow().is_some() {
            let (to_add, to_remove, ui) = {
                let m = lm.statics.state.ui_manager.borrow();
                (m.to_add.clone(), m.to_remove.clone(), m.ui.clone())
            };
            d.ui_list("toAdd", &to_add);
            d.ui_list("toRemove", &to_remove);
            d.ui_list("UI", &ui);
        }
    }
    Ok(d.sb)
}

/// LuaBootProbe's steps after LoadDirBase on the port: ZomboidGlobals.Load, then the events the original fires on
/// its way to the main menu, each through Event.trigger; returns the BOOT lines and the TermsOfServiceState given
/// to OnGameStateEnter.
fn luaboot_events(
    lm: &mut zomboid_engine::zombie::lua::lua_manager::LuaManager,
    value: &dyn Fn(&zomboid_engine::se::krka::kahlua::vm::object::Object) -> String,
) -> Result<
    (
        String,
        std::rc::Rc<zomboid_engine::zombie::lua::exposed_boot::TermsOfServiceStateObject>,
    ),
    zomboid_engine::se::krka::kahlua::vm::object::Throwable,
> {
    use std::rc::Rc;
    use zomboid_engine::se::krka::kahlua::vm::object::Object;
    use zomboid_engine::zombie::lua::exposed_boot::TermsOfServiceStateObject;
    let mut sb = String::new();
    let mut fire = |lm: &mut zomboid_engine::zombie::lua::lua_manager::LuaManager,
                    name: &str,
                    params: &[Object]|
     -> Result<(), zomboid_engine::se::krka::kahlua::vm::object::Throwable> {
        let e = lm.events.add_event(name, &lm.env)?;
        let n = e.callbacks.borrow().len();
        let _ = writeln!(sb, "BOOT {name} {n}");
        for (c, r) in e.trigger(&mut lm.thread, params)? {
            let ok = matches!(r.first(), Some(Object::Boolean(true)));
            let _ = write!(sb, "  {} {ok}", value(&Object::Closure(c)));
            if !ok {
                let _ = write!(sb, " {}", value(r.get(1).unwrap_or(&Object::Nil)));
            }
            sb.push('\n');
        }
        Ok(())
    };
    // LuaBootProbe's debugging aid: LUABOOT_STOP=<event> stops after that event, =load before ZomboidGlobals.Load
    let stop = std::env::var("LUABOOT_STOP").unwrap_or_default();
    let tos = Rc::new(TermsOfServiceStateObject::default());
    if stop == "load" {
        return Ok((sb, tos));
    }
    lm.statics
        .state
        .zomboid_globals
        .borrow_mut()
        .load(&lm.env)?;
    fire(lm, "OnLoadSoundBanks", &[])?;
    if stop == "OnLoadSoundBanks" {
        return Ok((sb, tos));
    }
    // GameWindow.enter's steps before OnGameBoot, in a client whose startup ran (LuaGameProbe.enter)
    zomboid_engine::zombie::lua::exposed_game::enter(&lm.statics)
        .map_err(zomboid_engine::se::krka::kahlua::vm::object::Throwable::runtime)?;
    fire(lm, "OnGameBoot", &[])?;
    if stop == "OnGameBoot" {
        return Ok((sb, tos));
    }
    fire(lm, "OnGameStateEnter", &[Object::Userdata(tos.clone())])?;
    // the rest of TermsOfServiceState.enter
    tos.0.borrow_mut().after_enter_event();
    if stop == "OnGameStateEnter" {
        return Ok((sb, tos));
    }
    fire(lm, "OnMainMenuEnter", &[])?;
    Ok((sb, tos))
}

/// The rest of LuaBootProbe's report: ZomboidGlobals, ObservationFactory, SurvivorDesc's colours and the
/// TermsOfServiceState.
fn luaboot_java_state(
    lm: &zomboid_engine::zombie::lua::lua_manager::LuaManager,
    tos: &zomboid_engine::zombie::lua::exposed_boot::TermsOfServiceStateObject,
    sb: &mut String,
    value: &dyn Fn(&zomboid_engine::se::krka::kahlua::vm::object::Object) -> String,
) {
    let state = &lm.statics.state;
    for (name, bits) in state.zomboid_globals.borrow().fields() {
        let _ = writeln!(sb, "ZOMBOIDGLOBALS {name} {bits}");
    }
    use zomboid_engine::se::krka::kahlua::vm::object::Object;
    use zomboid_engine::zombie::lua::exposed_boot::{
        ColorObject, ImmutableColorObject, ObservationMapObject,
    };
    use zomboid_engine::zombie::lua::exposed_methods::ArrayListObject;
    let s = |t: &Option<String>| {
        t.as_deref()
            .map_or("nil".to_string(), |x| value(&Object::str(x)))
    };
    if let Object::Userdata(u) = lm.statics.field(
        "zombie.characters.traits.ObservationFactory",
        "observationMap",
    ) {
        if let Some(m) = u.as_any().downcast_ref::<ObservationMapObject>() {
            let m = m.0.borrow();
            // TreeMap<String, ...>: UTF-16 order (a null key would throw there; none is added)
            let mut keys: Vec<&Option<String>> = m.observation_map.keys().collect();
            keys.sort_by(|a, b| {
                let a: Vec<u16> = a.as_deref().unwrap_or("").encode_utf16().collect();
                let b: Vec<u16> = b.as_deref().unwrap_or("").encode_utf16().collect();
                a.cmp(&b)
            });
            for k in keys {
                let o = &m.observation_map[k];
                let me: Vec<String> = o.mutually_exclusive.iter().map(s).collect();
                let _ = writeln!(
                    sb,
                    "OBSERVATION {} {} {} {} [{}]",
                    s(k),
                    s(&o.trait_id),
                    s(&o.name),
                    s(&o.description),
                    me.join(",")
                );
            }
        }
    }
    let list = |field: &str| -> Vec<Object> {
        match lm.statics.field("zombie.characters.SurvivorDesc", field) {
            Object::Userdata(u) => u
                .as_any()
                .downcast_ref::<ArrayListObject>()
                .map(|l| l.0.borrow().clone())
                .unwrap_or_default(),
            _ => Vec::new(),
        }
    };
    for o in list("TrouserCommonColors") {
        if let Object::Userdata(u) = o {
            if let Some(c) = u.as_any().downcast_ref::<ColorObject>() {
                let c = *c.0.borrow();
                let _ = writeln!(
                    sb,
                    "TROUSERCOLOR {:x} {:x} {:x} {:x}",
                    c.r.to_bits(),
                    c.g.to_bits(),
                    c.b.to_bits(),
                    c.a.to_bits()
                );
            }
        }
    }
    for o in list("HairCommonColors") {
        if let Object::Userdata(u) = o {
            if let Some(c) = u.as_any().downcast_ref::<ImmutableColorObject>() {
                let c = c.0;
                let _ = writeln!(
                    sb,
                    "HAIRCOLOR {:x} {:x} {:x} {:x}",
                    c.r.to_bits(),
                    c.g.to_bits(),
                    c.b.to_bits(),
                    c.a.to_bits()
                );
            }
        }
    }
    if let Object::Userdata(u) = zomboid_engine::zombie::lua::exposed_methods::singleton(
        &lm.statics,
        zomboid_engine::zombie::lua::exposer::class_named("zombie.scripting.ScriptManager"),
    ) {
        if let Some(m) = u
            .as_any()
            .downcast_ref::<zomboid_engine::zombie::lua::exposed_methods::ScriptManagerObject>()
        {
            // TreeMap<String, Integer> of String.valueOf(tag)
            let mut tags: Vec<(Vec<u16>, String, usize)> = m
                .tag_to_item_map
                .borrow()
                .iter()
                .map(|(k, v)| {
                    let name = if k.is_nil() {
                        "null".to_string()
                    } else {
                        k.java_to_string()
                    };
                    let n = match v {
                        Object::Userdata(l) => l
                            .as_any()
                            .downcast_ref::<ArrayListObject>()
                            .map_or(0, |l| l.0.borrow().len()),
                        _ => 0,
                    };
                    (name.encode_utf16().collect(), name, n)
                })
                .collect();
            tags.sort_by(|a, b| a.0.cmp(&b.0));
            for (_, name, n) in tags {
                let _ = writeln!(sb, "ITEMSTAG {} {n}", value(&Object::str(&name)));
            }
        }
    }
    let t = *tos.0.borrow();
    let _ = writeln!(sb, "TOSSTATE {} {}", t.created, t.exit);
}

/// LuaLoadEnvProbe's text of a static collection: a list's elements, a map's size (the port's other maps are
/// opaque and never change, so they are left out on both sides of the comparison).
fn static_collection_text(
    v: &zomboid_engine::se::krka::kahlua::vm::object::Object,
    value: &dyn Fn(&zomboid_engine::se::krka::kahlua::vm::object::Object) -> String,
) -> Option<String> {
    use zomboid_engine::se::krka::kahlua::vm::object::Object;
    use zomboid_engine::zombie::lua::exposed_boot::ObservationMapObject;
    use zomboid_engine::zombie::lua::exposed_methods::ArrayListObject;
    let Object::Userdata(u) = v else { return None };
    if let Some(l) = u.as_any().downcast_ref::<ArrayListObject>() {
        return Some(format!(
            "[{}]",
            l.0.borrow().iter().map(value).collect::<Vec<_>>().join(",")
        ));
    }
    if let Some(m) = u
        .as_any()
        .downcast_ref::<zomboid_engine::zombie::lua::exposed_game::StartupMapObject>()
    {
        return Some(format!("size {}", m.size()));
    }
    u.as_any()
        .downcast_ref::<ObservationMapObject>()
        .map(|m| format!("size {}", m.0.borrow().observation_map.len()))
}

/// The ArrayLists (and the maps the port keeps) public static fields hold, by "<class>.<field>", from the statics
/// tables globals hold (LuaLoadEnvProbe.staticCollections).
fn luaload_static_collections(
    env: &zomboid_engine::se::krka::kahlua::vm::object::TableRef,
) -> std::collections::BTreeMap<
    Vec<u16>,
    (String, zomboid_engine::se::krka::kahlua::vm::object::Object),
> {
    use zomboid_engine::se::krka::kahlua::j2se::kahlua_table_impl::KahluaTableImpl;
    use zomboid_engine::se::krka::kahlua::vm::object::{ClassObject, Object};
    use zomboid_engine::zombie::lua::exposed_methods::ArrayListObject;
    let mut r = std::collections::BTreeMap::new();
    for (_, v) in KahluaTableImpl::iterate_snapshot(env) {
        let Object::Table(t) = v else { continue };
        let Ok(Object::Userdata(c)) = KahluaTableImpl::rawget(&t, &Object::str("class")) else {
            continue;
        };
        let Some(c) = c.as_any().downcast_ref::<ClassObject>() else {
            continue;
        };
        for (k, v) in KahluaTableImpl::iterate_snapshot(&t) {
            if let Object::Userdata(u) = &v {
                if u.as_any().is::<ArrayListObject>()
                    || u.as_any()
                        .is::<zomboid_engine::zombie::lua::exposed_boot::ObservationMapObject>()
                    || u.as_any()
                        .is::<zomboid_engine::zombie::lua::exposed_game::StartupMapObject>()
                {
                    let name = format!("{}.{}", c.0.name, k.java_to_string());
                    r.insert(name.encode_utf16().collect(), (name, v.clone()));
                }
            }
        }
    }
    r
}

/// LuaLoadEnvProbe.javaState on the port: the Java state the scripts filled while loading.
fn luaload_java_state(
    lm: &zomboid_engine::zombie::lua::lua_manager::LuaManager,
    init_collections: &std::collections::HashMap<String, String>,
    sb: &mut String,
    value: &dyn Fn(&zomboid_engine::se::krka::kahlua::vm::object::Object) -> String,
) {
    use std::collections::BTreeMap;
    use std::rc::Rc;
    use zomboid_engine::se::krka::kahlua::vm::object::Object;
    use zomboid_engine::zombie::lua::exposed_methods::{singleton, ArrayListObject};
    use zomboid_engine::zombie::lua::exposed_table::RANDOMIZED_WORLD_BASE_LISTS;
    use zomboid_engine::zombie::lua::exposed_world::{
        BentFencesObject, BrokenFencesObject, ContainerOverlaysObject, TileOverlaysObject,
        WorldGenUtilsObject,
    };
    use zomboid_engine::zombie::lua::exposer::class_named;
    let s = |t: &str| value(&Object::str(t));
    let opt = |t: &Option<String>| t.as_deref().map_or("nil".to_string(), s);
    let list = |l: &[String]| format!("[{}]", l.iter().map(|x| s(x)).collect::<Vec<_>>().join(","));
    let opt_list = |l: &Option<Vec<String>>| l.as_deref().map_or("nil".to_string(), list);
    let objects = |l: &ArrayListObject| {
        format!(
            "[{}]",
            l.0.borrow().iter().map(value).collect::<Vec<_>>().join(",")
        )
    };
    // TreeMap<String, V>: String.compareTo is UTF-16 order
    fn sorted<'a, V>(
        m: impl IntoIterator<Item = (&'a String, V)>,
    ) -> BTreeMap<Vec<u16>, (&'a String, V)> {
        m.into_iter()
            .map(|(k, v)| (k.encode_utf16().collect(), (k, v)))
            .collect()
    }
    let events = lm.events.event_list.borrow();
    let _ = write!(sb, "EVENTS {}", events.len());
    for e in events.iter() {
        let _ = write!(sb, " {}", e.name);
    }
    sb.push('\n');
    for (_, (name, v)) in luaload_static_collections(&lm.env) {
        if name.ends_with("Texture.nullTextures") {
            continue;
        }
        let Some(is) = static_collection_text(&v, value) else {
            continue;
        };
        if init_collections.get(&name) != Some(&is) {
            let _ = writeln!(sb, "STATIC {name} {is}");
        }
    }
    let state = &lm.statics.state;
    let map_objects = state.map_objects.borrow();
    for (which, m) in [
        ("onNew", &map_objects.on_new),
        ("onLoad", &map_objects.on_load),
    ] {
        for (_, (sprite, cb)) in sorted(m) {
            let _ = write!(sb, "MAPOBJECTS {which} {}", s(sprite));
            for (p, f) in cb.priority.iter().zip(&cb.functions) {
                let _ = write!(sb, " {p}={}", value(f));
            }
            sb.push('\n');
        }
    }
    let the = |class: &str| singleton(&lm.statics, class_named(class));
    let o = the("zombie.iso.TileOverlays");
    if let Object::Userdata(u) = &o {
        if let Some(to) = u.as_any().downcast_ref::<TileOverlaysObject>() {
            let to = to.0.borrow();
            for (_, (k, o)) in sorted(&to.overlay_map) {
                let _ = writeln!(sb, "TILEOVERLAY {} {}", s(k), s(&o.tile));
                for e in &o.entries {
                    let u = &e.usage;
                    let _ = writeln!(
                        sb,
                        "  {} {} {} {} {} {:x} {} {}",
                        opt(&e.room),
                        e.chance,
                        opt(&u.usage),
                        u.z_only,
                        u.z_greater_than,
                        u.alpha.to_bits(),
                        u.table_top,
                        list(&e.tiles)
                    );
                }
            }
            for (_, (k, v)) in sorted(&to.overlay_name_to_underlying_name) {
                let _ = writeln!(sb, "TILEUNDER {} {}", s(k), list(v));
            }
        }
    }
    let o = the("zombie.iso.ContainerOverlays");
    if let Object::Userdata(u) = &o {
        if let Some(co) = u.as_any().downcast_ref::<ContainerOverlaysObject>() {
            let co = co.0.borrow();
            for (_, (k, o)) in sorted(&co.overlay_map) {
                let _ = writeln!(sb, "CONTAINEROVERLAY {} {}", s(k), s(&o.name));
                for e in &o.entries {
                    let _ = writeln!(
                        sb,
                        "  {} {} {}",
                        opt(&e.room),
                        opt(&e.many_items),
                        opt(&e.few_items)
                    );
                }
            }
            for (_, (k, v)) in sorted(&co.overlay_name_to_underlying_name) {
                let _ = writeln!(sb, "CONTAINERUNDER {} {}", s(k), list(v));
            }
        }
    }
    let o = the("zombie.iso.BentFences");
    if let Object::Userdata(u) = &o {
        if let Some(bf) = u.as_any().downcast_ref::<BentFencesObject>() {
            let bf = bf.0.borrow();
            for (i, e) in bf.entries.iter().enumerate() {
                let _ = writeln!(
                    sb,
                    "BENTFENCE {i} {} {} {} {} {} {} {}",
                    e.dir.name(),
                    e.health,
                    e.length,
                    e.collapsed_offset,
                    e.collapsed_size_x,
                    e.collapsed_size_y,
                    e.do_smash
                );
                let stages: BTreeMap<i32, &Vec<String>> =
                    e.stages.iter().map(|(k, v)| (*k, v)).collect();
                for (k, v) in stages {
                    let _ = writeln!(sb, "  stage {k} {}", list(v));
                }
                let _ = writeln!(sb, "  collapsed {}", list(&e.collapsed));
                let _ = writeln!(sb, "  debris {}", list(&e.debris));
            }
            let set = |h: &std::collections::HashSet<String>| {
                let v: Vec<String> = sorted(h.iter().map(|x| (x, ())))
                    .into_values()
                    .map(|(x, _)| x.clone())
                    .collect();
                list(&v)
            };
            let _ = writeln!(sb, "BENTCOLLAPSED {}", set(&bf.collapsed_tiles));
            let _ = writeln!(sb, "BENTDEBRIS {}", set(&bf.debris_tiles));
            for (_, (k, v)) in sorted(&bf.fence_map) {
                let _ = write!(sb, "BENTMAP {}", s(k));
                for e in v {
                    let n = bf
                        .entries
                        .iter()
                        .rposition(|x| Rc::ptr_eq(x, e))
                        .map_or(-1, |i| i as i64);
                    let _ = write!(sb, " {n}");
                }
                sb.push('\n');
            }
        }
    }
    let o = the("zombie.iso.BrokenFences");
    if let Object::Userdata(u) = &o {
        if let Some(brf) = u.as_any().downcast_ref::<BrokenFencesObject>() {
            let brf = brf.0.borrow();
            let unbroken = sorted(&brf.unbroken_map);
            for (k, t) in unbroken.values() {
                let t = t.borrow();
                let _ = write!(sb, "BROKENTILE {}", s(k));
                for l in [
                    &t.self_,
                    &t.left,
                    &t.right,
                    &t.debris_n,
                    &t.debris_s,
                    &t.debris_w,
                    &t.debris_e,
                ] {
                    let _ = write!(sb, " {}", opt_list(l));
                }
                sb.push('\n');
            }
            for (which, m) in [
                ("brokenLeftMap", &brf.broken_left_map),
                ("brokenRightMap", &brf.broken_right_map),
                ("allMap", &brf.all_map),
            ] {
                for (_, (k, t)) in sorted(m) {
                    let label = unbroken
                        .values()
                        .find(|(_, u)| Rc::ptr_eq(u, t))
                        .map_or("orphan".to_string(), |(uk, _)| s(uk));
                    let _ = writeln!(sb, "BROKENMAP {which} {} {label}", s(k));
                }
            }
        }
    }
    let mut lists: Vec<&str> = RANDOMIZED_WORLD_BASE_LISTS.to_vec();
    lists.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
    let clutter = state.clutter.borrow();
    for f in lists {
        let l = match clutter.get(f) {
            Some(Object::Userdata(u)) => u
                .as_any()
                .downcast_ref::<ArrayListObject>()
                .map_or("[]".to_string(), &objects),
            _ => "[]".to_string(),
        };
        let _ = writeln!(sb, "CLUTTER {f} {l}");
    }
    let o = the("zombie.iso.worldgen.WorldGenUtils");
    let files = match &o {
        Object::Userdata(u) => u
            .as_any()
            .downcast_ref::<WorldGenUtilsObject>()
            .map_or_else(Vec::new, |w| w.0.borrow().files.clone()),
        _ => Vec::new(),
    };
    let _ = writeln!(sb, "WORLDGENFILES {}", list(&files));
    let _ = writeln!(sb, "SGLOBALSYSTEMS 0");
    luaload_client_state(lm, sb, &s);
}

/// The client part of LuaLoadEnvProbe.javaState: the music configs, the controller bindings, the loaded binding
/// sets.
fn luaload_client_state(
    lm: &zomboid_engine::zombie::lua::lua_manager::LuaManager,
    sb: &mut String,
    s: &dyn Fn(&str) -> String,
) {
    use std::rc::Rc;
    use zomboid_engine::se::krka::kahlua::vm::object::Object;
    use zomboid_engine::zombie::characters::character_joypad_axis2d_binding as axis2d;
    use zomboid_engine::zombie::characters::character_joypad_button_binding::{
        IsDownBinding, CONSTANTS,
    };
    use zomboid_engine::zombie::lua::exposed_client::{
        MusicIntensityConfigObject, MusicThreatConfigObject,
    };
    // TreeMap<String, V>: String.compareTo is UTF-16 order
    fn sorted<'a, V>(
        m: impl IntoIterator<Item = (&'a String, V)>,
    ) -> std::collections::BTreeMap<Vec<u16>, (&'a String, V)> {
        m.into_iter()
            .map(|(k, v)| (k.encode_utf16().collect(), (k, v)))
            .collect()
    }
    let made = |class: &str| match lm.statics.singleton_if_made(class) {
        Some(Object::Userdata(u)) => Some(u),
        _ => None,
    };
    match made("zombie.audio.MusicIntensityConfig") {
        None => sb.push_str("MUSICINTENSITY nil\n"),
        Some(u) => {
            if let Some(o) = u.as_any().downcast_ref::<MusicIntensityConfigObject>() {
                let c = o.0.borrow();
                let _ = writeln!(sb, "MUSICINTENSITY {}", c.events.len());
                for e in &c.events {
                    let _ = writeln!(
                        sb,
                        "  {} {:x} {} {}",
                        s(&e.id),
                        e.intensity.to_bits(),
                        e.duration,
                        e.multiple
                    );
                }
                for (_, (k, e)) in sorted(&c.event_by_id) {
                    let i = c
                        .events
                        .iter()
                        .position(|x| Rc::ptr_eq(x, e))
                        .map_or(-1, |i| i as i64);
                    let _ = writeln!(sb, "MUSICINTENSITYID {} {i}", s(k));
                }
            }
        }
    }
    match made("zombie.audio.MusicThreatConfig") {
        None => sb.push_str("MUSICTHREAT nil\n"),
        Some(u) => {
            if let Some(o) = u.as_any().downcast_ref::<MusicThreatConfigObject>() {
                let c = o.0.borrow();
                let _ = writeln!(sb, "MUSICTHREAT {}", c.status_list.len());
                for st in &c.status_list {
                    let _ = writeln!(
                        sb,
                        "  {} {:x} {:x}",
                        s(&st.id),
                        st.intensity.to_bits(),
                        st.intensity_override.get().to_bits()
                    );
                }
                for (_, (k, st)) in sorted(&c.status_by_id) {
                    let i = c
                        .status_list
                        .iter()
                        .position(|x| Rc::ptr_eq(x, st))
                        .map_or(-1, |i| i as i64);
                    let _ = writeln!(sb, "MUSICTHREATID {} {i}", s(k));
                }
            }
        }
    }
    let b = lm.statics.state.input_bindings.borrow();
    let name = |n: Option<&str>| n.unwrap_or("nil").to_string();
    for (i, (n, _)) in CONSTANTS.iter().enumerate() {
        let d = match b.button_bindings.binding[i] {
            None => "nil".to_string(),
            Some(IsDownBinding::JoypadButton(x)) => format!("button {}", name(x)),
            Some(IsDownBinding::Axis1dMinMax { axis1d, min, max }) => {
                format!(
                    "axis1d {} {:x} {:x}",
                    name(axis1d),
                    min.to_bits(),
                    max.to_bits()
                )
            }
            Some(IsDownBinding::Axis2dMinMax { axis2d, min, max }) => {
                format!(
                    "axis2d {} {:x} {:x}",
                    name(axis2d),
                    min.to_bits(),
                    max.to_bits()
                )
            }
        };
        let _ = writeln!(sb, "JOYPADBUTTONBINDING {n} {d}");
    }
    for (i, (n, _)) in axis2d::CONSTANTS.iter().enumerate() {
        let _ = writeln!(
            sb,
            "JOYPADAXISBINDING {n} {}",
            name(b.axis2d_bindings.binding[i])
        );
    }
    match &b.loaded_binding_sets {
        None => sb.push_str("INPUTBINDINGSETS nil\n"),
        Some(sets) => {
            let names: Vec<String> = sets.iter().map(|x| s(&x.name)).collect();
            let _ = writeln!(sb, "INPUTBINDINGSETS [{}]", names.join(","));
        }
    }
}

/// TranslatorProbe's output for the port: `zombie.core.Translator` over the game folder, driven by the same command
/// file (see the probe's header). The scratch dir is unused: `json` texts go straight to the part of
/// `tryFillMapFromFile` after its `File.exists` (the probe writes them as files first).
fn translator(root: &Path, commands: &Path) -> Result<String, String> {
    use std::collections::HashMap;
    use zomboid_engine::java_formatter::FormatArg;
    use zomboid_engine::zombie::core::translator::{self, Translator, BY_NAME};

    fn esc(s: &str) -> String {
        let mut b = String::from("\"");
        for c in s.encode_utf16() {
            if (0x20..0x7f).contains(&c) && c != 0x22 && c != 0x5c {
                b.push(c as u8 as char);
            } else {
                let _ = write!(b, "\\u{c:04x}");
            }
        }
        b.push('"');
        b
    }
    fn esc_opt(s: Option<&str>) -> String {
        s.map_or_else(|| "null".to_string(), esc)
    }
    // the command file's escapes, as bytes (\xHH a raw byte, the rest UTF-8; \u a UTF-16 unit in 3-byte form)
    fn unescape(s: &str) -> Vec<u8> {
        let mut out = Vec::new();
        let mut it = s.chars();
        while let Some(c) = it.next() {
            if c != '\\' {
                let mut buf = [0u8; 4];
                out.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
                continue;
            }
            match it.next() {
                Some('n') => out.push(b'\n'),
                Some('t') => out.push(b'\t'),
                Some('r') => out.push(b'\r'),
                Some('\\') => out.push(b'\\'),
                Some('x') => {
                    let h: String = it.by_ref().take(2).collect();
                    out.push(u8::from_str_radix(&h, 16).unwrap_or(0));
                }
                Some('u') => {
                    let h: String = it.by_ref().take(4).collect();
                    let u = u32::from_str_radix(&h, 16).unwrap_or(0);
                    if u < 0x80 {
                        out.push(u as u8);
                    } else if u < 0x800 {
                        out.extend_from_slice(&[0xc0 | (u >> 6) as u8, 0x80 | (u & 0x3f) as u8]);
                    } else {
                        out.extend_from_slice(&[
                            0xe0 | (u >> 12) as u8,
                            0x80 | ((u >> 6) & 0x3f) as u8,
                            0x80 | (u & 0x3f) as u8,
                        ]);
                    }
                }
                Some(e) => {
                    out.push(b'\\');
                    let mut buf = [0u8; 4];
                    out.extend_from_slice(e.encode_utf8(&mut buf).as_bytes());
                }
                None => out.push(b'\\'),
            }
        }
        out
    }
    fn text(field: &str) -> String {
        String::from_utf8_lossy(&unescape(field)).into_owned()
    }
    fn arg(a: &str) -> Result<FormatArg, String> {
        if a == "n" {
            return Ok(FormatArg::Null);
        }
        let v = text(a.get(2..).unwrap_or(""));
        match a.as_bytes().first() {
            Some(b's') => Ok(FormatArg::Str(v)),
            Some(b'd') => java::parse_double(&v)
                .map(FormatArg::Double)
                .ok_or_else(|| format!("bad double {v}")),
            Some(b'b') => Ok(FormatArg::Bool(java::equals_ignore_case(&v, "true"))),
            _ => Err(format!("bad argument {a}")),
        }
    }
    fn arg_text(o: &FormatArg) -> String {
        match o {
            FormatArg::Null => "n".into(),
            FormatArg::Double(d) => format!("d:{:x}", d.to_bits()),
            FormatArg::Bool(b) => format!("b:{b}"),
            FormatArg::Str(s) => format!("s:{}", esc(s)),
            FormatArg::Object { .. } => "?".into(),
        }
    }
    fn fnv(mut h: u64, line: &str) -> u64 {
        for &x in line.as_bytes() {
            h ^= u64::from(x);
            h = h.wrapping_mul(0x100000001b3);
        }
        h
    }
    fn sorted(map: &HashMap<String, String>) -> Vec<(&String, &String)> {
        let mut v: Vec<_> = map.iter().collect();
        v.sort_by(|a, b| java::string_compare_to(a.0, b.0));
        v
    }
    fn dump_map(sb: &mut String, name: &str, map: &HashMap<String, String>, full: bool) {
        let entries = sorted(map);
        if full {
            let _ = writeln!(sb, "MAP {name} {}", entries.len());
            for (k, v) in &entries {
                let _ = writeln!(sb, "{} = {}", esc(k), esc(v));
            }
        } else {
            let mut h: u64 = 0xcbf29ce484222325;
            for (k, v) in &entries {
                h = fnv(h, &format!("{} = {}\n", esc(k), esc(v)));
            }
            let _ = writeln!(sb, "MAP {name} {} {h:016x}", entries.len());
        }
    }
    fn call(t: &mut Translator, method: &str, a: &[String]) -> Result<String, String> {
        let s0 = a.first().map(|f| text(f)).unwrap_or_default();
        let r: Result<Option<String>, String> = match method {
            "getText" | "getTextOrNull" => {
                let args = a[1..]
                    .iter()
                    .map(|x| arg(x))
                    .collect::<Result<Vec<_>, _>>()?;
                let r = if method == "getText" {
                    t.get_text(&s0, &args).map(Some)
                } else {
                    t.get_text_or_null(&s0, &args)
                };
                // a formatting the port does not print stops the dump (it would differ anyway)
                return r.map(|v| esc_opt(v.as_deref()));
            }
            "getDisplayItemName" => Ok(Some(t.get_display_item_name(&s0))),
            "getItemNameFromFullType" => t.get_item_name_from_full_type(&s0, None).map(Some),
            "getItemEvolvedRecipeName" => t.get_item_evolved_recipe_name(&s0, None).map(Some),
            "setDefaultItemEvolvedRecipeName" => t
                .set_default_item_evolved_recipe_name(&s0, &text(&a[1]))
                .map(|()| Some("(void)".to_string())),
            "getMoveableDisplayName" => Ok(Some(t.get_moveable_display_name(&s0))),
            "getMoveableDisplayNameOrNull" => Ok(t.get_moveable_display_name_or_null(&s0)),
            "getRecipeName" => Ok(Some(t.get_recipe_name(&s0))),
            "getRecipeGroupName" => Ok(Some(t.get_recipe_group_name(&s0))),
            "getTextMediaEN" => Ok(Some(t.get_text_media_en(&s0))),
            "getAttributeText" => Ok(t.get_attribute_text(&s0, false)),
            "getAttributeTextOrNull" => Ok(t.get_attribute_text(&s0, true)),
            "getFluidText" => Ok(Some(t.get_fluid_text(&s0))),
            "getEntityText" => Ok(Some(t.get_entity_text(&s0))),
            "getMapLabelText" => Ok(Some(t.get_map_label_text(&s0))),
            _ => return Err(format!("unknown method {method}")),
        };
        Ok(match r {
            Ok(v) => esc_opt(v.as_deref()),
            Err(e) => format!("ERROR {e}"),
        })
    }
    fn call_text(method: &str, a: &[String]) -> Result<String, String> {
        let mut b = format!("CALL {method}(");
        for (i, f) in a.iter().enumerate() {
            if i > 0 {
                b.push_str(", ");
            }
            if i == 0 || !method.starts_with("getText") {
                b.push_str(&esc(&text(f)));
            } else {
                b.push_str(&arg_text(&arg(f)?));
            }
        }
        b.push(')');
        Ok(b)
    }

    let mut t = Translator::new(root);
    t.languages.init(root);
    let mut sb = String::new();
    let mut langs = t.languages.languages.clone();
    langs.sort_by(|a, b| java::string_compare_to(&a.name, &b.name));
    let _ = writeln!(sb, "LANGUAGES {}", langs.len());
    for l in &langs {
        let _ = writeln!(
            sb,
            "LANG {} {} {} {}",
            l.name,
            esc_opt(l.text.as_deref()),
            esc_opt(l.base.as_deref()),
            l.azerty
        );
    }
    let _ = writeln!(sb, "DEFAULT {}", t.languages.get_default_language().name);

    let file =
        std::fs::read_to_string(commands).map_err(|e| format!("{}: {e}", commands.display()))?;
    let mut calls: Vec<Vec<String>> = Vec::new();
    for line in file.lines() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut f: Vec<String> = line.split('\t').map(str::to_string).collect();
        let cmd = f.remove(0);
        match cmd.as_str() {
            "userlang" => {
                t.user_language = text(&f[0]);
                let _ = writeln!(sb, "USERLANG {}", esc(&text(&f[0])));
            }
            "call" => calls.push(f),
            "load" => {
                let option = if f[0] == "_" {
                    String::new()
                } else {
                    text(&f[0])
                };
                let full = f[1] == "full";
                t.option_language_name = option.clone();
                let _ = writeln!(sb, "LOAD {}", esc(&option));
                if let Err(e) = t.load_files() {
                    let _ = writeln!(sb, "ERROR {}", e.cause.java_text());
                    continue;
                }
                let _ = writeln!(sb, "LANGUAGE {}", t.get_language().name);
                let mut stack = String::from("STACK");
                for l in t.for_language_stack() {
                    let _ = write!(stack, " {}", l.name);
                }
                let _ = writeln!(sb, "{stack}");
                for name in BY_NAME {
                    dump_map(&mut sb, name, t.map(name).ok_or("no map")?, full);
                }
                dump_map(&mut sb, "Recorded_Media_EN", &t.recorded_media_en, full);
                for c in &calls {
                    let args = &c[1..];
                    let _ = writeln!(
                        sb,
                        "{} = {}",
                        call_text(&c[0], args)?,
                        call(&mut t, &c[0], args)?
                    );
                }
                let mut missing: Vec<&String> = t.missing.iter().collect();
                missing.sort_by(|a, b| java::string_compare_to(a, b));
                let _ = writeln!(sb, "MISSING {}", missing.len());
                for m in missing {
                    let _ = writeln!(sb, "  {}", esc(m));
                }
                let br = sorted(&t.br_replacements);
                let _ = writeln!(sb, "BR {}", br.len());
                for (k, v) in br {
                    let _ = writeln!(sb, "  {} = {}", esc(k), esc(v));
                }
            }
            "fmt" => {
                let template = text(&f[0]);
                let mut args = f.clone();
                t.get_ui().insert("UI_Probe".into(), template.clone());
                args[0] = "UI_Probe".into();
                let _ = writeln!(sb, "FMT {}", esc(&template));
                for m in ["getText", "getTextOrNull"] {
                    let _ = writeln!(
                        sb,
                        "  {} = {}",
                        call_text(m, &args)?,
                        call(&mut t, m, &args)?
                    );
                }
                args[0] = f[0].clone();
                let _ = writeln!(
                    sb,
                    "  {} = {}",
                    call_text("getText", &args)?,
                    call(&mut t, "getText", &args)?
                );
                t.get_ui().remove("UI_Probe");
            }
            "json" => {
                let id = &f[0];
                let mut map = HashMap::new();
                let mut result = None;
                for field in &f[1..] {
                    if let Err(e) = translator::fill_map_from_bytes(&unescape(field), &mut map) {
                        result = Some(format!("ERROR {}", e.java_text()));
                        break;
                    }
                }
                match result {
                    Some(r) => {
                        let _ = writeln!(sb, "JSON {id} {r}");
                    }
                    None => {
                        let _ = writeln!(sb, "JSON {id} ok {}", map.len());
                        for (k, v) in sorted(&map) {
                            let _ = writeln!(sb, "  {} = {}", esc(k), esc(v));
                        }
                    }
                }
            }
            _ => return Err(format!("unknown command {cmd}")),
        }
    }
    Ok(sb)
}
