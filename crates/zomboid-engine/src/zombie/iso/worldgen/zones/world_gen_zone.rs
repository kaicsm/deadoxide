//! zombie.iso.worldgen.zones.WorldGenZone: a map zone with its Rocks option.

use crate::zombie::iso::zones::zone::Zone;

/// WorldGenZone: extends Zone with `Rocks`, defaulting to true.
#[derive(Clone, Debug)]
pub struct WorldGenZone {
    pub zone: Zone,
    pub rocks: bool,
}

impl WorldGenZone {
    /// WorldGenZone.<init>: the caller supplies the parsed Lua Rocks boolean (None retains true).
    pub fn new(zone: Zone, rocks: Option<bool>) -> Self {
        Self { zone, rocks: rocks.unwrap_or(true) }
    }

    /// WorldGenZone.getRocks.
    pub fn get_rocks(&self) -> bool {
        self.rocks
    }

    /// Zone.contains: delegate to the existing geometry model.
    pub fn contains(&mut self, x: i32, y: i32, z: i32) -> bool {
        self.zone.contains(x, y, z)
    }
}
