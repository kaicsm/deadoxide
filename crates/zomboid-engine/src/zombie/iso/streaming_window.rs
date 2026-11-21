//! Chunk residency window following zombie.iso.IsoChunkMap.LoadLeft/Right/Up/Down and Unload.
//! This is the deterministic **scheduling** part of streaming. I/O and WorldGen are handled
//! by Scene::stream_around, so failed loads never leave a half-updated resident window.
use std::collections::HashSet;

/// World-space chunk coordinates (not cell or tile coordinates).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChunkWindow {
    pub cx: i32,
    pub cy: i32,
    pub radius: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WindowDiff {
    /// New chunks, nearest to the player first (stable order for ties).
    pub load: Vec<(i32, i32)>,
    /// Chunks outside the next window, farthest first.
    pub unload: Vec<(i32, i32)>,
    /// IsoChunkMap.teleport-style full shift instead of a shared edge.
    pub teleport: bool,
}

impl ChunkWindow {
    /// IsoChunkMap.update: chunk coordinate is floor(player tile position / 8).
    pub fn for_player(x: f32, y: f32, radius: i32) -> Result<Self, String> {
        if !x.is_finite() || !y.is_finite() || !(1..=16).contains(&radius)
            || x.abs() > (i32::MAX as f32) / 2.0 || y.abs() > (i32::MAX as f32) / 2.0
        {
            return Err("invalid player position or streaming radius".into());
        }
        Ok(Self { cx: (x.floor() as i32).div_euclid(8), cy: (y.floor() as i32).div_euclid(8), radius })
    }

    pub fn contains(self, wx: i32, wy: i32) -> bool {
        (i64::from(wx) - i64::from(self.cx)).abs() <= i64::from(self.radius)
            && (i64::from(wy) - i64::from(self.cy)).abs() <= i64::from(self.radius)
    }

    pub fn cells(self) -> (i32, i32, i32, i32) {
        ((self.cx - self.radius).div_euclid(32), (self.cy - self.radius).div_euclid(32),
         (self.cx + self.radius).div_euclid(32), (self.cy + self.radius).div_euclid(32))
    }

    pub fn coords(self) -> Vec<(i32, i32)> {
        let mut coords = Vec::with_capacity(((2 * self.radius + 1).pow(2)) as usize);
        for x in self.cx - self.radius..=self.cx + self.radius {
            for y in self.cy - self.radius..=self.cy + self.radius {
                coords.push((x, y));
            }
        }
        coords
    }

    /// The queue still contains missing slots after a partial IsoChunkMap.updateInternal.
    /// Sort by proximity so the player's current chunk is available first after a teleport.
    pub fn missing_resident(self, resident: &HashSet<(i32, i32)>) -> Vec<(i32, i32)> {
        let mut missing: Vec<_> = self.coords().into_iter()
            .filter(|key| !resident.contains(key)).collect();
        missing.sort_by_key(|&(wx, wy)| (
            (i64::from(wx) - i64::from(self.cx)).abs()
                + (i64::from(wy) - i64::from(self.cy)).abs(), wx, wy));
        missing
    }

    /// zombie.iso.WorldStreamer.ChunkComparator.init/compare: sort outstanding
    /// jobs by squared Euclidean distance to the position ten tiles ahead of
    /// a moving player. When stationary the comparator uses the player position.
    /// Java's comparator returns the farthest job first and removes the last;
    /// our queue consumes the closest first, which is equivalent.
    pub fn missing_resident_projected(
        self, resident: &HashSet<(i32, i32)>,
        player: (f32, f32), previous: (f32, f32),
    ) -> Vec<(i32, i32)> {
        let (dx, dy) = (player.0 - previous.0, player.1 - previous.1);
        let length = (dx * dx + dy * dy).sqrt();
        let target = if length > 0.0 {
            (player.0 + dx / length * 10.0, player.1 + dy / length * 10.0)
        } else { player };
        let mut missing: Vec<_> = self.coords().into_iter()
            .filter(|key| !resident.contains(key)).collect();
        missing.sort_by(|a, b| {
            let score = |(wx, wy): &(i32, i32)| {
                let x = *wx as f32 * 8.0 + 4.0 - target.0;
                let y = *wy as f32 * 8.0 + 4.0 - target.1;
                x * x + y * y
            };
            score(a).total_cmp(&score(b)).then_with(|| a.cmp(b))
        });
        missing
    }

    /// Give a square immediately in the player's movement path priority over the
    /// remaining ready queue. This is a Rust adapter for avoiding movement against
    /// an as-yet-unavailable IsoGridSquare; normal jobs still use the original
    /// WorldStreamer.ChunkComparator distance ordering.
    pub fn prioritize_missing(missing: &mut Vec<(i32, i32)>, wanted: (i32, i32)) {
        if let Some(index) = missing.iter().position(|&key| key == wanted) {
            if index != 0 {
                missing.remove(index);
                missing.insert(0, wanted);
            }
        }
    }

    /// IsoChunkMap's one-edge scroll, large-jump/teleport, and negative-world transitions.
    /// Only new coordinates enter `load`; retained chunks keep their object identity/state.
    pub fn diff(self, next: Self) -> WindowDiff {
        let old: HashSet<_> = self.coords().into_iter().collect();
        let new: HashSet<_> = next.coords().into_iter().collect();
        let mut load: Vec<_> = new.difference(&old).copied().collect();
        let mut unload: Vec<_> = old.difference(&new).copied().collect();
        let distance = |(x, y): &(i32, i32)| (i64::from(*x) - i64::from(next.cx)).abs()
            + (i64::from(*y) - i64::from(next.cy)).abs();
        load.sort_by_key(|p| (distance(p), p.0, p.1));
        unload.sort_by_key(|p| (std::cmp::Reverse(distance(p)), p.0, p.1));
        WindowDiff { load, unload, teleport: (i64::from(next.cx) - i64::from(self.cx)).abs() >= (2 * self.radius + 1) as i64
            || (i64::from(next.cy) - i64::from(self.cy)).abs() >= (2 * self.radius + 1) as i64 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn one_chunk_scroll_preserves_all_but_one_edge() {
        let start = ChunkWindow { cx: 5, cy: 7, radius: 4 };
        let diff = start.diff(ChunkWindow { cx: 6, ..start });
        assert_eq!(diff.load.len(), 9);
        assert_eq!(diff.unload.len(), 9);
        assert!(diff.load.iter().all(|p| p.0 == 10));
        assert!(diff.unload.iter().all(|p| p.0 == 1));
        assert!(!diff.teleport);
    }
    #[test]
    fn teleport_replaces_window_and_handles_negative_chunks() {
        let start = ChunkWindow { cx: 10, cy: 20, radius: 2 };
        let diff = start.diff(ChunkWindow { cx: -10, cy: -20, ..start });
        assert!(diff.teleport);
        assert_eq!((diff.load.len(), diff.unload.len()), (25, 25));
        assert_eq!(ChunkWindow::for_player(-0.01, -8.01, 2).unwrap(), ChunkWindow { cx: -1, cy: -2, radius: 2 });
    }
    #[test]
    fn no_new_requests_if_player_stays_in_same_chunk() {
        let a = ChunkWindow::for_player(17.1, 22.8, 3).unwrap();
        let b = ChunkWindow::for_player(23.9, 23.9, 3).unwrap();
        assert_eq!(a, b);
        assert!(a.diff(b).load.is_empty());
    }
    #[test]
    fn partially_loaded_window_stays_queued_until_complete() {
        let first = ChunkWindow { cx: 18, cy: 31, radius: 2 };
        let moved = ChunkWindow { cx: 19, ..first };
        let mut resident: HashSet<_> = first.coords().into_iter().collect();
        let mut batches = Vec::new();
        for _ in 0..4 {
            let batch: Vec<_> = moved.missing_resident(&resident).into_iter().take(2).collect();
            for key in &batch { resident.insert(*key); }
            batches.push(batch.len());
        }
        assert_eq!(batches, vec![2, 2, 1, 0]);
        assert!(moved.missing_resident(&resident).is_empty());
        assert!(resident.contains(&(21, 31)));
    }

    #[test]
    fn missing_movement_square_preempts_distant_jobs_without_changing_membership() {
        let mut jobs = vec![(100, 100), (100, 101), (100, 102)];
        ChunkWindow::prioritize_missing(&mut jobs, (100, 102));
        assert_eq!(jobs, vec![(100, 102), (100, 100), (100, 101)]);
        ChunkWindow::prioritize_missing(&mut jobs, (999, 999));
        assert_eq!(jobs, vec![(100, 102), (100, 100), (100, 101)]);
    }

    #[test]
    fn teleport_prioritizes_destination_center() {
        let source = ChunkWindow { cx: 30, cy: 30, radius: 2 };
        let far = ChunkWindow { cx: -8, cy: -15, radius: 2 };
        let present: HashSet<_> = source.coords().into_iter().collect();
        assert_eq!(far.missing_resident(&present).first(), Some(&(-8, -15)));
        assert_eq!(far.missing_resident(&present).len(), 25);
    }

    #[test]
    fn moving_player_prioritizes_chunks_ten_tiles_ahead() {
        let window = ChunkWindow { cx: 10, cy: 10, radius: 2 };
        let loaded = HashSet::new();
        let east = window.missing_resident_projected(&loaded, (84.0, 84.0), (83.0, 84.0));
        let west = window.missing_resident_projected(&loaded, (84.0, 84.0), (85.0, 84.0));
        assert!(east[0].0 > west[0].0);
        let still = window.missing_resident_projected(&loaded, (84.0, 84.0), (84.0, 84.0));
        assert_eq!(still[0], (10, 10));
    }

    #[test]
    fn crossing_cell_32_boundary_preserves_neighbors() {
        let start = ChunkWindow { cx: 31, cy: 31, radius: 2 };
        let next = ChunkWindow { cx: 32, cy: 32, radius: 2 };
        let diff = start.diff(next);
        assert_eq!((diff.load.len(), diff.unload.len()), (9, 9));
        assert_eq!(next.cells(), (0, 0, 1, 1));
    }
}
