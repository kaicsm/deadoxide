//! zombie.iso.worldgen.roads.RoadDirection.

use std::fmt;

/// zombie.iso.worldgen.roads.RoadDirection: the two directions a road runs in. `NS` is a north-south road
/// (`Road`'s `a.x == b.x`), `WE` a west-east one; the Java constant names are already `NS`/`WE`, so the
/// Rust variants keep them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RoadDirection {
    NS,
    WE,
}

impl RoadDirection {
    /// java.lang.Enum.name
    pub fn name(&self) -> &'static str {
        match self {
            RoadDirection::NS => "NS",
            RoadDirection::WE => "WE",
        }
    }
}

/// java.lang.Enum.toString: the constant's name.
impl fmt::Display for RoadDirection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}
