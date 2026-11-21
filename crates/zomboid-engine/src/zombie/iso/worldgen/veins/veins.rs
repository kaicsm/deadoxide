//! zombie.iso.worldgen.veins.Veins: the per-cell ore vein cache — on a first `get(cellX, cellY)` it rolls one
//! `nextFloat` per OreVeinConfig (in the config map's iteration order) against that config's probability and
//! builds an OreVein from the same `Random` for every roll that passes, caching the list under the packed
//! cell coordinates.

use crate::java::JavaHashMap;
use crate::zombie::iso::worldgen::world_gen_params::WorldGenParams;

use super::long_map::LongMap;
use super::ore_vein::OreVein;
use super::ore_vein_config::OreVeinConfig;

/// zombie.iso.worldgen.veins.Veins.
pub struct Veins {
    /// `private final LongMap&lt;List&lt;OreVein&gt;&gt; cache` (Veins.java:12).
    cache: LongMap<Vec<OreVein>>,
    /// `private final Map&lt;String, OreVeinConfig&gt; config` (Veins.java:13): a java.util.HashMap in the
    /// loader, kept as the JavaHashMap port so `values()` keeps Java's iteration order.
    config: JavaHashMap<OreVeinConfig>,
}

impl Veins {
    /// zombie.iso.worldgen.veins.Veins.&lt;init&gt;(Map&lt;String, OreVeinConfig&gt;) (Veins.java:15-17):
    /// stores the config map. The original accepts any Map; the port takes the java.util.HashMap port,
    /// which is what loads it.
    pub fn new(config: JavaHashMap<OreVeinConfig>) -> Self {
        Veins {
            cache: LongMap::new(),
            config,
        }
    }

    /// zombie.iso.worldgen.veins.Veins.get(int cellX, int cellY) (Veins.java:19-40): the cell's vein list,
    /// served from the cache or generated into it. The key is `(long)cellX &lt;&lt; 32 | cellY &amp;
    /// 4294967295L` — cellY sign-extends to long and ANDs down to its unsigned 32 bits, then ORs over the
    /// shifted cellX (Veins.java:20). On a miss the Random is `WorldGenParams.INSTANCE.getRandom(cellX,
    /// cellY)` (Veins.java:24) — the port takes the WorldGenParams as an argument, there is no singleton —
    /// and every config entry, in the map's iteration order (`config.values()`, Veins.java:27), draws one
    /// `nextFloat`: `!(rnd.nextFloat() &gt; subConfig.getProbability())` (Veins.java:28) constructs the
    /// vein (a NaN probability passes, as in Java), the constructor drawing from the same rnd. The
    /// `DebugLog.log(ret.toString())` of a non-empty result (Veins.java:34-36) is debug output and is not
    /// ported, nor is OreVein.toString. The cache hit keeps the original's containsKey/get pair
    /// (Veins.java:21-22) - two lookups, as in Java.
    pub fn get(&mut self, cell_x: i32, cell_y: i32, params: &WorldGenParams) -> &[OreVein] {
        let key = ((cell_x as i64) << 32) | ((cell_y as i64) & 0xFFFF_FFFF);
        // The containsKey/get split of the original (Veins.java:21-22) is kept literally: the early
        // return's borrow must not span the miss path's put, so the guard runs before the lookup.
        if self.cache.contains_key(key) {
            return self.cache.get(key).expect("containsKey just matched the key");
        }
        let mut rnd = params.get_random(cell_x, cell_y);
        let mut ret: Vec<OreVein> = Vec::new();
        for sub_config in self.config.values() {
            if !(rnd.next_float() > sub_config.probability) {
                ret.push(OreVein::new(cell_x, cell_y, sub_config, &mut rnd));
            }
        }
        self.cache.put(key, ret);
        self.cache.get(key).expect("the key was just put")
    }
}
