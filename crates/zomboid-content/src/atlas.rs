//! Builds one RGBA atlas from the sub-textures a scene needs (shelf packing). Prefers 2x packs.
use crate::pack::{parse_pack, SubTexture};
use std::path::PathBuf;
use std::ops::Range;
use std::collections::BTreeMap;
use std::io::{Read, Seek, SeekFrom};
use std::collections::{HashMap, HashSet};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sprite {
    pub layer: u32,
    pub ax: u32,
    pub ay: u32,
    pub w: i32,
    pub h: i32,
    pub off_x: i32,
    pub off_y: i32,
    pub orig_w: i32,
    pub orig_h: i32,
}

#[derive(Clone)]
pub struct Atlas {
    pub rgba: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub layers: u32,
    pub sprites: HashMap<String, Sprite>,
    /// Next shelf position; existing sprite UVs are never repacked during streaming.
    pub append_cursor: (u32, u32, u32, u32), // (x, y, row_height, layer)
}

pub fn decode_png(bytes: &[u8]) -> Result<(Vec<u8>, u32, u32), String> {
    let mut dec = png::Decoder::new(std::io::Cursor::new(bytes));
    dec.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut rd = dec.read_info().map_err(|e| e.to_string())?;
    let mut buf = vec![0; rd.output_buffer_size()];
    let info = rd.next_frame(&mut buf).map_err(|e| e.to_string())?;
    let b = &buf[..info.buffer_size()];
    let px = match info.color_type {
        png::ColorType::Rgba => b.to_vec(),
        png::ColorType::Rgb => b.chunks(3).flat_map(|c| [c[0], c[1], c[2], 255]).collect(),
        png::ColorType::GrayscaleAlpha => {
            b.chunks(2).flat_map(|c| [c[0], c[0], c[0], c[1]]).collect()
        }
        png::ColorType::Grayscale => b.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        c => return Err(format!("unsupported png color {c:?}")),
    };
    Ok((px, info.width, info.height))
}

pub fn build_atlas(
    root: &Path,
    wanted: &HashSet<String>,
    atlas_width: u32,
    max_height: u32,
) -> Result<Atlas, String> {
    let mut files: Vec<_> = std::fs::read_dir(root.join("media/texturepacks"))
        .map_err(|e| e.to_string())?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|x| x.to_str()) == Some("pack"))
        .collect();
    files.sort_by_key(|p| pack_order(&p.file_name().unwrap().to_string_lossy()));
    let mut packs = files.into_iter().map(|p| {
        std::fs::read(&p)
            .map(|b| (p.display().to_string(), b))
            .map_err(|e| e.to_string())
    });
    build_atlas_from(&mut packs, wanted, atlas_width, max_height)
}

/// Order in which packs are searched: 2x packs first, then by name.
pub fn pack_order(file_name: &str) -> (bool, String) {
    (!file_name.contains("2x"), file_name.to_string())
}

/// `build_atlas` over packs supplied by the caller as (name, bytes), already in search order.
pub fn build_atlas_from(
    packs: &mut dyn Iterator<Item = Result<(String, Vec<u8>), String>>,
    wanted: &HashSet<String>,
    atlas_width: u32,
    max_height: u32,
) -> Result<Atlas, String> {
    build_atlas_indexed(packs, wanted, atlas_width, max_height, None).map(|(atlas, _)| atlas)
}

/// Build the initial atlas and index every available pack in the same pass.
/// Streaming then looks up subtexture PNG offsets without scanning packs again
/// on the first requested chunk.
pub fn build_atlas_and_catalog_from(
    packs: &mut dyn Iterator<Item = Result<(String, Vec<u8>), String>>,
    wanted: &HashSet<String>,
    atlas_width: u32,
    max_height: u32,
    pack_dir: &Path,
) -> Result<(Atlas, TextureCatalog), String> {
    let (atlas, catalog) = build_atlas_indexed(
        packs, wanted, atlas_width, max_height, Some(pack_dir))?;
    Ok((atlas, catalog.ok_or_else(|| "streaming texture index unavailable".to_owned())?))
}

fn build_atlas_indexed(
    packs: &mut dyn Iterator<Item = Result<(String, Vec<u8>), String>>,
    wanted: &HashSet<String>,
    atlas_width: u32,
    max_height: u32,
    pack_dir: Option<&Path>,
) -> Result<(Atlas, Option<TextureCatalog>), String> {
    let mut catalogue = HashMap::new();
    let found: HashMap<String, (Vec<u8>, u32, SubTexture)> = HashMap::new(); // name -> (page rgba shared? copy rect later)
    let mut pages: Vec<(Vec<u8>, u32)> = vec![];
    let mut sel: Vec<(String, usize, SubTexture)> = vec![];
    for pack in packs {
        let (name, b) = pack?;
        for pg in parse_pack(&b).map_err(|e| format!("{name}: {e:?}"))? {
            if let Some(pack_dir) = pack_dir {
                for entry in &pg.entries {
                    catalogue.entry(entry.name.clone()).or_insert_with(|| CatalogEntry {
                        pack_path: pack_dir.join(&name),
                        png: pg.png.clone(), tile: entry.clone(),
                    });
                }
            }
            let hits: Vec<_> = pg
                .entries
                .iter()
                .filter(|e| {
                    wanted.contains(&e.name)
                        && !sel.iter().any(|s| s.0 == e.name)
                        && !found.contains_key(&e.name)
                })
                .cloned()
                .collect();
            if hits.is_empty() {
                continue;
            }
            let (px, w, _h) = decode_png(&b[pg.png.clone()])?;
            pages.push((px, w));
            for e in hits {
                sel.push((e.name.clone(), pages.len() - 1, e));
            }
        }
    }
    // reserved solid-colour block used for the player marker (not from the game)
    pages.push(([255u8, 40, 40, 255].repeat(8 * 8), 8));
    sel.push((
        "__player".to_string(),
        pages.len() - 1,
        SubTexture {
            name: "__player".into(),
            x: 0,
            y: 0,
            w: 8,
            h: 8,
            off_x: 0,
            off_y: 0,
            orig_w: 8,
            orig_h: 8,
        },
    ));
    pages.push(([40u8, 200, 60, 255].repeat(8 * 8), 8));
    sel.push((
        "__zombie".to_string(),
        pages.len() - 1,
        SubTexture {
            name: "__zombie".into(),
            x: 0,
            y: 0,
            w: 8,
            h: 8,
            off_x: 0,
            off_y: 0,
            orig_w: 8,
            orig_h: 8,
        },
    ));
    // shelf pack, tallest first
    sel.sort_by_key(|s| std::cmp::Reverse(s.2.h));
    let (mut x, mut y, mut row_h, mut layer) = (0u32, 0u32, 0u32, 0u32);
    let mut place = vec![];
    let mut layer_h = 1u32;
    for (_, _, e) in &sel {
        let (w, h) = (e.w as u32 + 1, e.h as u32 + 1);
        if x + w > atlas_width {
            x = 0;
            y += row_h;
            row_h = 0;
        }
        if y + h > max_height {
            layer_h = max_height;
            layer += 1;
            x = 0;
            y = 0;
            row_h = 0;
        }
        place.push((layer, x, y));
        x += w;
        row_h = row_h.max(h);
        layer_h = layer_h.max(y + row_h);
    }
    let height = layer_h.max(1);
    let layers = layer + 1;
    let layer_px = (atlas_width * height * 4) as usize;
    let mut rgba = vec![0u8; layer_px * layers as usize];
    let mut sprites = HashMap::new();
    for ((name, pi, e), (ly, ax, ay)) in sel.into_iter().zip(place) {
        let (page, pw) = &pages[pi];
        for yy in 0..e.h as u32 {
            let s = (((e.y as u32 + yy) * pw + e.x as u32) * 4) as usize;
            let d = layer_px * ly as usize + (((ay + yy) * atlas_width + ax) * 4) as usize;
            rgba[d..d + e.w as usize * 4].copy_from_slice(&page[s..s + e.w as usize * 4]);
        }
        sprites.insert(
            name,
            Sprite {
                layer: ly,
                ax,
                ay,
                w: e.w,
                h: e.h,
                off_x: e.off_x,
                off_y: e.off_y,
                orig_w: e.orig_w,
                orig_h: e.orig_h,
            },
        );
    }
    let atlas = Atlas {
        rgba,
        width: atlas_width,
        height,
        layers,
        sprites,
        append_cursor: (x, y, row_h, layer),
    };
    Ok((atlas, pack_dir.map(|_| TextureCatalog { entries: catalogue,
        last_page: None })))
}

/// One new region of a streamed atlas. The Java TexturePack system resolves
/// individual sprites; wgpu uses this append-only atlas instead of repacking all
/// previously loaded subtextures whenever a chunk references a new sprite.
#[derive(Clone, Debug)]
pub struct AtlasPatch {
    pub layer: u32,
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

#[derive(Clone, Debug, Default)]
pub struct AtlasDelta {
    pub sprites: Vec<(String, Sprite)>,
    pub patches: Vec<AtlasPatch>,
    pub layers: u32,
    pub cursor: (u32, u32, u32, u32),
}

impl Atlas {
    /// Reserve some space for incoming streamed sprites without moving any
    /// sprite rectangle. Call before generating instances, since normalised UVs
    /// depend on the height. The base atlas remains unchanged for fixed scenes.
    pub fn reserve_height(&mut self, minimum: u32) {
        if self.height >= minimum { return; }
        let old = self.width as usize * self.height as usize * 4;
        let new = self.width as usize * minimum as usize * 4;
        let mut rgba = vec![0; new * self.layers as usize];
        for layer in 0..self.layers as usize {
            rgba[new * layer..new * layer + old]
                .copy_from_slice(&self.rgba[old * layer..old * (layer + 1)]);
        }
        self.rgba = rgba;
        self.height = minimum;
    }

    /// Apply a worker's appended subtextures to the rendering thread. The
    /// texture extent only changes if another array layer was allocated.
    pub fn apply_delta(&mut self, delta: &AtlasDelta) -> Result<bool, String> {
        if delta.layers < self.layers { return Err("atlas layers regressed".into()); }
        let changed_extent = delta.layers != self.layers;
        let layer_bytes = self.width as usize * self.height as usize * 4;
        self.rgba.resize(layer_bytes * delta.layers as usize, 0);
        self.layers = delta.layers;
        for patch in &delta.patches {
            if patch.layer >= self.layers || patch.x + patch.width > self.width
                || patch.y + patch.height > self.height
                || patch.rgba.len() != patch.width as usize * patch.height as usize * 4 {
                return Err("streamed atlas patch outside texture bounds".into());
            }
            for row in 0..patch.height as usize {
                let start = layer_bytes * patch.layer as usize
                    + ((patch.y as usize + row) * self.width as usize + patch.x as usize) * 4;
                let src = row * patch.width as usize * 4;
                self.rgba[start..start + patch.width as usize * 4]
                    .copy_from_slice(&patch.rgba[src..src + patch.width as usize * 4]);
            }
        }
        for (name, sprite) in &delta.sprites { self.sprites.insert(name.clone(), *sprite); }
        self.append_cursor = delta.cursor;
        Ok(changed_extent)
    }
}

#[derive(Clone)]
struct CatalogEntry {
    pack_path: PathBuf,
    png: Range<usize>,
    tile: SubTexture,
}

/// Parse each texture pack's directory once, then read/decode only PNG pages
/// used by incoming chunks. Does not retain all package bytes in RAM.
pub struct TextureCatalog {
    entries: HashMap<String, CatalogEntry>,
    // Avoid reopening and decoding the same PNG page for neighbouring chunks.
    last_page: Option<(PathBuf, usize, usize, Vec<u8>, u32)>,
}

impl TextureCatalog {
    pub fn open(root: &Path) -> Result<Self, String> {
        let pack_dir = root.join("media/texturepacks");
        let mut paths: Vec<PathBuf> = std::fs::read_dir(&pack_dir)
            .map_err(|e| e.to_string())?
            .filter_map(|entry| entry.ok().map(|e| e.path()))
            .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("pack"))
            .collect();
        paths.sort_by_key(|p| pack_order(&p.file_name().and_then(|n| n.to_str()).unwrap_or("")));
        let mut entries = HashMap::new();
        for path in paths {
            let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            let pages = parse_pack(&bytes).map_err(|e| format!("{}: {e:?}", path.display()))?;
            for page in pages {
                for tile in page.entries {
                    entries.entry(tile.name.clone()).or_insert_with(|| CatalogEntry {
                        pack_path: path.clone(), png: page.png.clone(), tile,
                    });
                }
            }
        }
        Ok(Self { entries, last_page: None })
    }

    /// Extend the atlas only with genuinely new sprite rectangles. Existing
    /// placements and UVs never change. The cursor is retained between calls
    /// so small batches do not waste an entire row per streamed chunk.
    pub fn append_missing(&mut self, atlas: &mut Atlas,
        names: &HashSet<String>) -> Result<AtlasDelta, String> {
        let mut pages: BTreeMap<(PathBuf, usize, usize), Vec<(String, SubTexture)>> = BTreeMap::new();
        for name in names {
            if atlas.sprites.contains_key(name) { continue; }
            if let Some(entry) = self.entries.get(name) {
                pages.entry((entry.pack_path.clone(), entry.png.start, entry.png.end))
                    .or_default().push((name.clone(), entry.tile.clone()));
            }
        }
        let mut delta = AtlasDelta { layers: atlas.layers, cursor: atlas.append_cursor,
            ..Default::default() };
        let (mut x, mut y, mut row_h, mut layer) = atlas.append_cursor;
        for ((path, start, end), mut sprites) in pages {
            if self.last_page.as_ref().is_none_or(|(cached, a, b, _, _)|
                cached != &path || *a != start || *b != end) {
                // The original pack format gives the PNG byte offsets, so the
                // entire pack file need not be copied just to decode one page.
                let mut file = std::fs::File::open(&path)
                    .map_err(|e| format!("{}: {e}", path.display()))?;
                file.seek(SeekFrom::Start(start as u64))
                    .map_err(|e| format!("{}: {e}", path.display()))?;
                let mut png = vec![0; end - start];
                file.read_exact(&mut png)
                    .map_err(|e| format!("{}: {e}", path.display()))?;
                let (pixels, width, _) = decode_png(&png)?;
                self.last_page = Some((path.clone(), start, end, pixels, width));
            }
            let (_, _, _, page, page_width) = self.last_page.as_ref()
                .ok_or_else(|| "PNG page cache missing after read".to_owned())?;
            sprites.sort_by(|a, b| b.1.h.cmp(&a.1.h).then_with(|| a.0.cmp(&b.0)));
            for (name, tile) in sprites {
                if tile.w <= 0 || tile.h <= 0 { continue; }
                if tile.x < 0 || tile.y < 0 {
                    return Err(format!("negative sprite rectangle for {name}"));
                }
                let w = tile.w as u32;
                let h = tile.h as u32;
                if w + 1 > atlas.width || h + 1 > atlas.height {
                    return Err(format!("atlas sprite {name} too large for the streaming page"));
                }
                if x + w + 1 > atlas.width {
                    x = 0; y += row_h; row_h = 0;
                }
                if y + h + 1 > atlas.height {
                    layer += 1; x = 0; y = 0; row_h = 0;
                }
                let mut rgba = Vec::with_capacity(w as usize * h as usize * 4);
                for yy in 0..h as usize {
                    let src = (((tile.y as usize + yy) * *page_width as usize
                        + tile.x as usize) * 4) as usize;
                    let row = page.get(src..src + w as usize * 4)
                        .ok_or_else(|| format!("bad sprite rectangle {name}"))?;
                    rgba.extend_from_slice(row);
                }
                let patch = AtlasPatch { layer, x, y, width: w, height: h, rgba };
                let sprite = Sprite { layer, ax: x, ay: y,
                    w: tile.w, h: tile.h, off_x: tile.off_x, off_y: tile.off_y,
                    orig_w: tile.orig_w, orig_h: tile.orig_h };
                delta.sprites.push((name, sprite));
                delta.patches.push(patch);
                x += w + 1;
                row_h = row_h.max(h + 1);
            }
        }
        delta.cursor = (x, y, row_h, layer);
        delta.layers = layer + 1;
        // Commit only after every referenced pack, page and sprite rectangle
        // has been validated. A corrupt texture pack must not leave an atlas
        // partially repacked when the chunk-load transaction rolls back.
        atlas.apply_delta(&delta)?;
        Ok(delta)
    }
}

#[cfg(test)]
mod streaming_atlas_tests {
    use super::*;

    #[test]
    fn reserve_and_append_preserves_existing_uvs_and_source_pixels() -> Result<(), String> {
        let old = Sprite { layer: 0, ax: 0, ay: 0, w: 2, h: 2,
            off_x: 0, off_y: 0, orig_w: 2, orig_h: 2 };
        let mut atlas = Atlas {
            rgba: vec![0; 8 * 4 * 4], width: 8, height: 4, layers: 1,
            sprites: HashMap::from([("old".to_owned(), old)]),
            append_cursor: (3, 0, 3, 0),
        };
        atlas.rgba[0..8].copy_from_slice(&[11; 8]);
        atlas.reserve_height(8);
        assert_eq!(&atlas.rgba[0..8], &[11; 8]);
        let delta = AtlasDelta {
            sprites: vec![("new".into(), Sprite { layer: 0, ax: 3, ay: 0,
                w: 2, h: 2, ..old })],
            patches: vec![AtlasPatch { layer: 0, x: 3, y: 0,
                width: 2, height: 2, rgba: vec![50; 16] }],
            layers: 1, cursor: (6, 0, 3, 0),
        };
        assert!(!atlas.apply_delta(&delta)?);
        assert_eq!(atlas.sprites["old"], old);
        assert_eq!(&atlas.rgba[0..8], &[11; 8]);
        assert_eq!(&atlas.rgba[12..20], &[50; 8]);
        assert_eq!(&atlas.rgba[8 * 4 + 12..8 * 4 + 20], &[50; 8]);
        let grow = AtlasDelta {
            sprites: vec![("layer1".into(), Sprite { layer: 1, ax: 0, ay: 0,
                w: 2, h: 2, ..old })],
            patches: vec![AtlasPatch { layer: 1, x: 0, y: 0,
                width: 2, height: 2, rgba: vec![80; 16] }],
            layers: 2, cursor: (3, 0, 3, 1),
        };
        assert!(atlas.apply_delta(&grow)?);
        assert_eq!(atlas.layers, 2);
        assert_eq!(atlas.sprites["old"], old);
        assert_eq!(&atlas.rgba[(8 * 8 * 4)..(8 * 8 * 4 + 8)], &[80; 8]);
        Ok(())
    }
}
