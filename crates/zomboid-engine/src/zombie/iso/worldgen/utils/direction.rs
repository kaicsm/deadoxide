//! zombie.iso.worldgen.utils.Direction enum: the eight compass directions of the world grid (x east, y south).

use std::fmt;

/// zombie.iso.worldgen.utils.Direction: the constants `NORTH(0, 1, 4, 5, 0, -1, "n")` ... in declaration
/// order, the constructor parameters being (index, opposite, prev, next, x, y, name). Note the y field is
/// negative to the north: grid y grows southwards.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Direction {
    North,
    South,
    West,
    East,
    NorthWest,
    NorthEast,
    SouthWest,
    SouthEast,
}

/// The constant table, one row per constant in the constructor's parameter order:
/// (index, opposite, prev, next, x, y, name).
const FIELDS: [(i32, i32, i32, i32, i32, i32, &str); 8] = [
    (0, 1, 4, 5, 0, -1, "n"),    // NORTH
    (1, 0, 7, 6, 0, 1, "s"),     // SOUTH
    (2, 3, 6, 4, -1, 0, "w"),    // WEST
    (3, 2, 5, 7, 1, 0, "e"),     // EAST
    (4, 7, 2, 0, -1, -1, "nw"),  // NORTH_WEST
    (5, 6, 0, 3, 1, -1, "ne"),   // NORTH_EAST
    (6, 5, 1, 2, -1, 1, "sw"),   // SOUTH_WEST
    (7, 4, 3, 1, 1, 1, "se"),    // SOUTH_EAST
];

/// `private static final List<Direction> cardinals = List.of(NORTH, SOUTH, WEST, EAST);`
static CARDINALS: [Direction; 4] = [
    Direction::North,
    Direction::South,
    Direction::West,
    Direction::East,
];

/// `private static final List<Direction> diagonals = List.of(NORTH_WEST, NORTH_EAST, SOUTH_WEST, SOUTH_EAST);`
static DIAGONALS: [Direction; 4] = [
    Direction::NorthWest,
    Direction::NorthEast,
    Direction::SouthWest,
    Direction::SouthEast,
];

/// `private static final List<Direction> rose = List.of(NORTH, SOUTH, WEST, EAST, NORTH_WEST, NORTH_EAST,
/// SOUTH_WEST, SOUTH_EAST);` — the same order as `values()`.
static ROSE: [Direction; 8] = [
    Direction::North,
    Direction::South,
    Direction::West,
    Direction::East,
    Direction::NorthWest,
    Direction::NorthEast,
    Direction::SouthWest,
    Direction::SouthEast,
];

impl Direction {
    /// `Direction.values()`: the constants in declaration order.
    pub const ALL: &[Direction; 8] = &[
        Direction::North,
        Direction::South,
        Direction::West,
        Direction::East,
        Direction::NorthWest,
        Direction::NorthEast,
        Direction::SouthWest,
        Direction::SouthEast,
    ];

    /// `Enum.ordinal()` (which equals the `index` constant of every direction).
    fn ordinal(self) -> usize {
        Self::ALL.iter().position(|d| *d == self).unwrap_or(0)
    }

    fn fields(self) -> (i32, i32, i32, i32, i32, i32, &'static str) {
        FIELDS[self.ordinal()]
    }

    /// zombie.iso.worldgen.utils.Direction.index (the final int field).
    pub fn index(&self) -> i32 {
        self.fields().0
    }

    /// zombie.iso.worldgen.utils.Direction.x (the final int field).
    pub fn x(&self) -> i32 {
        self.fields().4
    }

    /// zombie.iso.worldgen.utils.Direction.y (the final int field).
    pub fn y(&self) -> i32 {
        self.fields().5
    }

    /// zombie.iso.worldgen.utils.Direction.name: the final String FIELD ("n", "s", ...). This shadows nothing
    /// in Java (fields and methods are separate there) — the inherited `Enum.name()` would return "NORTH";
    /// the game's own code never reads the field.
    pub fn name(&self) -> &'static str {
        self.fields().6
    }

    /// zombie.iso.worldgen.utils.Direction.opposite: `values()[this.opposite]`.
    pub fn opposite(&self) -> Direction {
        Self::ALL[self.fields().1 as usize]
    }

    /// zombie.iso.worldgen.utils.Direction.prev: `values()[this.prev]`.
    pub fn prev(&self) -> Direction {
        Self::ALL[self.fields().2 as usize]
    }

    /// zombie.iso.worldgen.utils.Direction.next: `values()[this.next]`.
    pub fn next(&self) -> Direction {
        Self::ALL[self.fields().3 as usize]
    }

    /// zombie.iso.worldgen.utils.Direction.cardinals: `List.of(NORTH, SOUTH, WEST, EAST)`.
    pub fn cardinals() -> &'static [Direction] {
        &CARDINALS
    }

    /// zombie.iso.worldgen.utils.Direction.diagonals: `List.of(NORTH_WEST, NORTH_EAST, SOUTH_WEST, SOUTH_EAST)`.
    pub fn diagonals() -> &'static [Direction] {
        &DIAGONALS
    }

    /// zombie.iso.worldgen.utils.Direction.rose: all eight, cardinals first (the same order as `values()`).
    pub fn rose() -> &'static [Direction] {
        &ROSE
    }
}

impl fmt::Display for Direction {
    /// The Java `name` FIELD ("n", "s", ...). The enum's inherited `toString` (what string concatenation
    /// would print) is the constant name "NORTH"; the original never overrides it and in fact never prints a
    /// Direction at all.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}
