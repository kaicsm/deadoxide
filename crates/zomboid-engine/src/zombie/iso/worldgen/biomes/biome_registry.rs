//! zombie.iso.worldgen.biomes.BiomeRegistry: the noise thresholds and the pick among the biomes whose
//! parameter sets contain the sampled values.

use crate::java::JavaHashMap;

use super::biome::Biome;
use super::biome_noise::BiomeNoise;
use super::biome_type::{BiomeEnum, Bush, Hygrometry, Landscape, OreLevel, Plant, Temperature};

/// `Map<T, List<Double>>` as loadSelection returns: the `[low, high)` thresholds of one parameter. The
/// original's HashMap has enum keys (their identity hashes differ per run, so its iteration order is not
/// reproducible); nothing iterates it, so the port keeps the members in ordinal order.
#[derive(Clone, Debug)]
pub struct Selection<T: BiomeEnum> {
    entries: Vec<(T, Vec<f64>)>,
}

impl<T: BiomeEnum> Selection<T> {
    /// The members in ordinal order (a deterministic stand-in for the map; lookups are by key either way).
    pub fn from_entries(entries: Vec<(T, Vec<f64>)>) -> Self {
        let mut entries = entries;
        entries.sort_by_key(|(k, _)| k.ordinal());
        Selection { entries }
    }

    /// `map.get(key)`.
    pub fn get(&self, key: T) -> Option<&[f64]> {
        self.entries
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, v)| v.as_slice())
    }

    /// The members, in ordinal order.
    pub fn iter(&self) -> impl Iterator<Item = (T, &[f64])> + '_ {
        self.entries.iter().map(|(k, v)| (*k, v.as_slice()))
    }
}

/// What `BiomeRegistry.get` picked: one of the passed-in biomes, by name, or the DEFAULT_BIOME. The port
/// identifies the biome by name (the map's keys are unique names) instead of handing out a shared object.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BiomeSelection {
    Default,
    Named(String),
}

impl BiomeSelection {
    /// `IBiome.name()`.
    pub fn name(&self) -> &str {
        match self {
            BiomeSelection::Default => "DEFAULT",
            BiomeSelection::Named(n) => n,
        }
    }
}

/// `BiomeRegistry.BiomeGetter` record (the cache key).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct BiomeGetter {
    landscape: Landscape,
    plant: Plant,
    bush: Bush,
    temperature: Temperature,
    hygrometry: Hygrometry,
    ore_level: OreLevel,
}

/// `BiomeRegistry.BiomeGetterFiltered` record (the filtered cache key).
#[derive(Clone, PartialEq, Eq, Debug)]
struct BiomeGetterFiltered {
    filter: String,
    bush: Bush,
    ore_level: OreLevel,
}

/// zombie.iso.worldgen.biomes.BiomeRegistry. The original is the static `instance` (a mutable singleton with
/// `reset()`); the port makes one value the world generation owns. The two caches only memoize lists that
/// depend on the passed-in biomes map, so they are a linear list here (a fresh registry behaves identically).
#[derive(Default)]
pub struct BiomeRegistry {
    biome_cache: Vec<(BiomeGetter, Vec<BiomeSelection>)>,
    biome_cache_filtered: Vec<(BiomeGetterFiltered, Vec<BiomeSelection>)>,
}

/// `prob.get(key).get(0)`: the original throws NullPointerException when the map lacks the key (a null list)
/// and IndexOutOfBoundsException when the list is empty - malformed selection tables the port reports as
/// errors instead of guessing.
fn threshold_low<T: BiomeEnum>(prob: &Selection<T>, key: T, what: &str) -> Result<f64, String> {
    let Some(list) = prob.get(key) else {
        return Err(format!(
            "NullPointerException: selection {what} lacks {}",
            key.name()
        ));
    };
    list.first().copied().ok_or_else(|| {
        format!(
            "IndexOutOfBoundsException: selection {what} {} is empty",
            key.name()
        )
    })
}

/// `prob.get(key).get(1)` (only read when the low comparison passed, as the original's `&&` does).
fn threshold_high<T: BiomeEnum>(prob: &Selection<T>, key: T, what: &str) -> Result<f64, String> {
    let Some(list) = prob.get(key) else {
        return Err(format!(
            "NullPointerException: selection {what} lacks {}",
            key.name()
        ));
    };
    list.get(1).copied().ok_or_else(|| {
        format!(
            "IndexOutOfBoundsException: selection {what} {} has {} elements",
            key.name(),
            list.len()
        )
    })
}

/// `noise >= prob.get(key).get(0) && noise < prob.get(key).get(1)` with the original's evaluation order: the
/// high end only when the low comparison passed.
fn in_range<T: BiomeEnum>(
    noise: f64,
    prob: &Selection<T>,
    key: T,
    what: &str,
) -> Result<bool, String> {
    Ok(noise >= threshold_low(prob, key, what)? && noise < threshold_high(prob, key, what)?)
}

impl BiomeRegistry {
    /// `BiomeRegistry.reset()`.
    pub fn reset(&mut self) {
        self.biome_cache.clear();
        self.biome_cache_filtered.clear();
    }

    /// zombie.iso.worldgen.biomes.BiomeRegistry.get(Map, BiomeNoise, double, six selections): the six noise
    /// fields to parameter values, then `biomes.get((int)(selector * biomes.size()))` among the biomes whose
    /// parameter sets contain them (the biomes map's own iteration order), DEFAULT_BIOME when none matches.
    #[allow(clippy::too_many_arguments)]
    pub fn get(
        &mut self,
        biomes_in: &JavaHashMap<Biome>,
        noises: BiomeNoise,
        selector: f64,
        landscape_prob: &Selection<Landscape>,
        plant_prob: &Selection<Plant>,
        bush_prob: &Selection<Bush>,
        temperature_prob: &Selection<Temperature>,
        hygrometry_prob: &Selection<Hygrometry>,
        ore_level_prob: &Selection<OreLevel>,
    ) -> Result<BiomeSelection, String> {
        let landscape = Self::get_landscape(noises.landscape, landscape_prob)?;
        let plant = Self::get_plant(noises.plant, plant_prob)?;
        let bush = Self::get_bush(noises.bush, bush_prob)?;
        let temperature = Self::get_temperature(noises.temperature, temperature_prob)?;
        let hygrometry = Self::get_hygrometry(noises.hygrometry, hygrometry_prob)?;
        let ore_level = Self::get_ore(noises.ore, ore_level_prob)?;
        let getter = BiomeGetter {
            landscape,
            plant,
            bush,
            temperature,
            hygrometry,
            ore_level,
        };
        let biomes = match self.biome_cache.iter().position(|(k, _)| *k == getter) {
            Some(i) => self.biome_cache[i].1.clone(),
            None => {
                let list: Vec<BiomeSelection> = biomes_in
                    .values()
                    .filter(|b| b.landscape.contains(landscape))
                    .filter(|b| b.plant.contains(plant))
                    .filter(|b| b.bush.contains(bush))
                    .filter(|b| b.temperature.contains(temperature))
                    .filter(|b| b.hygrometry.contains(hygrometry))
                    .filter(|b| b.ore_level.contains(ore_level))
                    .map(|b| BiomeSelection::Named(b.name.clone()))
                    .collect();
                self.biome_cache.push((getter, list.clone()));
                list
            }
        };
        Self::pick(&biomes, selector)
    }

    /// BiomeRegistry.get(Map, String filter, BiomeNoise, double, bush, oreLevel): the same pick among the
    /// biomes whose name starts with `filter`; `None` is the original's null for a null filter.
    pub fn get_filtered(
        &mut self,
        biomes_in: &JavaHashMap<Biome>,
        filter: Option<&str>,
        noises: BiomeNoise,
        selector: f64,
        bush_prob: &Selection<Bush>,
        ore_level_prob: &Selection<OreLevel>,
    ) -> Result<Option<BiomeSelection>, String> {
        let Some(filter) = filter else {
            return Ok(None);
        };
        let bush = Self::get_bush(noises.bush, bush_prob)?;
        let ore_level = Self::get_ore(noises.ore, ore_level_prob)?;
        let getter = BiomeGetterFiltered {
            filter: filter.to_string(),
            bush,
            ore_level,
        };
        let biomes = match self
            .biome_cache_filtered
            .iter()
            .position(|(k, _)| *k == getter)
        {
            Some(i) => self.biome_cache_filtered[i].1.clone(),
            None => {
                let list: Vec<BiomeSelection> = biomes_in
                    .values()
                    .filter(|b| b.name.starts_with(filter))
                    .filter(|b| b.bush.contains(bush))
                    .filter(|b| b.ore_level.contains(ore_level))
                    .map(|b| BiomeSelection::Named(b.name.clone()))
                    .collect();
                self.biome_cache_filtered.push((getter, list.clone()));
                list
            }
        };
        Ok(Some(Self::pick(&biomes, selector)?))
    }

    /// `biomes.isEmpty() ? Biome.DEFAULT_BIOME : biomes.get((int)(selector * biomes.size()))`: an index past
    /// the end is the original's IndexOutOfBoundsException, not a silent default.
    fn pick(biomes: &[BiomeSelection], selector: f64) -> Result<BiomeSelection, String> {
        if biomes.is_empty() {
            return Ok(BiomeSelection::Default);
        }
        let index = (selector * biomes.len() as f64) as i32 as usize;
        biomes
            .get(index)
            .cloned()
            .ok_or_else(|| format!("IndexOutOfBoundsException: Index {index} out of bounds"))
    }

    /// BiomeRegistry.getLandscape.
    fn get_landscape(noise: f64, prob: &Selection<Landscape>) -> Result<Landscape, String> {
        if in_range(noise, prob, Landscape::Forest, "landscape")? {
            Ok(Landscape::Forest)
        } else if in_range(noise, prob, Landscape::LightForest, "landscape")? {
            Ok(Landscape::LightForest)
        } else if in_range(noise, prob, Landscape::Plain, "landscape")? {
            Ok(Landscape::Plain)
        } else {
            Ok(Landscape::None)
        }
    }

    /// BiomeRegistry.getPlant.
    fn get_plant(noise: f64, prob: &Selection<Plant>) -> Result<Plant, String> {
        if in_range(noise, prob, Plant::Flower, "plant")? {
            Ok(Plant::Flower)
        } else if in_range(noise, prob, Plant::Grass, "plant")? {
            Ok(Plant::Grass)
        } else {
            Ok(Plant::None)
        }
    }

    /// BiomeRegistry.getBush.
    fn get_bush(noise: f64, prob: &Selection<Bush>) -> Result<Bush, String> {
        if in_range(noise, prob, Bush::Dry, "bush")? {
            Ok(Bush::Dry)
        } else if in_range(noise, prob, Bush::Regular, "bush")? {
            Ok(Bush::Regular)
        } else if in_range(noise, prob, Bush::Fat, "bush")? {
            Ok(Bush::Fat)
        } else {
            Ok(Bush::None)
        }
    }

    /// BiomeRegistry.getTemperature.
    fn get_temperature(noise: f64, prob: &Selection<Temperature>) -> Result<Temperature, String> {
        if in_range(noise, prob, Temperature::Hot, "temperature")? {
            Ok(Temperature::Hot)
        } else if in_range(noise, prob, Temperature::Medium, "temperature")? {
            Ok(Temperature::Medium)
        } else if in_range(noise, prob, Temperature::Cold, "temperature")? {
            Ok(Temperature::Cold)
        } else {
            Ok(Temperature::None)
        }
    }

    /// BiomeRegistry.getHygrometry.
    fn get_hygrometry(noise: f64, prob: &Selection<Hygrometry>) -> Result<Hygrometry, String> {
        if in_range(noise, prob, Hygrometry::Flooding, "hygrometry")? {
            Ok(Hygrometry::Flooding)
        } else if in_range(noise, prob, Hygrometry::Rain, "hygrometry")? {
            Ok(Hygrometry::Rain)
        } else if in_range(noise, prob, Hygrometry::Dry, "hygrometry")? {
            Ok(Hygrometry::Dry)
        } else {
            Ok(Hygrometry::None)
        }
    }

    /// BiomeRegistry.getOre.
    fn get_ore(noise: f64, prob: &Selection<OreLevel>) -> Result<OreLevel, String> {
        if in_range(noise, prob, OreLevel::VeryHigh, "ore_level")? {
            Ok(OreLevel::VeryHigh)
        } else if in_range(noise, prob, OreLevel::High, "ore_level")? {
            Ok(OreLevel::High)
        } else if in_range(noise, prob, OreLevel::Medium, "ore_level")? {
            Ok(OreLevel::Medium)
        } else if in_range(noise, prob, OreLevel::Low, "ore_level")? {
            Ok(OreLevel::Low)
        } else if in_range(noise, prob, OreLevel::VeryLow, "ore_level")? {
            Ok(OreLevel::VeryLow)
        } else {
            Ok(OreLevel::None)
        }
    }
}
