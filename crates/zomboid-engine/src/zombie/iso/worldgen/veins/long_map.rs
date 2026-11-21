//! zombie.entity.util.LongMap: the gnu.trove-style long-keyed map the veins code caches its cells in.

use std::collections::HashMap;

/// zombie.entity.util.LongMap&lt;V&gt; — the gnu.trove-style long-keyed map, the methods Veins uses (containsKey,
/// get, put). Only keyed lookups are observable (the map is never iterated by this caller), so a std HashMap
/// is faithful: the original is a libgdx-style open-addressing table (Fibonacci-hashed `place`, linear
/// probing, a separate `zeroValue` slot for key 0), and every key answers containsKey/get/put exactly as a
/// std HashMap does — key 0 included, since the original's `zeroValue` slot behaves as a normal entry through
/// those three methods. Left out for that reason: the trove iteration order (`iterator`/`entries()`/
/// `values()`/`keys()` walk the open-addressing table, an order no caller of this port observes) and with it
/// `putAll`, `remove`, `shrink`, both `clear`s, `containsValue`, `findKey`, `ensureCapacity`, `notEmpty`,
/// `hashCode`, `equals`, `equalsIdentity`, `toString` and the copy constructor.
#[derive(Clone, Debug)]
pub struct LongMap<V> {
    map: HashMap<i64, V>,
}

impl<V> Default for LongMap<V> {
    /// LongMap(): initial capacity 51 at load factor 0.8. The table shape is not observable through the
    /// ported methods, so the std default stands in for it.
    fn default() -> Self {
        LongMap {
            map: HashMap::new(),
        }
    }
}

impl<V> LongMap<V> {
    /// zombie.entity.util.LongMap().
    pub fn new() -> Self {
        LongMap::default()
    }

    /// zombie.entity.util.LongMap.containsKey(long).
    pub fn contains_key(&self, key: i64) -> bool {
        self.map.contains_key(&key)
    }

    /// zombie.entity.util.LongMap.get(long): null when the key is absent.
    pub fn get(&self, key: i64) -> Option<&V> {
        self.map.get(&key)
    }

    /// zombie.entity.util.LongMap.put(long, V): the key's previous value, null if it was absent.
    pub fn put(&mut self, key: i64, value: V) -> Option<V> {
        self.map.insert(key, value)
    }

    /// zombie.entity.util.LongMap.size (the public `size` field).
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// zombie.entity.util.LongMap.isEmpty. Beyond the requested containsKey/get/put/len slice; `len`
    /// without it trips clippy's len_without_is_empty.
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}
