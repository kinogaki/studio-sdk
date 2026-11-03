//! [`Texture`]: decoded pixels (the host decodes files; engines need no
//! decoders).
use kinogaki_geometry::Error;
use std::sync::Arc;

/// What a texel's channels are.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TexelFormat {
    /// 8-bit unsigned normalized.
    U8,
    /// IEEE half floats (their bits, little-endian).
    F16,
    /// IEEE single floats (little-endian).
    F32,
}

impl TexelFormat {
    /// Bytes a channel takes.
    pub fn bytes(self) -> usize {
        match self {
            TexelFormat::U8 => 1,
            TexelFormat::F16 => 2,
            TexelFormat::F32 => 4,
        }
    }
}

/// How a texture's values map to light.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextureColorSpace {
    /// sRGB-encoded Rec.709 color (8-bit; engines decode it and convert it
    /// to ACEScg).
    Srgb,
    /// Linear ACEScg, the render's color space (hosts convert other spaces
    /// before sending).
    Linear,
    /// Data, read as stored.
    Raw,
}

/// Decoded pixels.
#[derive(Clone, Debug, PartialEq)]
pub struct Texture {
    width: u32,
    height: u32,
    channels: u32,
    format: TexelFormat,
    pixels: Arc<[u8]>,
    color_space: TextureColorSpace,
}

impl Texture {
    /// `pixels`: `width × height × channels` values of `format`, interleaved,
    /// rows from the top, meaning `color_space`.
    ///
    /// # Errors
    /// Not 1 to 4 channels, or pixels of another length.
    pub fn new(
        [width, height]: [u32; 2],
        channels: u32,
        format: TexelFormat,
        pixels: Arc<[u8]>,
        color_space: TextureColorSpace,
    ) -> Result<Texture, Error> {
        if !(1..=4).contains(&channels) {
            return Err(Error::invalid(format_args!("{channels} channels a texel")));
        }
        let wanted = (width as usize)
            .checked_mul(height as usize)
            .and_then(|n| n.checked_mul(channels as usize))
            .and_then(|n| n.checked_mul(format.bytes()));
        if wanted != Some(pixels.len()) {
            return Err(Error::invalid(format_args!(
                "{} bytes of pixels for a {width} × {height} texture",
                pixels.len()
            )));
        }
        Ok(Texture {
            width,
            height,
            channels,
            format,
            pixels,
            color_space,
        })
    }

    /// Texels across.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Texels down.
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Channels a texel.
    pub fn channels(&self) -> u32 {
        self.channels
    }

    /// The channels' format.
    pub fn format(&self) -> TexelFormat {
        self.format
    }

    /// The values, interleaved, rows from the top.
    pub fn pixels(&self) -> &Arc<[u8]> {
        &self.pixels
    }

    /// What the values mean.
    pub fn color_space(&self) -> TextureColorSpace {
        self.color_space
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_texture_holds_exactly_its_pixels() {
        let make = |size, channels, bytes: usize| {
            let pixels = Arc::from(vec![0u8; bytes]);
            Texture::new(
                size,
                channels,
                TexelFormat::F16,
                pixels,
                TextureColorSpace::Raw,
            )
        };
        assert!(make([2, 2], 3, 24).is_ok());
        assert!(make([2, 2], 3, 23).is_err());
        assert!(make([2, 2], 5, 40).is_err());
        assert!(make([u32::MAX, u32::MAX], 4, 8).is_err());
    }
}
