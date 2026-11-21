//! zombie.iso.worldgen.maps.BiomeRaster: the first two channels of each map pixel.

/// BiomeRaster.createFromImage / getSamples. The image loader is external to the engine: callers
/// supply decoded RGBA data; the raster preserves exactly the two bytes the Java class extracts.
#[derive(Clone, Debug)]
pub struct BiomeRaster {
    pub width: i32,
    pub height: i32,
    pixels: Vec<u8>,
}

impl BiomeRaster {
    /// BiomeRaster.createFromImage: copy the R and G channels of every pixel, x first within y.
    pub fn from_rgba(width: i32, height: i32, rgba: &[u8]) -> Result<Self, String> {
        let count = usize::try_from(width).ok().and_then(|w| usize::try_from(height).ok().and_then(|h| w.checked_mul(h)))
            .ok_or("invalid biome raster dimensions")?;
        let expected = count.checked_mul(4).ok_or("biome raster dimensions overflow")?;
        if rgba.len() < expected {
            return Err("insufficient RGBA pixels for biome raster".to_owned());
        }
        let mut pixels = Vec::with_capacity(count.checked_mul(2).ok_or("biome raster dimensions overflow")?);
        for pixel in rgba.chunks_exact(4).take(count) {
            pixels.extend_from_slice(&pixel[..2]);
        }
        Ok(Self { width, height, pixels })
    }

    /// BiomeRaster.getSamples: extract rectangle in row-major order, ignoring all channels but
    /// the selected band (0 biome or 1 zone), with unsigned byte-to-int conversion.
    pub fn get_samples(&self, x: i32, y: i32, w: i32, h: i32, band: i32) -> Result<Vec<i32>, String> {
        if x < 0 || y < 0 || w < 0 || h < 0 || x.checked_add(w).is_none_or(|v| v > self.width) || y.checked_add(h).is_none_or(|v| v > self.height) {
            return Err("java.lang.ArrayIndexOutOfBoundsException: Invalid coordinates.".to_owned());
        }
        if !(0..2).contains(&band) {
            return Err("java.lang.IllegalArgumentException: invalid band".to_owned());
        }
        let mut out = Vec::with_capacity((w * h) as usize);
        for yy in y..y + h {
            for xx in x..x + w {
                out.push(self.pixels[(xx * 2 + band + yy * self.width * 2) as usize] as i32);
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_bands_are_separate_and_unsigned() {
        let raster = BiomeRaster::from_rgba(2, 1, &[250, 16, 0, 255, 30, 128, 0, 255]).expect("valid raster");
        assert_eq!(raster.get_samples(0, 0, 2, 1, 0).expect("red"), vec![250, 30]);
        assert_eq!(raster.get_samples(0, 0, 2, 1, 1).expect("green"), vec![16, 128]);
    }
}
