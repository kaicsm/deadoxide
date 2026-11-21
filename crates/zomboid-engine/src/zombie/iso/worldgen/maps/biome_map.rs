//! zombie.iso.worldgen.maps.BiomeMap: cached, lazily loaded per-cell biome PNGs.
//! The two image bands select the biome and zone configured by WorldGenReader.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::java::JavaIntHashMap;
use crate::se::krka::kahlua::j2se::kahlua_table_impl::KahluaTableImpl;
use crate::se::krka::kahlua::vm::object::{Object, TableRef};
use crate::zombie::sandbox_options::SandboxOptions;
use crate::zombie::zomboid_file_system::ZomboidFileSystem;
use crate::zombie::iso::worldgen::world_gen_reader::WorldGenReader;

use super::biome_map_entry::BiomeMapEntry;
use super::biome_raster::BiomeRaster;

/// BiomeMap.Type: which channel of a biome image is sampled.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BiomeMapType {
    Biome,
    Zone,
}

impl BiomeMapType {
    pub fn id(self) -> i32 {
        match self { Self::Biome => 0, Self::Zone => 1 }
    }
}

/// The map list and the active-file-map snapshot at construction time. The file map is the
/// ZomboidFileSystem index *after* mods have been loaded, so mod overrides take precedence.
/// The snapshot also permits lazy lookups without borrowing the mutable file system.
#[derive(Debug)]
struct BiomeMapSource {
    world_maps: Vec<String>,
    game_root: PathBuf,
    raster_files: HashMap<String, PathBuf>,
}

impl BiomeMapSource {
    fn new(world_map_names: &str, fs: &ZomboidFileSystem) -> Self {
        let raster_files = fs.active_file_map.iter()
            .filter(|(relative, _)| {
                relative.starts_with("media/maps/")
                    && relative.contains("/maps/biomemap_")
                    && relative.ends_with(".png")
            })
            .map(|(relative, absolute)| (relative.to_lowercase(), PathBuf::from(absolute)))
            .collect();
        Self {
            world_maps: world_map_names.split(';').map(str::to_owned).collect(),
            game_root: fs.base().to_path_buf(),
            raster_files,
        }
    }

    /// BiomeMap.getRaster: look at the active maps in IsoWorld.getMap() order, stopping at
    /// the first existing file, even when its PNG ultimately fails to decode.
    fn find_image(&self, cell_x: i32, cell_y: i32) -> Option<PathBuf> {
        for name in &self.world_maps {
            let relative = format!("media/maps/{name}/maps/biomemap_{cell_x}_{cell_y}.png");
            let path = self.raster_files.get(&relative.to_lowercase())
                .cloned().unwrap_or_else(|| self.game_root.join(&relative));
            if path.exists() {
                return Some(path);
            }
        }
        None
    }
}

/// BiomeMap: Java's 16-entry CacheMap and the WorldGenReader.loadBiomeMapConfig entries.
/// `new_with_filesystem` additionally enables automatic PNG discovery and on-demand decoding.
#[derive(Debug)]
pub struct BiomeMap {
    pub zone_map: JavaIntHashMap<BiomeMapEntry>,
    cache: Mutex<HashMap<(i32, i32), Option<Arc<BiomeRaster>>>>,
    source: Option<BiomeMapSource>,
    pub foraging_zones: Vec<Option<String>>,
}

impl BiomeMap {
    /// BiomeMap.<init>: preserves zoneMap.entrySet order for the foraging zones.
    /// The source can be configured when the selected map names and file index are available.
    pub fn new(zone_map: JavaIntHashMap<BiomeMapEntry>) -> Self {
        let foraging_zones = zone_map.values().map(|e| e.zone.clone()).collect();
        Self { zone_map, cache: Mutex::new(HashMap::new()), source: None, foraging_zones }
    }

    /// BiomeMap.<init>/getRaster: configure the same world map search and mod file overrides
    /// that IsoWorld.getMap and ZomboidFileSystem.instance.getString provide in Java.
    pub fn new_with_filesystem(
        zone_map: JavaIntHashMap<BiomeMapEntry>,
        world_map_names: &str,
        fs: &ZomboidFileSystem,
    ) -> Self {
        let mut map = Self::new(zone_map);
        map.configure_world_maps(world_map_names, fs);
        map
    }

    /// Create the normal runtime BiomeMap from the `biome_map_config` global Lua table.
    /// The caller supplies the game's selected map string and its initialized file system.
    pub fn from_lua_environment(
        world_map_names: &str,
        fs: &ZomboidFileSystem,
        env: &TableRef,
        sandbox: &SandboxOptions,
    ) -> Result<Self, String> {
        let config = KahluaTableImpl::rawget(env, &Object::str("biome_map_config"))
            .map_err(|e| format!("{e:?}"))?;
        let Object::Table(config) = config else {
            return Err("biome_map_config is not a Lua table".to_owned());
        };
        let reader = WorldGenReader { sandbox };
        let zones = reader.load_biome_map_config(&config)
            .map_err(|e| format!("{e:?}"))?;
        Ok(Self::new_with_filesystem(zones, world_map_names, fs))
    }

    /// Update the selected map set (e.g. when switching saves) and invalidate cached rasters.
    /// Call this again if ZomboidFileSystem loads additional mods after initialization.
    pub fn configure_world_maps(&mut self, world_map_names: &str, fs: &ZomboidFileSystem) {
        self.source = Some(BiomeMapSource::new(world_map_names, fs));
        self.cache.get_mut().unwrap_or_else(|p| p.into_inner()).clear();
    }

    /// BiomeMap.getRaster/CacheMap.put: manual injection remains available for fixtures and
    /// callers with an already-decoded image. Java clears only when size() BEFORE put exceeds 16.
    pub fn set_raster(&mut self, cell_x: i32, cell_y: i32, raster: Option<BiomeRaster>) {
        let cache = self.cache.get_mut().unwrap_or_else(|p| p.into_inner());
        if cache.len() > 16 {
            cache.clear();
        }
        cache.insert((cell_x, cell_y), raster.map(Arc::new));
    }

    fn load_raster(&self, cell_x: i32, cell_y: i32) -> Option<Arc<BiomeRaster>> {
        let source = self.source.as_ref()?;
        let path = source.find_image(cell_x, cell_y)?;
        match BiomeRaster::from_png_path(&path) {
            Ok(raster) => Some(Arc::new(raster)),
            Err(error) => {
                // ImageData(String) in Java swallows native PNG decode errors and sets
                // width/height to -1. BiomeRaster.createFromImage still returns, and
                // getSamples later throws "Invalid coordinates.". Cache that invalid
                // raster rather than treating the corrupt file as a missing image.
                // The first existing world map still wins; do not fall through.
                eprintln!("BiomeMap: invalid image at ({cell_x}, {cell_y}), {}: {error}", path.display());
                Some(Arc::new(BiomeRaster::invalid_image()))
            }
        }
    }

    /// BiomeMap.getRaster: miss (including a cached null) is retained until eviction.
    /// A shared raster handle lets callers read decoded pixels without holding the cache lock.
    pub fn get_raster(&self, cell_x: i32, cell_y: i32) -> Option<Arc<BiomeRaster>> {
        let key = (cell_x, cell_y);
        {
            let cache = self.cache.lock().unwrap_or_else(|p| p.into_inner());
            if let Some(cached) = cache.get(&key) {
                return cached.clone();
            }
        }
        let raster = self.load_raster(cell_x, cell_y);
        let mut cache = self.cache.lock().unwrap_or_else(|p| p.into_inner());
        // Another thread may have supplied a raster while the image was being decoded.
        if let Some(cached) = cache.get(&key) {
            return cached.clone();
        }
        if cache.len() > 16 {
            cache.clear();
        }
        cache.insert(key, raster.clone());
        raster
    }

    /// BiomeMap.getCell: return all 256x256 samples from the selected image band.
    pub fn get_cell(&self, cell_x: i32, cell_y: i32, kind: BiomeMapType) -> Result<Option<Vec<i32>>, String> {
        self.get_raster(cell_x, cell_y).map(|r| r.get_samples(0, 0, 256, 256, kind.id())).transpose()
    }

    /// BiomeMap.getZones: Java integer division truncates toward zero, and chunks with negative
    /// local offsets are rejected. Each chunk samples an 8x8 rectangle within a 256x256 image.
    pub fn get_zones(&self, chunk_x: i32, chunk_y: i32, kind: BiomeMapType) -> Result<Option<Vec<i32>>, String> {
        let cell_x = chunk_x / 32;
        let cell_y = chunk_y / 32;
        let dx = chunk_x - cell_x * 32;
        let dy = chunk_y - cell_y * 32;
        if dx < 0 || dy < 0 {
            return Ok(None);
        }
        self.get_raster(cell_x, cell_y).map(|r| r.get_samples(dx * 8, dy * 8, 8, 8, kind.id())).transpose()
    }

    /// BiomeMap.getEntry: resolve a pixel index to configured biome, ore and zone names.
    pub fn get_entry(&self, pixel: i32) -> Option<&BiomeMapEntry> {
        self.zone_map.get(pixel)
    }

    /// BiomeMap.getZoneName.
    pub fn get_zone_name(&self, pixel: i32) -> Option<&str> {
        self.get_entry(pixel).and_then(|e| e.zone.as_deref())
    }

    /// BiomeMap.getBiomeName.
    pub fn get_biome_name(&self, pixel: i32) -> Option<&str> {
        self.get_entry(pixel).and_then(|e| e.biome.as_deref())
    }

    /// BiomeMap.getOreName.
    pub fn get_ore_name(&self, pixel: i32) -> Option<&str> {
        self.get_entry(pixel).and_then(|e| e.ore.as_deref())
    }

    /// BiomeMap.Dispose: discard the decoded images and file search context.
    pub fn dispose(&mut self) {
        self.cache.get_mut().unwrap_or_else(|p| p.into_inner()).clear();
        self.source = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::BufWriter;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let path = std::env::temp_dir().join(format!(
                "deadoxide-biome-map-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir_all(&path).expect("create fixture root");
            Self(path)
        }
        fn raster(&self, map: &str, cell_x: i32, cell_y: i32, red: u8, green: u8) -> PathBuf {
            let folder = self.0.join("media/maps").join(map).join("maps");
            std::fs::create_dir_all(&folder).expect("create map folder");
            let path = folder.join(format!("biomemap_{cell_x}_{cell_y}.png"));
            let file = std::fs::File::create(&path).expect("create PNG");
            let mut encoder = png::Encoder::new(BufWriter::new(file), 256, 256);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().expect("write PNG header");
            let mut image = vec![red, green, 0, 255].repeat(256 * 256);
            // A pixel at x=8,y=0 is in the next chunk to the east.
            image[(8 * 4)..(8 * 4 + 2)].copy_from_slice(&[red.wrapping_add(1), green.wrapping_add(1)]);
            writer.write_image_data(&image).expect("write PNG pixels");
            path
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); }
    }

    fn empty_map() -> JavaIntHashMap<BiomeMapEntry> { JavaIntHashMap::default() }

    #[test]
    fn finds_first_existing_world_map_and_extracts_both_bands() {
        let root = Fixture::new();
        root.raster("First", 0, 0, 3, 7);
        root.raster("Second", 0, 0, 50, 60);
        let fs = ZomboidFileSystem::init(&root.0).expect("index game files");
        let map = BiomeMap::new_with_filesystem(empty_map(), "Missing;First;Second", &fs);
        let biome = map.get_zones(0, 0, BiomeMapType::Biome).expect("samples").expect("image");
        let zone = map.get_zones(1, 0, BiomeMapType::Zone).expect("samples").expect("image");
        assert_eq!(biome, vec![3; 64]);
        assert_eq!(zone[0], 8);
        assert_eq!(zone[1], 7);
        assert_eq!(map.get_zones(32, 0, BiomeMapType::Biome).expect("missing"), None);
        assert_eq!(map.get_zones(-1, 0, BiomeMapType::Biome).expect("negative"), None);
    }

    #[test]
    fn a_cached_missing_cell_remains_missing_until_reconfigured() {
        let root = Fixture::new();
        let mut fs = ZomboidFileSystem::init(&root.0).expect("index empty game");
        let mut map = BiomeMap::new_with_filesystem(empty_map(), "Map", &fs);
        assert!(map.get_cell(0, 0, BiomeMapType::Biome).expect("missing").is_none());
        let path = root.raster("Map", 0, 0, 10, 20);
        // File map was indexed before the file existed; a fresh index resolves it.
        assert!(map.get_cell(0, 0, BiomeMapType::Biome).expect("cached miss").is_none());
        fs.active_file_map.insert("media/maps/map/maps/biomemap_0_0.png".into(), path.to_string_lossy().to_string());
        map.configure_world_maps("Map", &fs);
        assert_eq!(map.get_zones(0, 0, BiomeMapType::Zone).expect("zones").expect("image"), vec![20; 64]);
    }

    #[test]
    fn cache_clears_before_the_eighteenth_distinct_insert() {
        let mut map = BiomeMap::new(empty_map());
        for x in 0..17 {
            map.set_raster(x, 0, None);
        }
        assert_eq!(map.cache.lock().expect("cache").len(), 17);
        map.set_raster(17, 0, None);
        let cache = map.cache.lock().expect("cache");
        assert_eq!(cache.len(), 1);
        assert!(cache.contains_key(&(17, 0)));
    }

    #[test]
    fn mod_file_index_overrides_vanilla_png() {
        let root = Fixture::new();
        root.raster("Map", 0, 0, 1, 2);
        let override_file = root.raster("ModMap", 0, 0, 42, 43);
        let mut fs = ZomboidFileSystem::init(&root.0).expect("index files");
        fs.active_file_map.insert("media/maps/map/maps/biomemap_0_0.png".into(), override_file.to_string_lossy().to_string());
        let map = BiomeMap::new_with_filesystem(empty_map(), "Map", &fs);
        assert_eq!(map.get_zones(0, 0, BiomeMapType::Biome).expect("biome").expect("image"), vec![42; 64]);
    }

    #[test]
    fn corrupt_first_map_does_not_fall_through_to_later_maps() {
        let root = Fixture::new();
        let bad = root.raster("First", 0, 0, 1, 2);
        std::fs::write(bad, b"not a PNG").expect("corrupt image");
        root.raster("Second", 0, 0, 90, 91);
        let fs = ZomboidFileSystem::init(&root.0).expect("index files");
        let map = BiomeMap::new_with_filesystem(empty_map(), "First;Second", &fs);
        assert_eq!(
            map.get_cell(0, 0, BiomeMapType::Biome).unwrap_err(),
            "java.lang.ArrayIndexOutOfBoundsException: Invalid coordinates."
        );
        assert_eq!(
            map.get_cell(0, 0, BiomeMapType::Zone).unwrap_err(),
            "java.lang.ArrayIndexOutOfBoundsException: Invalid coordinates."
        );
    }
}
