//! [`Color`]: what everything on screen is drawn in, premultiplied sRGBA
//! bytes.
//!
//! The bytes and every formula here are egui's `Color32` (from `ecolor`),
//! copied exactly, so a host built on egui passes a [`Color`] through
//! unchanged and the results match to the bit (see
//! `THIRD_PARTY_NOTICES.md`).

/// A color as the screen takes it: sRGB-encoded red, green and blue with
/// the alpha already multiplied in, then the alpha (egui's `Color32`).
///
/// A zero alpha with nonzero color is *additive*: it brightens what it
/// is drawn over. Build colors with [`Color::rgb`], [`Color::gray`] and
/// [`Color::unmultiplied`]; [`Color::premultiplied`] takes the bytes as
/// they are.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Color(pub [u8; 4]);

impl Color {
    /// Nothing: fully transparent black.
    pub const TRANSPARENT: Color = Color([0, 0, 0, 0]);
    /// Opaque black.
    pub const BLACK: Color = Color::rgb(0, 0, 0);
    /// Opaque white.
    pub const WHITE: Color = Color::rgb(255, 255, 255);

    /// Opaque sRGB red, green and blue.
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Color([r, g, b, 255])
    }

    /// Opaque gray of sRGB level `l`.
    pub const fn gray(l: u8) -> Self {
        Color([l, l, l, 255])
    }

    /// The premultiplied bytes as they are.
    pub const fn premultiplied(r: u8, g: u8, b: u8, a: u8) -> Self {
        Color([r, g, b, a])
    }

    /// sRGB red, green and blue at opacity `a`, multiplied in here (a
    /// color picker's values).
    pub const fn unmultiplied(r: u8, g: u8, b: u8, a: u8) -> Self {
        match a {
            0 => Self::TRANSPARENT,
            255 => Self::rgb(r, g, b),
            a => Color([
                mul_frac_round(r, a),
                mul_frac_round(g, a),
                mul_frac_round(b, a),
                a,
            ]),
        }
    }

    /// Black at opacity `a`.
    pub const fn black_alpha(a: u8) -> Self {
        Color([0, 0, 0, a])
    }

    /// White at opacity `a`.
    pub const fn white_alpha(a: u8) -> Self {
        Color([a, a, a, a])
    }

    /// An opaque color of linear-light RGB (each channel 0 to 1, clamped),
    /// encoded to sRGB: a working-space swatch shown as it is.
    pub fn from_linear_rgb([r, g, b]: [f32; 3]) -> Self {
        Color([
            gamma_u8_from_linear_f32(r),
            gamma_u8_from_linear_f32(g),
            gamma_u8_from_linear_f32(b),
            255,
        ])
    }

    /// Red, alpha multiplied in.
    pub const fn r(self) -> u8 {
        self.0[0]
    }

    /// Green, alpha multiplied in.
    pub const fn g(self) -> u8 {
        self.0[1]
    }

    /// Blue, alpha multiplied in.
    pub const fn b(self) -> u8 {
        self.0[2]
    }

    /// Opacity.
    pub const fn a(self) -> u8 {
        self.0[3]
    }

    /// Whether it is fully opaque.
    pub const fn is_opaque(self) -> bool {
        self.a() == 255
    }

    /// The red, green and blue with the alpha divided out again, then the
    /// alpha (close to what [`Color::unmultiplied`] took; translucent
    /// colors may be a little off from rounding).
    pub fn to_unmultiplied(self) -> [u8; 4] {
        let [r, g, b, a] = self.0;
        match a {
            0 | 255 => self.0,
            a => {
                let factor = 255.0 / a as f32;
                [
                    fast_round(factor * r as f32),
                    fast_round(factor * g as f32),
                    fast_round(factor * b as f32),
                    a,
                ]
            }
        }
    }

    /// Scaled by `factor` (0 or more) in sRGB, alpha included: 0.5 makes it
    /// half as opaque, perceptually. The usual way to fade a color.
    pub fn gamma_multiply(self, factor: f32) -> Self {
        debug_assert!(
            0.0 <= factor && factor.is_finite(),
            "factor should be finite, but was {factor}"
        );
        let [r, g, b, a] = self.0;
        Color([
            (r as f32 * factor + 0.5) as u8,
            (g as f32 * factor + 0.5) as u8,
            (b as f32 * factor + 0.5) as u8,
            (a as f32 * factor + 0.5) as u8,
        ])
    }

    /// Scaled by `factor / 255` in sRGB, alpha included.
    pub fn gamma_multiply_u8(self, factor: u8) -> Self {
        let [r, g, b, a] = self.0;
        let factor = factor as u32;
        Color([
            ((r as u32 * factor + 127) / 255) as u8,
            ((g as u32 * factor + 127) / 255) as u8,
            ((b as u32 * factor + 127) / 255) as u8,
            ((a as u32 * factor + 127) / 255) as u8,
        ])
    }

    /// Scaled by `factor` (0 or more) in linear light, alpha included: 0.5
    /// makes it half as opaque physically, which looks fainter than
    /// [`Color::gamma_multiply`]'s half.
    pub fn linear_multiply(self, factor: f32) -> Self {
        debug_assert!(
            0.0 <= factor && factor.is_finite(),
            "factor should be finite, but was {factor}"
        );
        let [r, g, b, a] = to_linear(self);
        from_linear([r * factor, g * factor, b * factor, a * factor])
    }

    /// `on_top` drawn over this color, in sRGB (as browsers and image
    /// editors blend).
    pub fn blend(self, on_top: Color) -> Self {
        let under = self.gamma_multiply_u8(255 - on_top.a());
        Color(std::array::from_fn(|i| {
            under.0[i].saturating_add(on_top.0[i])
        }))
    }

    /// Opaque: the alpha divided out in linear light (an additive or
    /// transparent color keeps its channels).
    pub fn to_opaque(self) -> Self {
        let [r, g, b, a] = to_linear(self);
        if a == 0.0 {
            from_linear([r, g, b, 1.0])
        } else {
            from_linear([r / a, g / a, b / a, 1.0])
        }
    }

    /// The relative luminance (WCAG 2) of the color as drawn over black,
    /// 0 to 1.
    pub fn luminance(self) -> f32 {
        let l = linear_f32_from_gamma_u8;
        0.2126 * l(self.r()) + 0.7152 * l(self.g()) + 0.0722 * l(self.b())
    }
}

/// The color in premultiplied linear light, 0 to 1 (egui's `Rgba`).
fn to_linear(color: Color) -> [f32; 4] {
    let [r, g, b, a] = color.0;
    if a == 0 {
        // Additive, or completely transparent.
        [
            linear_f32_from_gamma_u8(r),
            linear_f32_from_gamma_u8(g),
            linear_f32_from_gamma_u8(b),
            0.0,
        ]
    } else {
        let a = a as f32 / 255.0;
        [
            linear_from_gamma(r as f32 / (255.0 * a)) * a,
            linear_from_gamma(g as f32 / (255.0 * a)) * a,
            linear_from_gamma(b as f32 / (255.0 * a)) * a,
            a,
        ]
    }
}

/// Premultiplied linear light back to a [`Color`].
fn from_linear([r, g, b, a]: [f32; 4]) -> Color {
    if a == 0.0 {
        // Additive, or completely transparent.
        Color([
            gamma_u8_from_linear_f32(r),
            gamma_u8_from_linear_f32(g),
            gamma_u8_from_linear_f32(b),
            0,
        ])
    } else {
        Color([
            fast_round(gamma_u8_from_linear_f32(r / a) as f32 * a),
            fast_round(gamma_u8_from_linear_f32(g / a) as f32 * a),
            fast_round(gamma_u8_from_linear_f32(b / a) as f32 * a),
            fast_round(a * 255.0),
        ])
    }
}

/// sRGB 0–255 to linear 0–1.
fn linear_f32_from_gamma_u8(s: u8) -> f32 {
    if s <= 10 {
        s as f32 / 3294.6
    } else {
        ((s as f32 + 14.025) / 269.025).powf(2.4)
    }
}

/// Linear 0–1 to sRGB 0–255, clamped.
fn gamma_u8_from_linear_f32(l: f32) -> u8 {
    if l <= 0.0 {
        0
    } else if l <= 0.0031308 {
        fast_round(3294.6 * l)
    } else if l <= 1.0 {
        fast_round(269.025 * l.powf(1.0 / 2.4) - 14.025)
    } else {
        255
    }
}

/// sRGB 0–1 to linear 0–1, unclamped (negative values mirror).
fn linear_from_gamma(gamma: f32) -> f32 {
    if gamma < 0.0 {
        -linear_from_gamma(-gamma)
    } else if gamma <= 0.04045 {
        gamma / 12.92
    } else {
        ((gamma + 0.055) / 1.055).powf(2.4)
    }
}

/// Rounded to the nearest byte, saturating.
const fn fast_round(r: f32) -> u8 {
    (r + 0.5) as _
}

/// `val * frac / 255`, rounded, in integers.
const fn mul_frac_round(val: u8, frac: u8) -> u8 {
    let p = (val as u16) * (frac as u16) + 128;
    ((p + (p >> 8)) >> 8) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    fn samples() -> impl Iterator<Item = [u8; 4]> {
        [
            [0, 0, 0, 0],
            [0, 0, 0, 255],
            [10, 0, 30, 0],
            [10, 0, 30, 40],
            [10, 100, 200, 0],
            [10, 100, 200, 100],
            [10, 100, 200, 200],
            [10, 100, 200, 255],
            [10, 100, 200, 40],
            [10, 20, 0, 0],
            [10, 20, 0, 255],
            [10, 20, 30, 255],
            [10, 20, 30, 40],
            [255, 255, 255, 0],
            [255, 255, 255, 255],
        ]
        .into_iter()
    }

    #[test]
    fn every_srgb_byte_survives_linear_and_back() {
        for b in 0..=255 {
            let l = linear_f32_from_gamma_u8(b);
            assert!((0.0..=1.0).contains(&l));
            assert_eq!(gamma_u8_from_linear_f32(l), b);
        }
    }

    #[test]
    fn additive_colors_brighten_and_opaque_ones_cover() {
        let opaque = Color::rgb(40, 50, 60);
        let additive = Color::premultiplied(255, 127, 10, 0);
        assert_eq!(additive.blend(opaque), opaque);
        assert_eq!(opaque.blend(additive), Color::rgb(255, 177, 70));
    }

    #[test]
    fn blending_is_in_srgb() {
        let opaque = Color::rgb(0x60, 0x60, 0x60);
        let translucent = Color::unmultiplied(168, 65, 65, 79);
        assert_eq!(translucent.blend(opaque), opaque);
        let blend = |dest: u8, src: u8, alpha: u8| {
            let (src, dest, alpha) = (
                src as f32 / 255.0,
                dest as f32 / 255.0,
                alpha as f32 / 255.0,
            );
            fast_round((src * alpha + dest * (1.0 - alpha)) * 255.0)
        };
        assert_eq!(
            opaque.blend(translucent),
            Color::rgb(
                blend(0x60, 168, 79),
                blend(0x60, 65, 79),
                blend(0x60, 65, 79)
            )
        );
    }

    #[test]
    fn unmultiplied_round_trips_within_rounding() {
        for rgba in samples() {
            let [r, g, b, a] = rgba;
            if a == 0 {
                continue;
            }
            let back = Color::unmultiplied(r, g, b, a).to_unmultiplied();
            if a == 255 {
                assert_eq!(rgba, back);
            } else {
                for (x, y) in rgba.iter().zip(&back) {
                    assert!(x.abs_diff(*y) <= 3, "{rgba:?} != {back:?}");
                }
            }
        }
    }

    #[test]
    fn black_and_white_alphas_agree_with_their_definitions() {
        for a in 0..=255 {
            assert_eq!(Color::white_alpha(a), Color::unmultiplied(255, 255, 255, a));
            assert_eq!(Color::white_alpha(a), Color::WHITE.gamma_multiply_u8(a));
            assert_eq!(Color::black_alpha(a), Color::unmultiplied(0, 0, 0, a));
            assert_eq!(Color::black_alpha(a), Color::BLACK.gamma_multiply_u8(a));
        }
    }

    #[test]
    fn linear_light_round_trips() {
        for rgba in samples() {
            let [r, g, b, a] = rgba;
            let color = Color::unmultiplied(r, g, b, a);
            assert_eq!(from_linear(to_linear(color)), color);
        }
        assert_eq!(
            from_linear([0.5, 0.0, 0.0, 0.5]),
            Color::unmultiplied(255, 0, 0, 128)
        );
    }

    #[test]
    fn the_integer_fraction_matches_float_rounding() {
        for x in (0..=255u8).step_by(4) {
            for a in (1..=255u8).step_by(4) {
                assert_eq!(
                    fast_round(x as f32 * (a as f32 / 255.0)),
                    mul_frac_round(x, a)
                );
            }
        }
    }

    #[test]
    fn multiplying_fades() {
        let c = Color::rgb(200, 100, 50);
        assert_eq!(
            c.gamma_multiply(0.5),
            Color::premultiplied(100, 50, 25, 128)
        );
        assert_eq!(c.linear_multiply(1.0), c);
        assert_eq!(c.linear_multiply(0.0), Color::TRANSPARENT);
        assert_eq!(c.gamma_multiply(0.5).to_opaque().a(), 255);
        assert_eq!(
            Color::from_linear_rgb([1.0, 0.0, 2.0]),
            Color::rgb(255, 0, 255)
        );
        assert_eq!(Color::from_linear_rgb([0.5; 3]), Color::gray(188));
    }
}
