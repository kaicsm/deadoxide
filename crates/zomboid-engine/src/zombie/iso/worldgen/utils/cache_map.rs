//! zombie.iso.worldgen.utils.CacheMap: a self-clearing HashMap cache.

use std::collections::HashMap;

/// zombie.iso.worldgen.utils.CacheMap<K, V>: a HashMap whose put clears the whole map once size() exceeds the
/// fixed capacity (checked BEFORE the put). The game only ever calls get/put/containsKey on it (BiomeMap's
/// 16-entry raster cache), never iterating it, so a std HashMap keyed by the value key is faithful — the
/// original's iteration order is unobserved and not reproduced.
pub struct CacheMap<K: std::hash::Hash + Eq + Clone, V> {
    map: HashMap<K, V>,
    /// The Java field `size`, which shadows the inherited `size()` method inside the class body: the fixed
    /// capacity given to the constructor.
    size: usize,
}

impl<K: std::hash::Hash + Eq + Clone, V> CacheMap<K, V> {
    /// zombie.iso.worldgen.utils.CacheMap.CacheMap(int size).
    pub fn new(size: usize) -> Self {
        CacheMap {
            map: HashMap::new(),
            size,
        }
    }

    /// zombie.iso.worldgen.utils.CacheMap.put: `if (this.size() > this.size) clear();` — the method call is
    /// the CURRENT entry count, the field the capacity: the whole map is cleared when the count before this
    /// put exceeds the capacity, then the plain put runs. Returns the previous value of the key.
    pub fn put(&mut self, key: K, value: V) -> Option<V> {
        if self.map.len() > self.size {
            self.map.clear();
        }
        self.map.insert(key, value)
    }

    /// java.util.HashMap.get
    pub fn get(&self, key: &K) -> Option<&V> {
        self.map.get(key)
    }

    /// java.util.HashMap.containsKey
    pub fn contains_key(&self, key: &K) -> bool {
        self.map.contains_key(key)
    }

    /// java.util.HashMap.size
    pub fn len(&self) -> usize {
        self.map.len()
    }
}
