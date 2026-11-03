//! Surface colors as the viewport carries them: linear, in the host's
//! working space, packed as RGB9E5 (pure arithmetic, no GPU). A look's
//! colors ([`ColorLayout`](crate::ColorLayout)) are these words.

/// `rgb` (linear, negatives clamped to 0) as RGB9E5: three 9-bit mantissas
/// sharing a 5-bit exponent biased by 15, the format GPUs filter shared-
/// exponent textures in. Never 0, which means "no color" (black packs with
/// exponent 1).
pub fn pack_linear(rgb: [f32; 3]) -> u32 {
    const MAX: f32 = 65408.0;
    let c = rgb.map(|v| {
        if v.is_finite() {
            v.clamp(0.0, MAX)
        } else {
            0.0
        }
    });
    let largest = c[0].max(c[1]).max(c[2]);
    // The exponent that puts the largest channel's mantissa below 512.
    let mut exponent = if largest > 0.0 {
        (largest.log2().floor() as i32 + 1).max(-15) + 15
    } else {
        0
    };
    let scale = |e: i32| 2f32.powi(e - 15 - 9);
    if (largest / scale(exponent)).round() >= 512.0 {
        exponent += 1;
    }
    let exponent = exponent.clamp(1, 31);
    let m = c.map(|v| ((v / scale(exponent)).round() as u32).min(511));
    m[0] | m[1] << 9 | m[2] << 18 | (exponent as u32) << 27
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What a packed color unpacks to in the shader (`packed_linear`).
    fn unpack_linear(c: u32) -> [f32; 3] {
        let scale = 2f32.powi((c >> 27) as i32 - 24);
        [c & 511, (c >> 9) & 511, (c >> 18) & 511].map(|m| m as f32 * scale)
    }

    #[test]
    fn packed_colors_round_trip_within_a_mantissa_step_and_are_never_zero() {
        for rgb in [
            [0.0, 0.0, 0.0],
            [1.0, 0.5, 0.25],
            [0.18, 0.18, 0.18],
            [0.001, 0.02, 0.3],
            [12.0, 0.0, 3.0],
            [-1.0, 2.0, f32::NAN],
        ] {
            let packed = pack_linear(rgb);
            assert_ne!(packed, 0);
            let back = unpack_linear(packed);
            let largest = rgb
                .iter()
                .filter(|v| v.is_finite())
                .fold(0f32, |m, v| m.max(*v));
            for k in 0..3 {
                let want = if rgb[k].is_finite() {
                    rgb[k].max(0.0)
                } else {
                    0.0
                };
                assert!(
                    (back[k] - want).abs() <= largest / 511.0 + 1e-9,
                    "{rgb:?} channel {k}: {}",
                    back[k]
                );
            }
        }
    }
}
