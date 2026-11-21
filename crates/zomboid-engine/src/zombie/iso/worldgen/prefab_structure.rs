//! zombie.iso.worldgen.PrefabStructure.

use crate::java::JavaHashMap;

/// zombie.iso.worldgen.PrefabStructure: a static module's prefab - its tile dimensions, the tile names
/// `getTile` indexes, and the per-category schematics as `Map<String, int[][]>` (a java.util.HashMap's
/// iteration order). The original's `categories` field (`List.of("Floor", "FloorFurniture", "FloorOverlay",
/// "Furniture")`, immutable) is [`PrefabStructure::get_categories`].
#[derive(Clone, Debug)]
pub struct PrefabStructure {
    /// `int[2]`: a `dimensions` key the Lua leaves out keeps its slot 0.
    dimensions: [i32; 2],
    tiles: Vec<String>,
    /// Per category the rows of `new int[dimensions[1]][dimensions[0]]` - every row allocated zero-filled
    /// (a row the Lua does not mention is zeros, not null), a row it does mention replaced by the ints of
    /// its comma-split string, of any length.
    schematic: JavaHashMap<Vec<Vec<i32>>>,
    zombies: f32,
}

impl PrefabStructure {
    /// PrefabStructure's constructor.
    pub fn new(
        dimensions: [i32; 2],
        tiles: Vec<String>,
        schematic: JavaHashMap<Vec<Vec<i32>>>,
        zombies: f32,
    ) -> Self {
        PrefabStructure {
            dimensions,
            tiles,
            schematic,
            zombies,
        }
    }

    /// PrefabStructure.getX: `dimensions[0]`.
    pub fn get_x(&self) -> i32 {
        self.dimensions[0]
    }

    /// PrefabStructure.getY: `dimensions[1]`.
    pub fn get_y(&self) -> i32 {
        self.dimensions[1]
    }

    /// PrefabStructure.getCategories: `List.of("Floor", "FloorFurniture", "FloorOverlay", "Furniture")`.
    pub fn get_categories(&self) -> &'static [&'static str; 4] {
        &["Floor", "FloorFurniture", "FloorOverlay", "Furniture"]
    }

    /// PrefabStructure.hasCategory: `schematic.containsKey(category)`.
    pub fn has_category(&self, category: &str) -> bool {
        self.schematic.contains_key(category)
    }

    /// PrefabStructure.getTileRef: `schematic.get(category)[Math.max(y, 0)][Math.max(x, 0)]` - the row is
    /// its own length (a schematic row may be shorter or longer than `dimensions[0]`). The original's
    /// NullPointerException for a missing category and its ArrayIndexOutOfBoundsException for a clamped
    /// index past the row or column are the port's `None`.
    pub fn get_tile_ref(&self, category: &str, x: i32, y: i32) -> Option<i32> {
        let rows = self.schematic.get(category)?;
        let row = rows.get(y.max(0) as usize)?;
        row.get(x.max(0) as usize).copied()
    }

    /// PrefabStructure.getTile: `tiles.get(index)`. The original's IndexOutOfBoundsException is the port's
    /// `None`.
    pub fn get_tile(&self, index: i32) -> Option<&str> {
        usize::try_from(index).ok().and_then(|i| self.tiles.get(i)).map(String::as_str)
    }

    /// The `tiles` field the probe reads through reflection (no getter in the original): its length.
    pub fn tiles_len(&self) -> usize {
        self.tiles.len()
    }

    /// The `tiles` field the probe reads through reflection, in list order.
    pub fn tiles_iter(&self) -> impl Iterator<Item = &str> {
        self.tiles.iter().map(String::as_str)
    }

    /// The `schematic` field the probe reads through reflection, in the java.util.HashMap iteration order.
    pub fn schematic_iter(
        &self,
    ) -> impl Iterator<Item = (&str, &Vec<Vec<i32>>)> {
        self.schematic
            .iter()
            .map(|(category, rows)| (category, rows))
    }

    /// PrefabStructure.getZombies.
    pub fn get_zombies(&self) -> f32 {
        self.zombies
    }
}
