//! Tone curves as a [`Kind::ToneCurves`](super::Kind::ToneCurves) field
//! holds them: [`ToneCurve`]s encoded in a `Value::Numbers` with
//! [`encode`], read back with [`decode`].
//!
//! A curve maps 0–1 to 0–1 through its keys, sorted by x: a monotone cubic
//! between them (smooth, never overshooting) or straight lines, going on
//! straight along its end tangents past the first and last key. Sampling
//! it is the owner's: the host only edits the keys.

/// How a curve runs between its keys.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Interpolation {
    /// A monotone cubic: smooth, never overshooting the keys.
    #[default]
    Monotone,
    /// Straight lines.
    Linear,
}

impl Interpolation {
    /// Its code in an encoding.
    pub fn code(self) -> f64 {
        match self {
            Interpolation::Monotone => 0.0,
            Interpolation::Linear => 1.0,
        }
    }

    /// The interpolation with `code` (Monotone for anything but 1).
    pub fn from_code(code: f64) -> Interpolation {
        if code == 1.0 {
            Interpolation::Linear
        } else {
            Interpolation::Monotone
        }
    }
}

/// One tone curve: keys (x, y), sorted by x, and how it runs between them.
#[derive(Clone, Debug, PartialEq)]
pub struct ToneCurve {
    /// The keys, by x.
    pub keys: Vec<[f64; 2]>,
    /// How it runs between them.
    pub interpolation: Interpolation,
}

impl Default for ToneCurve {
    /// [`ToneCurve::identity`].
    fn default() -> Self {
        Self::identity()
    }
}

impl ToneCurve {
    /// The diagonal: keys at (0, 0) and (1, 1), smooth.
    pub fn identity() -> Self {
        ToneCurve {
            keys: vec![[0.0, 0.0], [1.0, 1.0]],
            interpolation: Interpolation::Monotone,
        }
    }
}

/// The numbers one curve takes in an encoding of `capacity` keys a curve.
pub fn stride(capacity: usize) -> usize {
    2 + 2 * capacity
}

/// `curves` as numbers: per curve its interpolation's code, its key count
/// (at most `capacity`), then `capacity` (x, y) pairs, those past the count
/// 0.
pub fn encode(curves: &[ToneCurve], capacity: usize) -> Vec<f64> {
    let mut out = Vec::with_capacity(curves.len() * stride(capacity));
    for curve in curves {
        let count = curve.keys.len().min(capacity);
        out.push(curve.interpolation.code());
        out.push(count as f64);
        for k in 0..capacity {
            out.extend(curve.keys.get(k).filter(|_| k < count).unwrap_or(&[0.0; 2]));
        }
    }
    out
}

/// The curves [`encode`] wrote. A trailing partial curve is dropped; a
/// count is clamped to `capacity`, keys that are not finite are dropped,
/// and a curve left with fewer than two keys is the identity.
pub fn decode(numbers: &[f64], capacity: usize) -> Vec<ToneCurve> {
    numbers
        .chunks_exact(stride(capacity))
        .map(|chunk| {
            let count = if chunk[1].is_finite() {
                (chunk[1].max(0.0).round() as usize).min(capacity)
            } else {
                0
            };
            let keys: Vec<[f64; 2]> = chunk[2..2 + 2 * count]
                .as_chunks::<2>()
                .0
                .iter()
                .copied()
                .filter(|p| p[0].is_finite() && p[1].is_finite())
                .collect();
            if keys.len() < 2 {
                return ToneCurve::identity();
            }
            ToneCurve {
                keys,
                interpolation: Interpolation::from_code(chunk[0]),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encoding_round_trips_and_tolerates_garbage() {
        let curves = [
            ToneCurve::identity(),
            ToneCurve {
                keys: vec![[0.0, 0.1], [0.5, 0.4], [1.0, 0.95]],
                interpolation: Interpolation::Linear,
            },
        ];
        let numbers = encode(&curves, 4);
        assert_eq!(numbers.len(), 2 * stride(4));
        assert_eq!(decode(&numbers, 4), curves);
        // A short tail is dropped, a wild count clamped, NaN keys dropped.
        let mut bad = numbers.clone();
        bad.push(1.0);
        bad[stride(4) + 1] = 99.0;
        assert_eq!(decode(&bad, 4).len(), 2);
        bad[1] = f64::NAN;
        assert_eq!(decode(&bad, 4)[0], ToneCurve::identity());
        bad[stride(4) + 2] = f64::NAN;
        assert!(decode(&bad, 4)[1].keys.iter().all(|k| k[0].is_finite()));
        // Keys past the capacity are left out.
        let many = ToneCurve {
            keys: (0..6).map(|i| [i as f64 / 5.0; 2]).collect(),
            interpolation: Interpolation::Monotone,
        };
        assert_eq!(decode(&encode(&[many], 3), 3)[0].keys.len(), 3);
    }
}
