//! [`Decoding`]: how a color layer's values become linear light in another
//! space, as a GPU runs it after fetching texels; [`Codes`], a curve's value
//! at every code of an 8- or 16-bit sample; and [`TextureImage`], an image
//! with the decoding its reader applies.
use super::{Image, Sample};
use std::sync::{Arc, Mutex};

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

    /// The curve as bits, telling curves apart exactly.
    fn bits(self) -> [u32; 3] {
        match self {
            Transfer::Linear => [0, 0, 0],
            Transfer::Exponent { gamma, offset } => [1, gamma.to_bits(), offset.to_bits()],
            Transfer::Power(gamma) => [2, gamma.to_bits(), 0],
        }
    }
}

/// A transfer function's value at every code of an 8- or 16-bit sample:
/// [`Transfer::decode`] of each code over the largest code (the value
/// [`Sample::read`] gives for it), so a converter looks a stored value up
/// rather than computing the curve, a power, for each one. Made once a
/// process for each curve and width, and shared.
///
/// ```
/// use kinogaki_geometry::image::{Codes, Sample, Transfer};
/// let codes = Codes::of(Transfer::SRGB, Sample::U8).unwrap();
/// let stored = Sample::U8.read(&[187]);
/// assert_eq!(codes.decode(stored), Transfer::SRGB.decode(stored));
/// assert!(Codes::of(Transfer::SRGB, Sample::F16).is_none());
/// ```
#[derive(Clone, Debug)]
pub struct Codes(Arc<[f32]>);

impl Codes {
    /// `transfer`'s table for values of `sample`, an 8- or 16-bit one's;
    /// none for the others (their values are no codes) and for a linear
    /// curve (nothing to look up).
    pub fn of(transfer: Transfer, sample: Sample) -> Option<Codes> {
        let top = match sample {
            Sample::U8 => u32::from(u8::MAX),
            Sample::U16 => u32::from(u16::MAX),
            _ => return None,
        };
        if transfer == Transfer::Linear {
            return None;
        }
        // The tables made so far, by curve and largest code: a process
        // reads through a few curves.
        type Key = ([u32; 3], u32);
        static MADE: Mutex<Vec<(Key, Codes)>> = Mutex::new(Vec::new());
        let key = (transfer.bits(), top);
        let mut made = MADE.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((_, codes)) = made.iter().find(|(k, _)| *k == key) {
            return Some(codes.clone());
        }
        let values = (0..=top).map(|code| transfer.decode(code as f32 / top as f32));
        let codes = Codes(values.collect());
        made.push((key, codes.clone()));
        Some(codes)
    }

    /// `v`, a value of its sample (a code over the largest), as linear
    /// light: what the curve gives for it, bit for bit. A value between
    /// codes takes the nearest code's.
    pub fn decode(&self, v: f32) -> f32 {
        let last = self.0.len() - 1;
        self.0[((v * last as f32).round() as usize).min(last)]
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
        self.mixed(rgb.map(|v| self.transfer.decode(v)))
    }

    /// [`apply`](Self::apply) of `rgb`, values of an 8- or 16-bit sample,
    /// its curve looked up in `codes` (the curve's table for that sample):
    /// the same values, bit for bit, with no power computed.
    pub fn apply_codes(&self, rgb: [f32; 3], codes: &Codes) -> [f32; 3] {
        self.mixed(rgb.map(|v| codes.decode(v)))
    }

    /// Linear values `c` through the matrix.
    fn mixed(&self, c: [f32; 3]) -> [f32; 3] {
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

    /// Each curve's table holds, at every code of an 8- and a 16-bit
    /// sample, what the curve gives for the value read, bit for bit; one
    /// table a curve and width is made, and shared.
    #[test]
    fn a_curves_codes_are_its_values_at_every_code() {
        let curves = [
            Transfer::SRGB,
            Transfer::Power(2.2),
            Transfer::Exponent {
                gamma: 1.0 / 0.45,
                offset: 0.099,
            },
        ];
        for curve in curves {
            let eight = Codes::of(curve, Sample::U8).expect("8 bits");
            for code in 0..=u8::MAX {
                let v = Sample::U8.read(&[code]);
                assert_eq!(eight.decode(v).to_bits(), curve.decode(v).to_bits());
            }
            let sixteen = Codes::of(curve, Sample::U16).expect("16 bits");
            for code in 0..=u16::MAX {
                let v = Sample::U16.read(&code.to_ne_bytes());
                assert_eq!(sixteen.decode(v).to_bits(), curve.decode(v).to_bits());
            }
            let again = Codes::of(curve, Sample::U16).expect("16 bits");
            assert!(Arc::ptr_eq(&sixteen.0, &again.0), "made once");
            assert!(!Arc::ptr_eq(&eight.0, &sixteen.0));
        }
        assert!(Codes::of(Transfer::Linear, Sample::U8).is_none());
        assert!(Codes::of(Transfer::SRGB, Sample::F32).is_none());
        assert!(Codes::of(Transfer::SRGB, Sample::U32).is_none());
        let decoding = Decoding {
            transfer: Transfer::SRGB,
            matrix: [[0.6, 0.3, 0.1], [0.2, 0.7, 0.1], [0.0, 0.1, 0.9]],
        };
        let codes = Codes::of(Transfer::SRGB, Sample::U8).expect("8 bits");
        let rgb = [17u8, 128, 250].map(|c| Sample::U8.read(&[c]));
        assert_eq!(decoding.apply_codes(rgb, &codes), decoding.apply(rgb));
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
