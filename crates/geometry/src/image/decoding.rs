//! [`Decoding`]: how a color layer's values become linear light in another
//! space, as a GPU runs it after fetching texels; and [`TextureImage`], an
//! image with the decoding its reader applies.
use super::Image;

/// A transfer function on each channel, from a layer's stored values to
/// linear light.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Transfer {
    /// None: the values are linear.
    Linear,
    /// A power with an offset and a straight segment near black
    /// (OpenColorIO's `ExponentWithLinear`, moncurve): sRGB's curve is
    /// `gamma` 2.4, `offset` 0.055 ([`Transfer::SRGB`]).
    Exponent {
        /// The power.
        gamma: f32,
        /// The offset.
        offset: f32,
    },
    /// A pure power.
    Power(f32),
}

impl Transfer {
    /// sRGB's curve (IEC 61966-2-1's, as OpenColorIO builds it): what an
    /// 8-bit sRGB texture's GPU format decodes.
    pub const SRGB: Transfer = Transfer::Exponent {
        gamma: 2.4,
        offset: 0.055,
    };

    /// Where its straight segment meets the power, in stored values, and the
    /// segment's slope (an exponent curve's; none for the others).
    pub fn segment(self) -> Option<(f32, f32)> {
        let Transfer::Exponent { gamma, offset } = self else {
            return None;
        };
        let (gamma, offset) = (f64::from(gamma), f64::from(offset));
        let at = offset / (gamma - 1.0);
        let linear = ((at + offset) / (1.0 + offset)).powf(gamma);
        Some((at as f32, (linear / at) as f32))
    }

    /// `v`, a stored value, as linear light. A negative value mirrors a
    /// positive one; what the host's conversion gives for values above the
    /// straight segment's negative end.
    pub fn decode(self, v: f32) -> f32 {
        let (magnitude, sign) = (v.abs(), v.signum());
        let linear = match self {
            Transfer::Linear => return v,
            Transfer::Power(gamma) => magnitude.powf(gamma),
            Transfer::Exponent { gamma, offset } => {
                let (at, slope) = self.segment().unwrap_or((0.0, 1.0));
                match magnitude <= at {
                    true => magnitude * slope,
                    false => ((magnitude + offset) / (1.0 + offset)).powf(gamma),
                }
            }
        };
        linear * sign
    }
}

/// How a color layer's values become linear light in a space its reader
/// works in (an engine's render space, a view's working space): each channel
/// through `transfer`, then `matrix`. A host gives one where its conversion is
/// exactly that (the api's `ColorService::decoding`), so a GPU can run it
/// after it fetches, and filters, texels.
///
/// ```
/// use kinogaki_geometry::image::{Transfer, Decoding};
/// let srgb = Decoding { transfer: Transfer::SRGB, matrix: Decoding::IDENTITY.matrix };
/// let [r, g, b] = srgb.apply([1.0, 0.5, 0.0]);
/// assert!((r - 1.0).abs() < 1e-6 && (g - 0.214).abs() < 1e-3 && b == 0.0);
/// ```
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Decoding {
    /// The transfer function each channel goes through first.
    pub transfer: Transfer,
    /// Then this matrix, by rows: linear light in the reader's space.
    pub matrix: [[f32; 3]; 3],
}

impl Decoding {
    /// Values that are linear light in the reader's space already.
    pub const IDENTITY: Decoding = Decoding {
        transfer: Transfer::Linear,
        matrix: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
    };

    /// `rgb`, stored values, as linear light in the reader's space.
    pub fn apply(&self, rgb: [f32; 3]) -> [f32; 3] {
        let c = rgb.map(|v| self.transfer.decode(v));
        self.matrix
            .map(|row| row[0] * c[0] + row[1] * c[1] + row[2] * c[2])
    }
}

/// An image as a GPU samples it: its first layer as the host holds the file
/// (its own samples and 1 to 4 channels, over the data window), and how
/// its colors become linear light in the space its reader works in, none
/// for data, read as stored. What an engine's Texture prim and a shaded
/// view's image are; the reader picks its own GPU form of it, and reads a
/// texel as [`texel`](TextureImage::texel) does.
#[derive(Clone, Debug, PartialEq)]
pub struct TextureImage {
    /// The image.
    pub image: Image,
    /// Its colors' decoding; none: data.
    pub decoding: Option<Decoding>,
}

impl TextureImage {
    /// Texel (`x`, `y`) of its first layer as its reader takes it, what a
    /// GPU reader's shader makes of a fetched texel: one channel grey in
    /// red, green and blue, two grey and alpha, a missing alpha 1; a color
    /// through the decoding (its curve, then its matrix) and made straight
    /// where the layer is premultiplied; data as stored. Outside the data
    /// window, in a tile of nothing, or of a held layer, every channel
    /// reads 0.
    ///
    /// ```
    /// use kinogaki_geometry::image::{Decoding, Encoding, Format, Image, Rect, Sample};
    /// use kinogaki_geometry::image::{TextureImage, Transfer};
    /// let grey = Format {
    ///     channels: ["Y"].map(Into::into).into(),
    ///     ..Format::rgba(Sample::U8, Encoding::color("sRGB", false))
    /// };
    /// let window = Rect::new([0, 0], [1, 1]).unwrap();
    /// let image = Image::builder(window).plane(grey, &[255u8]).build().unwrap();
    /// let decoding = Decoding { transfer: Transfer::SRGB, matrix: Decoding::IDENTITY.matrix };
    /// let texture = TextureImage { image, decoding: Some(decoding) };
    /// assert_eq!(texture.texel(0, 0), [1.0; 4]);
    /// ```
    pub fn texel(&self, x: i32, y: i32) -> [f32; 4] {
        let Some(layer) = self.image.layers().first() else {
            return [0.0; 4];
        };
        let format = layer.format();
        let [r, g, b, a] = super::spread(format.channels.len(), self.image.read_pixel(0, x, y));
        let Some(decoding) = &self.decoding else {
            return [r, g, b, a];
        };
        let rgb = decoding.apply([r, g, b]);
        // Straight after the matrix: dividing by alpha commutes with it.
        match format.encoding.premultiplied() && a > 0.0 {
            true => [rgb[0] / a, rgb[1] / a, rgb[2] / a, a],
            false => [rgb[0], rgb[1], rgb[2], a],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::image::{Encoding, Format, Rect, Sample};

    /// The IEC 61966-2-1 formula the GPU's sRGB formats implement.
    fn iec(v: f32) -> f32 {
        match v <= 0.04045 {
            true => v / 12.92,
            false => ((v + 0.055) / 1.055).powf(2.4),
        }
    }

    /// An 8-bit sRGB texture's GPU decode lands within a hundredth of a
    /// step of what the host's curve gives.
    #[test]
    fn srgbs_curve_is_iecs_within_a_hundredth_of_an_8_bit_step() {
        for code in 0..255u8 {
            let (v, next) = (f32::from(code) / 255.0, f32::from(code + 1) / 255.0);
            let d = Transfer::SRGB.decode(v);
            let step = iec(next) - iec(v);
            assert!((d - iec(v)).abs() <= step / 100.0, "{code}: {d} {}", iec(v));
        }
        assert_eq!(Transfer::SRGB.decode(-0.5), -Transfer::SRGB.decode(0.5));
        assert_eq!(Transfer::Power(2.2).decode(0.5), 0.5f32.powf(2.2));
        assert_eq!(Transfer::Linear.decode(-3.0), -3.0);
    }

    #[test]
    fn a_decoding_runs_its_curve_then_its_matrix() {
        let swap = Decoding {
            transfer: Transfer::Power(2.0),
            matrix: [[0.0, 1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 2.0]],
        };
        assert_eq!(swap.apply([0.5, 1.0, 0.5]), [1.0, 0.25, 0.5]);
        assert_eq!(Decoding::IDENTITY.apply([0.1, 0.2, 3.0]), [0.1, 0.2, 3.0]);
    }

    /// A texture of `channels` channels of floats, `encoding`, one pixel of
    /// `values`, read through `decoding`.
    fn texel(
        channels: usize,
        encoding: Encoding,
        values: &[f32],
        decoding: Option<Decoding>,
    ) -> [f32; 4] {
        let format = Format {
            channels: Format::rgba(Sample::F32, Encoding::Raw).channels[..channels].into(),
            ..Format::rgba(Sample::F32, encoding)
        };
        let bytes: Vec<u8> = values.iter().flat_map(|v| v.to_ne_bytes()).collect();
        let window = Rect::new([0, 0], [1, 1]).unwrap();
        let image = Image::builder(window)
            .plane(format, &bytes)
            .build()
            .unwrap();
        TextureImage { image, decoding }.texel(0, 0)
    }

    #[test]
    fn a_texel_spreads_its_channels_decodes_and_is_made_straight() {
        let color = Encoding::color("space", false);
        let half = Decoding {
            transfer: Transfer::Linear,
            matrix: [[0.5, 0.0, 0.0], [0.0, 0.5, 0.0], [0.0, 0.0, 0.5]],
        };
        assert_eq!(
            texel(1, color.clone(), &[0.8], Some(half)),
            [0.4, 0.4, 0.4, 1.0]
        );
        assert_eq!(
            texel(2, color.clone(), &[0.8, 0.5], Some(half)),
            [0.4, 0.4, 0.4, 0.5]
        );
        assert_eq!(
            texel(3, color.clone(), &[0.2, 0.4, 0.6], Some(half)),
            [0.1, 0.2, 0.3, 1.0]
        );
        let premultiplied = Encoding::color("space", true);
        let made = texel(4, premultiplied, &[0.2, 0.4, 0.6, 0.5], Some(half));
        assert_eq!(made, [0.2, 0.4, 0.6, 0.5], "decoded, then straight");
        assert_eq!(
            texel(1, Encoding::Raw, &[0.8], None),
            [0.8, 0.8, 0.8, 1.0],
            "data as stored"
        );
        let outside = TextureImage {
            image: Image::builder(Rect::new([0, 0], [1, 1]).unwrap())
                .plane(Format::rgba(Sample::F32, Encoding::Raw), &[0u8; 16])
                .build()
                .unwrap(),
            decoding: None,
        };
        assert_eq!(outside.texel(1, 0), [0.0; 4]);
    }
}
