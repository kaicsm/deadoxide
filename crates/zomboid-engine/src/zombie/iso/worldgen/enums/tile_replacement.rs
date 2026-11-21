//! zombie.iso.worldgen.enums.TileReplacement: the outcome of placing one worldgen tile.

/// zombie.iso.worldgen.enums.TileReplacement (declaration order): WorldGenTile.applyTile's result — SUCCESS
/// with the placed sprite, FAILURE falling back to the carpentry placeholder, PENDING/DELETE the
/// to-be-done features.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TileReplacement {
    Success,
    Failure,
    Pending,
    Delete,
}

impl TileReplacement {
    /// TileReplacement.values(): the constants in declaration order.
    pub const ALL: &[TileReplacement; 4] = &[
        TileReplacement::Success,
        TileReplacement::Failure,
        TileReplacement::Pending,
        TileReplacement::Delete,
    ];

    /// `Enum.name()`: the Java constant names.
    pub fn name(&self) -> &'static str {
        match self {
            TileReplacement::Success => "SUCCESS",
            TileReplacement::Failure => "FAILURE",
            TileReplacement::Pending => "PENDING",
            TileReplacement::Delete => "DELETE",
        }
    }
}
