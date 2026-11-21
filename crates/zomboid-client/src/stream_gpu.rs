//! Renderer-only batching for streaming chunks. The Java game integrates chunks
//! individually in IsoChunkMap.updateInternal; the Rust wgpu renderer keeps
//! global (z, y, x) painter order by grouping consecutive rows into bands.
//! A changed chunk then invalidates only the affected y bands, not every GPU
//! instance belonging to the resident window.

/// Indices refer to Scene.instances and Scene.meta (one meta per instance).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BandSlice {
    pub z: i32,
    pub y_band: i32,
    pub start: usize,
    pub end: usize,
}

/// Consecutive world-space groups of eight rows. Negative coordinates must use
/// Euclidean division, matching the chunk coordinate calculations.
/// Precondition: instances are in the existing global (z, y, x) painter order.
pub fn partition_bands(meta: &[(i32, i32, i32, i32)]) -> Vec<BandSlice> {
    let mut out: Vec<BandSlice> = Vec::new();
    for (i, (_, y, z, _)) in meta.iter().enumerate() {
        let y_band = y.div_euclid(8);
        if let Some(last) = out.last_mut() {
            if (last.z, last.y_band) == (*z, y_band) {
                last.end = i + 1;
                continue;
            }
        }
        out.push(BandSlice { z: *z, y_band, start: i, end: i + 1 });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn streamed_instance_bands_preserve_painter_order_and_negative_rows() {
        let meta = [
            (0, -9, 0, -1), (1, -8, 0, -1), (2, -1, 0, -1),
            (0, 0, 0, -1), (2, 7, 0, -1), (0, 8, 0, -1),
            (0, -8, 1, -1), (1, 0, 1, -1),
        ];
        let bands = partition_bands(&meta);
        assert_eq!(bands.iter().map(|b| (b.z, b.y_band, b.start, b.end)).collect::<Vec<_>>(),
            vec![(0, -2, 0, 1), (0, -1, 1, 3), (0, 0, 3, 5),
                (0, 1, 5, 6), (1, -1, 6, 7), (1, 0, 7, 8)]);
        // Concatenating the bands is exactly the original global instance order.
        assert_eq!(bands.iter().flat_map(|b| b.start..b.end).collect::<Vec<_>>(),
            (0..meta.len()).collect::<Vec<_>>());
    }

    #[test]
    fn identical_instance_bands_avoid_all_gpu_writes() {
        let meta = [(0, 0, 0, 0), (1, 1, 0, 0), (8, 8, 0, 0)];
        assert_eq!(partition_bands(&meta), partition_bands(&meta));
    }
}
