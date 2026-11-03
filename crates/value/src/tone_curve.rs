//! Tone curves: what a [`ValueKind::ToneCurves`](crate::ValueKind::ToneCurves)
//! value holds ([`Value::ToneCurves`](crate::Value::ToneCurves)), one
//! [`ToneCurve`] a channel.
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
    /// Its key in files (`monotone`, `linear`).
    pub fn key(self) -> &'static str {
        match self {
            Interpolation::Monotone => "monotone",
            Interpolation::Linear => "linear",
        }
    }

    /// The interpolation whose [`key`](Self::key) is `key`.
    pub fn of_key(key: &str) -> Option<Interpolation> {
        [Interpolation::Monotone, Interpolation::Linear]
            .into_iter()
            .find(|i| i.key() == key)
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

    /// Whether it is a curve: at least two keys, finite, x ascending.
    pub fn is_valid(&self) -> bool {
        self.keys.len() >= 2
            && self.keys.iter().flatten().all(|v| v.is_finite())
            && self.keys.windows(2).all(|k| k[0][0] <= k[1][0])
    }
}
