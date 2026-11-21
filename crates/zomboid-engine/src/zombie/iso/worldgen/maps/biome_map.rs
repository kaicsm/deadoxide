//! zombie.iso.worldgen.maps.BiomeMap: biome/zone lookups over cached per-cell rasters.

use std::collections::HashMap;

use crate::java::JavaIntHashMap;

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

/// BiomeMap: Java's 16-entry CacheMap plus the table WorldGenReader.loadBiomeMapConfig returns.
/// The original searches `media/maps/<map>/maps/biomemap_cx_cy.png` and decodes ImageData; the
/// caller supplies that decoded raster through `set_raster` until the engine has an image loader.
#[derive(Debug)]
pub struct BiomeMap {
    pub zone_map: JavaIntHashMap<BiomeMapEntry>,
    cache: HashMap<(i32, i32), Option<BiomeRaster>>,
    pub foraging_zones: Vec<Option<String>>,
}

impl BiomeMap {
    /// BiomeMap.<init>: the foraging zone list preserves zoneMap.entrySet order.
    pub fn new(zone_map: JavaIntHashMap<BiomeMapEntry>) -> Self {
        let foraging_zones = zone_map.values().map(|e| e.zone.clone()).collect();
        Self { zone_map, cache: HashMap::new(), foraging_zones }
    }

    /// BiomeMap.getRaster: insert the raster associated with this world cell, or None when absent.
    /// The underlying CacheMap clears once its size *before* insertion exceeds 16.
    pub fn set_raster(&mut self, cell_x: i32, cell_y: i32, raster: Option<BiomeRaster>) {
        if self.cache.len() > 16 {
            self.cache.clear();
        }
        self.cache.insert((cell_x, cell_y), raster);
    }

    /// BiomeMap.getRaster: a previously supplied image for that cell.
    pub fn get_raster(&self, cell_x: i32, cell_y: i32) -> Option<&BiomeRaster> {
        self.cache.get(&(cell_x, cell_y)).and_then(Option::as_ref)
    }

    /// BiomeMap.getCell: return all 256x256 samples from the chosen band.
    pub fn get_cell(&self, cell_x: i32, cell_y: i32, kind: BiomeMapType) -> Result<Option<Vec<i32>>, String> {
        self.get_raster(cell_x, cell_y).map(|r| r.get_samples(0, 0, 256, 256, kind.id())).transpose()
    }

    /// BiomeMap.getZones: Java integer division by 32 truncates toward zero; negative chunks
    /// are rejected when their resulting local offset would be negative.
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

    /// BiomeMap.getEntry: map a raster's channel value to the Lua-configured biome, ore and zone.
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
}
