//! zombie.iso.worldgen.maps.BiomeRaster: the first two channels of each map pixel.

/// BiomeRaster.createFromImage / getSamples: retain the first two components of each pixel,
/// as ImageData.getPixelClamped does in the Java engine.
#[derive(Clone, Debug)]
pub struct BiomeRaster {
    pub width: i32,
    pub height: i32,
    pixels: Vec<u8>,
}

impl BiomeRaster {
    /// ImageData(String) in the original Java catches native image decoding failures,
    /// sets width and height to -1 and returns. BiomeRaster.createFromImage then
    /// retains those dimensions rather than throwing. A later getSamples call throws
    /// ArrayIndexOutOfBoundsException("Invalid coordinates.").
    ///
    /// Preserve that observable state instead of mistaking a corrupt image for
    /// an absent image. Missing files are handled by BiomeMap before decoding.
    pub(crate) fn invalid_image() -> Self {
        Self { width: -1, height: -1, pixels: vec![0; 2] }
    }

    /// BiomeRaster.createFromImage(String): decode the selected image as RGBA, then preserve
    /// the red (biome) and green (zone) bytes. PNG rows are stored top-to-bottom, just as
    /// NativeImage.read/ImageData.getPixelClamped exposes them to the Java world generator.
    /// Palette, transparency and sub-eight-bit samples are expanded by the PNG decoder.
    pub fn from_png_path(path: &std::path::Path) -> Result<Self, String> {
        use std::io::BufReader;

        let file = std::fs::File::open(path)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        let mut decoder = png::Decoder::new(BufReader::new(file));
        decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
        let mut reader = decoder.read_info()
            .map_err(|e| format!("{}: {e}", path.display()))?;
        let mut raw = vec![0u8; reader.output_buffer_size()];
        let frame = reader.next_frame(&mut raw)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        let width = i32::try_from(frame.width).map_err(|_| "biome raster width overflow")?;
        let height = i32::try_from(frame.height).map_err(|_| "biome raster height overflow")?;
        if width == 0 || height == 0 {
            return Err("invalid biome raster dimensions".to_owned());
        }
        let count = (width as usize).checked_mul(height as usize)
            .ok_or("biome raster dimensions overflow")?;
        let mut pixels = Vec::with_capacity(count.checked_mul(2)
            .ok_or("biome raster dimensions overflow")?);
        let buffer = &raw[..frame.buffer_size()];
        match frame.color_type {
            png::ColorType::Rgba => {
                for pixel in buffer.chunks_exact(4) {
                    pixels.extend_from_slice(&pixel[..2]);
                }
            }
            png::ColorType::Rgb => {
                for pixel in buffer.chunks_exact(3) {
                    pixels.extend_from_slice(&pixel[..2]);
                }
            }
            png::ColorType::GrayscaleAlpha => {
                for pixel in buffer.chunks_exact(2) {
                    pixels.extend_from_slice(&[pixel[0], pixel[0]]);
                }
            }
            png::ColorType::Grayscale => {
                for &grey in buffer {
                    pixels.extend_from_slice(&[grey, grey]);
                }
            }
            png::ColorType::Indexed => {
                return Err("indexed PNG was not expanded".to_owned());
            }
        }
        if pixels.len() != count * 2 {
            return Err("PNG pixels do not match image dimensions".to_owned());
        }
        Ok(Self { width, height, pixels })
    }

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
