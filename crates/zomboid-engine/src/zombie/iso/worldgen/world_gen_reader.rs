//! zombie.iso.worldgen.WorldGenReader: the Lua tables under `worldgen` (the game's own
//! `media/lua/server/WorldGen/*.lua`) into the biome registry's inputs - loadBiomes with its parent merge,
//! loadSelection, loadPriorities. The roads, veins, static modules and biome map parsers are not ported yet.

use crate::java::{self, JavaHashMap};
use crate::se::krka::kahlua::j2se::kahlua_table_impl::KahluaTableImpl;
use crate::se::krka::kahlua::vm::object::{cast_string, rstr, Object, TableRef, Throwable, R};
use crate::zombie::sandbox_options::SandboxOptions;

use super::biomes::biome::{Biome, BiomeFeatures, Placements, SubBiomeInner, SubBiomes};
use super::biomes::biome_registry::Selection;
use super::biomes::biome_type::{
    BiomeEnum, Bush, EnumSet, Hygrometry, Landscape, OreLevel, Plant, Temperature,
};
use super::biomes::feature::Feature;
use super::biomes::feature_type::FeatureType;
use super::biomes::grass::Grass;
use super::biomes::probability::Probability;
use super::biomes::tile_group::TileGroup;

/// `(KahluaTableImpl) value`: a table, else the ClassCastException.
fn cast_table(o: &Object) -> R<TableRef> {
    match o {
        Object::Table(t) => Ok(t.clone()),
        _ => Err(Throwable::class_cast(
            o,
            "se.krka.kahlua.j2se.KahluaTableImpl",
        )),
    }
}

/// `(KahluaTableImpl) rawget(..)` where the original keeps a null (a nil value is null, anything else a
/// non-table value is the ClassCastException).
fn cast_table_opt(o: &Object) -> R<Option<TableRef>> {
    match o {
        Object::Nil => Ok(None),
        Object::Table(t) => Ok(Some(t.clone())),
        o => Err(Throwable::class_cast(
            o,
            "se.krka.kahlua.j2se.KahluaTableImpl",
        )),
    }
}

/// `table.rawget(key)` where a nil result is the NullPointerException the original throws using it.
fn rawget_table_or_npe(table: &TableRef, key: &str) -> R<TableRef> {
    match KahluaTableImpl::rawget(table, &Object::str(key))? {
        Object::Nil => Err(Throwable::npe(key)),
        o => cast_table(&o),
    }
}

/// `(String) value`: the string or null (nil), else the ClassCastException.
fn cast_str(o: &Object) -> R<Option<String>> {
    Ok(cast_string(o)?.map(|s| rstr(&s)))
}

/// zombie.iso.worldgen.WorldGenReader. Stateless in the original; the port carries the sandbox options
/// `ProbaString` reads (the original reads the `SandboxOptions.instance` static at parse time).
pub struct WorldGenReader<'a> {
    pub sandbox: &'a SandboxOptions,
}

impl WorldGenReader<'_> {
    /// WorldGenReader.loadBiomes(KahluaTable, String): every biome of the table (the Kahlua iteration order
    /// into a java.util.HashMap), then the parent merge of `forEach` - each biome, in the HashMap's own
    /// iteration order, walks to its parentless ancestor and rebuilds from the chain. The merge builds a
    /// FRESH features and replacements map per merged biome (`new HashMap<>()`, then `putAll` of every chain
    /// member's own map, root first, later members overriding earlier ones); the original never aliases a
    /// member's map into the result. A biome that merges may read an already-merged parent, so the port
    /// applies the merges in the same order it iterates the keys.
    pub fn load_biomes(&self, worldgen_table: &TableRef, table_key: &str) -> R<JavaHashMap<Biome>> {
        let main_table = rawget_table_or_npe(worldgen_table, table_key)?;
        let mut biomes: JavaHashMap<Biome> = JavaHashMap::default();
        for (biome_name, biome_table) in KahluaTableImpl::iterate_snapshot(&main_table) {
            let name = biome_name.java_to_string();
            let table = cast_table(&biome_table)?;
            let biome = self.load_biome(&name, &table)?;
            biomes.put(name, biome);
        }
        // biomes.forEach((k, v) -> merge): the keys in the map's iteration order; the merge puts back into
        // the same map (an existing key keeps its position, so the order does not change).
        let keys: Vec<String> = biomes.iter().map(|(k, _)| k.to_string()).collect();
        for k in keys {
            let Some(v) = biomes.get(&k).cloned() else {
                return Err(Throwable::npe(format!("biomes lacks {k}")));
            };
            // biomeList: this biome, then each parent prepended until the parentless ancestor.
            let mut chain: Vec<Biome> = vec![v.clone()];
            while let Some(parent_name) = chain[0].parent.clone() {
                if parent_name.is_empty() {
                    break;
                }
                let Some(parent) = biomes.get(&parent_name).cloned() else {
                    // biomes.get(parent) returned null: the next loop's biome.parent() is the NPE
                    return Err(Throwable::npe(format!("biomes lacks parent {parent_name}")));
                };
                chain.insert(0, parent);
                // a cyclic parent chain grows without end in the original (until memory runs out); the
                // port reports it instead
                if chain.len() > 10_000 {
                    return Err(Throwable::runtime(format!(
                        "parent chain of {k} does not terminate"
                    )));
                }
            }
            // the lambda's `return` for a biome without parents: it keeps what loadBiome parsed
            if chain.len() == 1 {
                continue;
            }
            let mut features = BiomeFeatures::default();
            // Map<String, List<Feature>> replacements = new HashMap<>(): a FRESH map per merged biome, putAll
            // of every chain member's map in chain order (the first putAll pre-sizes the table, the later
            // ones put one by one - JavaHashMap::put_all models both)
            let mut replacements: JavaHashMap<Vec<Feature>> = JavaHashMap::default();
            let mut landscape = chain[0].landscape.clone();
            let mut plant = chain[0].plant.clone();
            let mut bush = chain[0].bush.clone();
            let mut temperature = chain[0].temperature.clone();
            let mut hygrometry = chain[0].hygrometry.clone();
            let mut ore_level = chain[0].ore_level.clone();
            let mut placements = chain[0].placements.clone();
            let mut sub_biomes = chain[0].sub_biomes.clone();
            let mut protected_list = chain[0].protected_list.clone();
            let mut zombies = chain[0].zombies;
            let mut grass = chain[0].grass.clone();
            for biomex in &chain {
                if let Some(biome_features) = &biomex.features {
                    for feature_type in FeatureType::ALL {
                        if let Some(list) = biome_features.get(*feature_type) {
                            features.put(*feature_type, list.clone());
                        }
                    }
                }
                if let Some(biome_replacements) = &biomex.replacements {
                    // replacements.putAll(biomeReplacements)
                    replacements.put_all(biome_replacements);
                }
                if !biomex.landscape.is_all_of() {
                    landscape = biomex.landscape.clone();
                }
                if !biomex.plant.is_all_of() {
                    plant = biomex.plant.clone();
                }
                if !biomex.bush.is_all_of() {
                    bush = biomex.bush.clone();
                }
                if !biomex.temperature.is_all_of() {
                    temperature = biomex.temperature.clone();
                }
                if !biomex.hygrometry.is_all_of() {
                    hygrometry = biomex.hygrometry.clone();
                }
                if !biomex.ore_level.is_all_of() {
                    ore_level = biomex.ore_level.clone();
                }
                // placements() of a loaded biome is never empty (loadPlacements puts all five types), so
                // the original's !isEmpty() test only rejects null
                if biomex.placements.is_some() {
                    placements = biomex.placements.clone();
                }
                if !biomex.sub_biomes.is_empty() {
                    sub_biomes = biomex.sub_biomes.clone();
                }
                if !biomex.protected_list.is_empty() {
                    protected_list = biomex.protected_list.clone();
                }
                if biomex.zombies >= 0.0 {
                    zombies = biomex.zombies;
                }
                grass = biomex.grass.clone();
            }
            let merged = Biome::new(
                k.clone(),
                v.parent.clone(),
                v.generate,
                Some(features),
                Some(replacements),
                Some(landscape),
                Some(plant),
                Some(bush),
                Some(temperature),
                Some(hygrometry),
                Some(ore_level),
                zombies,
                placements,
                sub_biomes,
                protected_list,
                grass,
            );
            biomes.put(k, merged);
        }
        Ok(biomes)
    }

    /// WorldGenReader.loadBiome(String, KahluaTable): one biome table (the caller owns the null case the
    /// original answers with null).
    pub fn load_biome(&self, biome_name: &str, biome_table: &TableRef) -> R<Biome> {
        let parent = cast_str(&KahluaTableImpl::rawget(
            biome_table,
            &Object::str("parent"),
        )?)?;
        let features = self.load_biome_features(
            biome_name,
            cast_table_opt(&KahluaTableImpl::rawget(
                biome_table,
                &Object::str("features"),
            )?)?,
        )?;
        let replacements = self.load_replacements(cast_table_opt(&KahluaTableImpl::rawget(
            biome_table,
            &Object::str("replacements"),
        )?)?)?;
        // a missing params table is the NPE of paramsTable.rawget on a null table
        let params_table = rawget_table_or_npe(biome_table, "params")?;
        let params = &params_table;
        let landscape = self.load_biome_type(
            Landscape::None,
            KahluaTableImpl::rawget(params, &Object::str("landscape"))?,
        )?;
        let plant = self.load_biome_type(
            Plant::None,
            KahluaTableImpl::rawget(params, &Object::str("plant"))?,
        )?;
        let bush = self.load_biome_type(
            Bush::None,
            KahluaTableImpl::rawget(params, &Object::str("bush"))?,
        )?;
        let temperature = self.load_biome_type(
            Temperature::None,
            KahluaTableImpl::rawget(params, &Object::str("temperature"))?,
        )?;
        let hygrometry = self.load_biome_type(
            Hygrometry::None,
            KahluaTableImpl::rawget(params, &Object::str("hygrometry"))?,
        )?;
        let ore_level = self.load_biome_type(
            OreLevel::None,
            KahluaTableImpl::rawget(params, &Object::str("ore_level"))?,
        )?;
        let placements = self.load_placements(cast_table_opt(&KahluaTableImpl::rawget(
            params,
            &Object::str("placements"),
        )?)?)?;
        let sub_biomes = self.load_sub_biomes(cast_table_opt(&KahluaTableImpl::rawget(
            params,
            &Object::str("subbiomes"),
        )?)?)?;
        let protected_list = Self::load_list_strings(cast_table_opt(&KahluaTableImpl::rawget(
            params,
            &Object::str("protected"),
        )?)?)?;
        let grass = match cast_table_opt(&KahluaTableImpl::rawget(params, &Object::str("grass"))?)?
        {
            Some(grass_table) => {
                let fern_chance = self.load_double(
                    &KahluaTableImpl::rawget(&grass_table, &Object::str("fernChance"))?,
                    0.7,
                )? as f32;
                let no_grass_div = self.load_double(
                    &KahluaTableImpl::rawget(&grass_table, &Object::str("noGrassDiv"))?,
                    3.0,
                )? as f32;
                let mut no_grass_stages = Self::load_list_doubles(cast_table_opt(
                    &KahluaTableImpl::rawget(&grass_table, &Object::str("noGrassStages"))?,
                )?)?;
                if no_grass_stages.is_empty() {
                    no_grass_stages = vec![0.4];
                }
                let mut grass_stages = Self::load_list_doubles(cast_table_opt(
                    &KahluaTableImpl::rawget(&grass_table, &Object::str("grassStages"))?,
                )?)?;
                if grass_stages.is_empty() {
                    grass_stages = vec![0.33, 0.5];
                }
                Grass {
                    fern_chance,
                    no_grass_div,
                    no_grass_stages,
                    grass_stages,
                }
            }
            None => Grass {
                // the literal default of loadBiome's else branch (not Grass.DEFAULT)
                fern_chance: 0.7,
                no_grass_div: 3.0,
                no_grass_stages: vec![0.4],
                grass_stages: vec![0.33, 0.5],
            },
        };
        let zombies = self.load_double(
            &KahluaTableImpl::rawget(params, &Object::str("zombies"))?,
            -1.0,
        )? as f32;
        let generate = self.load_boolean(
            &KahluaTableImpl::rawget(params, &Object::str("generate"))?,
            true,
        )?;
        Ok(Biome::new(
            biome_name.to_string(),
            parent,
            generate,
            features,
            replacements,
            landscape,
            plant,
            bush,
            temperature,
            hygrometry,
            ore_level,
            zombies,
            placements,
            sub_biomes,
            protected_list,
            Some(grass),
        ))
    }

    /// WorldGenReader.loadSelection&lt;T&gt;(T, KahluaTable, String): the `worldgen.selection` thresholds of
    /// one parameter (`BiomeType.keys` indexes the table by the enum's class).
    pub fn load_selection<T: BiomeEnum>(
        &self,
        worldgen_table: &TableRef,
        table_key: &str,
    ) -> R<Selection<T>> {
        let main_table = rawget_table_or_npe(worldgen_table, table_key)?;
        // mainTable.rawget(keys.get(..)) on a missing selection table is the NPE the original throws
        let inner_table = rawget_table_or_npe(&main_table, T::KEY)?;
        let mut entries = Vec::new();
        for (key, value) in KahluaTableImpl::iterate_snapshot(&inner_table) {
            let name =
                cast_str(&key)?.ok_or_else(|| Throwable::class_cast(&key, "java.lang.String"))?;
            let value_enum = T::from_name(&name)
                .ok_or_else(|| Throwable::runtime(format!("No enum constant {name}")))?;
            let list = Self::load_list_doubles(Some(cast_table(&value)?))?;
            entries.push((value_enum, list));
        }
        Ok(Selection::from_entries(entries))
    }

    /// WorldGenReader.loadPriorities(KahluaTable, String): the tile-priority numbers, keyed by tile name
    /// (the Lua table is number -> name; the map inverts it).
    pub fn load_priorities(
        &self,
        worldgen_table: &TableRef,
        table_key: &str,
    ) -> R<JavaHashMap<f64>> {
        let priority_table = rawget_table_or_npe(worldgen_table, table_key)?;
        let mut priorities: JavaHashMap<f64> = JavaHashMap::default();
        for (key, value) in KahluaTableImpl::iterate_snapshot(&priority_table) {
            let number = match key {
                Object::Number(d) => d,
                o => return Err(Throwable::class_cast(&o, "java.lang.Double")),
            };
            let name = cast_str(&value)?
                .ok_or_else(|| Throwable::class_cast(&value, "java.lang.String"))?;
            priorities.put(name, number);
        }
        Ok(priorities)
    }

    /// WorldGenReader.loadBiomeFeatures(String, KahluaTable): the feature lists per type; `None` is the
    /// original's null for a missing features table.
    fn load_biome_features(
        &self,
        name: &str,
        main_table: Option<TableRef>,
    ) -> R<Option<BiomeFeatures>> {
        let Some(main_table) = main_table else {
            return Ok(None);
        };
        let mut types = BiomeFeatures::default();
        for (type_key, type_value) in KahluaTableImpl::iterate_snapshot(&main_table) {
            let type_name = cast_str(&type_key)?
                .ok_or_else(|| Throwable::class_cast(&type_key, "java.lang.String"))?;
            let type_table = cast_table(&type_value)?;
            let mut features = Vec::new();
            for (_, entry_value) in KahluaTableImpl::iterate_snapshot(&type_table) {
                let entry = cast_table(&entry_value)?;
                features.push(self.feature_of(name, &type_name, &entry)?);
            }
            let feature_type = FeatureType::from_name(&type_name).ok_or_else(|| {
                Throwable::runtime(format!("No enum constant FeatureType.{type_name}"))
            })?;
            types.put(feature_type, features);
        }
        Ok(Some(types))
    }

    /// The `{f = {main = ..}, p = ..}` entry both loadBiomeFeatures and loadReplacements inline (the
    /// original writes the block twice; the port shares it). The malformed-data branch the original builds
    /// a reflection message for is the error the port reports.
    fn feature_of(&self, name: &str, type_name: &str, entry: &TableRef) -> R<Feature> {
        let features_obj = KahluaTableImpl::rawget(entry, &Object::str("f"))?;
        let proba_obj = KahluaTableImpl::rawget(entry, &Object::str("p"))?;
        if features_obj.is_nil() || proba_obj.is_nil() {
            return Err(Throwable::runtime(format!(
                "Features not found or probability absents | {name} | {type_name}"
            )));
        }
        let features_table = cast_table(&features_obj)?;
        let main = rawget_table_or_npe(&features_table, "main")?;
        let tile_groups = self.load_features(&main)?;
        let probability = match proba_obj {
            Object::Number(d) => Probability::proba_double(d),
            Object::String(s) => {
                Probability::proba_string(&rstr(&s), self.sandbox).map_err(Throwable::runtime)?
            }
            _ => return Err(Throwable::runtime("Unsupported probability type")),
        };
        let mut min_size = i32::MAX;
        let mut max_size = 0;
        for tile_group in &tile_groups {
            min_size = min_size.min(tile_group.sx).min(tile_group.sy);
            max_size = max_size.max(tile_group.sx).max(tile_group.sy);
        }
        Ok(Feature {
            tile_groups,
            min_size,
            max_size,
            probability,
        })
    }

    /// WorldGenReader.loadFeatures(KahluaTable): the tile groups of one feature - a string is the 1x1
    /// group, a table a row list (`sy` counts the rows, `sx` is the size of the last row, whatever the
    /// others are).
    fn load_features(&self, main_table: &TableRef) -> R<Vec<TileGroup>> {
        let mut features = Vec::new();
        for (_, value) in KahluaTableImpl::iterate_snapshot(main_table) {
            match value {
                Object::String(s) => features.push(TileGroup {
                    sx: 1,
                    sy: 1,
                    tiles: vec![rstr(&s)],
                }),
                Object::Table(kahlua_table) => {
                    let mut sx = 0;
                    let mut sy = 0;
                    let mut tiles = Vec::new();
                    for (_, row_value) in KahluaTableImpl::iterate_snapshot(&kahlua_table) {
                        sy += 1;
                        let row = cast_table(&row_value)?;
                        let row_tiles = Self::load_list_strings(Some(row))?;
                        sx = row_tiles.len() as i32;
                        tiles.extend(row_tiles);
                    }
                    features.push(TileGroup { sx, sy, tiles });
                }
                _ => return Err(Throwable::runtime("Only strings and tables in there!")),
            }
        }
        Ok(features)
    }

    /// WorldGenReader.loadReplacements(KahluaTable): the tile replacements (an empty map for a missing
    /// table, never null - only Biome.DEFAULT_BIOME passes null).
    fn load_replacements(
        &self,
        replacements_table: Option<TableRef>,
    ) -> R<Option<JavaHashMap<Vec<Feature>>>> {
        let mut replacements: JavaHashMap<Vec<Feature>> = JavaHashMap::default();
        if let Some(replacements_table) = replacements_table {
            for (key, value) in KahluaTableImpl::iterate_snapshot(&replacements_table) {
                let key_name = cast_str(&key)?
                    .ok_or_else(|| Throwable::class_cast(&key, "java.lang.String"))?;
                let value_table = cast_table(&value)?;
                let mut features = Vec::new();
                for (_, entry_value) in KahluaTableImpl::iterate_snapshot(&value_table) {
                    let entry = cast_table(&entry_value)?;
                    features.push(self.feature_of("", &key_name, &entry)?);
                }
                replacements.put(key_name, features);
            }
        }
        Ok(Some(replacements))
    }

    /// WorldGenReader.loadPlacements(KahluaTable): the tiles each feature type may sit on - GENERIC first,
    /// then the type's own list.
    fn load_placements(&self, placements_table: Option<TableRef>) -> R<Option<Placements>> {
        let Some(placements_table) = placements_table else {
            return Ok(None);
        };
        let mut map: JavaHashMap<Vec<String>> = JavaHashMap::default();
        for (key, value) in KahluaTableImpl::iterate_snapshot(&placements_table) {
            let type_name =
                cast_str(&key)?.ok_or_else(|| Throwable::class_cast(&key, "java.lang.String"))?;
            let list = Self::load_list_strings(Some(cast_table(&value)?))?;
            map.put(type_name, list);
        }
        let mut placements = Placements::default();
        for feature_type in FeatureType::ALL {
            if let Some(generic) = map.get("GENERIC") {
                placements.by_type[feature_type.index()].extend_from_slice(generic);
            }
            if let Some(own) = map.get(feature_type.name()) {
                placements.by_type[feature_type.index()].extend_from_slice(own);
            }
        }
        Ok(Some(placements))
    }

    /// WorldGenReader.loadSubBiomes(KahluaTable): feature type -> feature type -> the inline biomes (named
    /// by their Lua number key, `Double.toString`).
    fn load_sub_biomes(&self, sub_biomes_table: Option<TableRef>) -> R<SubBiomes> {
        let Some(sub_biomes_table) = sub_biomes_table else {
            return Ok(SubBiomes::default());
        };
        let mut sub_biomes = SubBiomes::default();
        for (type_key, type_value) in KahluaTableImpl::iterate_snapshot(&sub_biomes_table) {
            let type_name = cast_str(&type_key)?
                .ok_or_else(|| Throwable::class_cast(&type_key, "java.lang.String"))?;
            let outer_type = FeatureType::from_name(&type_name).ok_or_else(|| {
                Throwable::runtime(format!("No enum constant FeatureType.{type_name}"))
            })?;
            let sub_type_table = cast_table(&type_value)?;
            let mut sub_type_map = SubBiomeInner::default();
            for (sub_key, sub_value) in KahluaTableImpl::iterate_snapshot(&sub_type_table) {
                let sub_name = cast_str(&sub_key)?
                    .ok_or_else(|| Throwable::class_cast(&sub_key, "java.lang.String"))?;
                let sub_type = FeatureType::from_name(&sub_name).ok_or_else(|| {
                    Throwable::runtime(format!("No enum constant FeatureType.{sub_name}"))
                })?;
                let inner_table = cast_table(&sub_value)?;
                let mut inner_biomes = Vec::new();
                for (biome_key, biome_value) in KahluaTableImpl::iterate_snapshot(&inner_table) {
                    let number = match biome_key {
                        Object::Number(d) => d,
                        o => return Err(Throwable::class_cast(&o, "java.lang.Double")),
                    };
                    let name = java::double_to_string(number);
                    let biome = self.load_biome(&name, &cast_table(&biome_value)?)?;
                    inner_biomes.push(biome);
                }
                sub_type_map.by_type[sub_type.index()] = Some(inner_biomes);
            }
            sub_biomes.put(outer_type, sub_type_map);
        }
        Ok(sub_biomes)
    }

    /// WorldGenReader.loadBiomeType&lt;T&gt;(T, Object): the parameter list of one type; `None` is the
    /// original's null for a missing list (the Biome constructor then keeps the allOf default).
    fn load_biome_type<T: BiomeEnum>(&self, default: T, object: Object) -> R<Option<EnumSet<T>>> {
        match object {
            Object::Nil => Ok(None),
            Object::Table(table) => {
                let mut tmp = Vec::new();
                for (_, value) in KahluaTableImpl::iterate_snapshot(&table) {
                    let name = cast_str(&value)?
                        .ok_or_else(|| Throwable::class_cast(&value, "java.lang.String"))?;
                    let member = T::from_name(&name)
                        .ok_or_else(|| Throwable::runtime(format!("No enum constant {name}")))?;
                    tmp.push(member);
                }
                Ok(Some(if tmp.is_empty() {
                    EnumSet::of(default)
                } else {
                    EnumSet::copy_of(&tmp)
                }))
            }
            o => Err(Throwable::class_cast(
                &o,
                "se.krka.kahlua.j2se.KahluaTableImpl",
            )),
        }
    }

    /// WorldGenReader.loadDouble(Object, Double).
    fn load_double(&self, object: &Object, default_value: f64) -> R<f64> {
        match object {
            Object::Nil => Ok(default_value),
            Object::Number(d) => Ok(*d),
            o => Err(Throwable::class_cast(o, "java.lang.Double")),
        }
    }

    /// WorldGenReader.loadBoolean(Object, boolean).
    fn load_boolean(&self, object: &Object, default_value: bool) -> R<bool> {
        match object {
            Object::Nil => Ok(default_value),
            Object::Boolean(b) => Ok(*b),
            o => Err(Throwable::class_cast(o, "java.lang.Boolean")),
        }
    }

    /// WorldGenReader.loadList(KahluaTable) as the List&lt;Double&gt; callers read it; a null table is the
    /// empty list.
    fn load_list_doubles(table: Option<TableRef>) -> R<Vec<f64>> {
        let Some(table) = table else {
            return Ok(Vec::new());
        };
        let mut values = Vec::new();
        for (_, value) in KahluaTableImpl::iterate_snapshot(&table) {
            match value {
                Object::Number(d) => values.push(d),
                o => return Err(Throwable::class_cast(&o, "java.lang.Double")),
            }
        }
        Ok(values)
    }

    /// WorldGenReader.loadList(KahluaTable) as the List&lt;String&gt; callers read it; a null table is the
    /// empty list.
    fn load_list_strings(table: Option<TableRef>) -> R<Vec<String>> {
        let Some(table) = table else {
            return Ok(Vec::new());
        };
        let mut values = Vec::new();
        for (_, value) in KahluaTableImpl::iterate_snapshot(&table) {
            values.push(
                cast_str(&value)?
                    .ok_or_else(|| Throwable::class_cast(&value, "java.lang.String"))?,
            );
        }
        Ok(values)
    }
}
