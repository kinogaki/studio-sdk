//! Tone curves: what a [`ValueKind::ToneCurves`](crate::ValueKind::ToneCurves)
//! value holds ([`Value::ToneCurves`](crate::Value::ToneCurves)), one
//! [`ToneCurve`] a channel.
//!
//! A curve maps 0–1 to 0–1 through its keys, sorted by x: a monotone cubic
//! between them (smooth, never overshooting) or straight lines, going on
//! straight along its end tangents past the first and last key
//! ([`ToneCurve::evaluate`], the one definition of its values).

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

    /// The curve at `x`: through its keys as its interpolation runs (a
    /// monotone cubic's tangents the harmonic mean of its neighbouring
    /// slopes, zero where they turn), straight along its end tangents past
    /// the first and last key. Keys at one x make a step there. The one
    /// definition of a tone curve's values: an owner samples with it, and
    /// the host resamples a curve with it between animation keys.
    pub fn evaluate(&self, x: f64) -> f64 {
        let k = &self.keys;
        let n = k.len();
        if n < 2 {
            return k.first().map_or(x, |k| k[1]);
        }
        let slope = |i: usize| {
            let dx = k[i + 1][0] - k[i][0];
            if dx > 0.0 {
                (k[i + 1][1] - k[i][1]) / dx
            } else {
                0.0
            }
        };
        let tangent = |i: usize| match (self.interpolation, i) {
            (Interpolation::Linear, _) | (_, 0) => slope(i.min(n - 2)),
            (_, i) if i == n - 1 => slope(n - 2),
            (_, i) => match (slope(i - 1), slope(i)) {
                (a, b) if a * b <= 0.0 => 0.0,
                (a, b) => 2.0 / (1.0 / a + 1.0 / b),
            },
        };
        if x <= k[0][0] {
            return k[0][1] + tangent(0) * (x - k[0][0]);
        }
        if x >= k[n - 1][0] {
            return k[n - 1][1] + tangent(n - 1) * (x - k[n - 1][0]);
        }
        let i = k.partition_point(|key| key[0] <= x) - 1;
        let (x0, y0, x1, y1) = (k[i][0], k[i][1], k[i + 1][0], k[i + 1][1]);
        let h = x1 - x0;
        let u = (x - x0) / h;
        if self.interpolation == Interpolation::Linear {
            return y0 + (y1 - y0) * u;
        }
        let (u2, u3) = (u * u, u * u * u);
        let (m0, m1) = (tangent(i) * h, tangent(i + 1) * h);
        (2.0 * u3 - 3.0 * u2 + 1.0) * y0
            + (u3 - 2.0 * u2 + u) * m0
            + (-2.0 * u3 + 3.0 * u2) * y1
            + (u3 - u2) * m1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_curve_runs_through_its_keys_and_straight_past_its_ends() {
        let s = ToneCurve {
            keys: vec![[0.0, 0.0], [0.25, 0.1], [0.75, 0.9], [1.0, 1.0]],
            interpolation: Interpolation::Monotone,
        };
        for k in &s.keys {
            assert_eq!(s.evaluate(k[0]), k[1]);
        }
        // Monotone: never back down between rising keys.
        let ys: Vec<f64> = (0..=100)
            .map(|i| s.evaluate(f64::from(i) / 100.0))
            .collect();
        assert!(ys.windows(2).all(|w| w[1] >= w[0]), "{ys:?}");
        // Past the ends along the end tangents (the end segments' slopes).
        assert!((s.evaluate(-0.1) - -0.04).abs() < 1e-12);
        assert!((s.evaluate(1.1) - 1.04).abs() < 1e-12);
        let lines = ToneCurve {
            interpolation: Interpolation::Linear,
            ..s.clone()
        };
        assert!((lines.evaluate(0.5) - 0.5).abs() < 1e-12);
        assert!((lines.evaluate(0.125) - 0.05).abs() < 1e-12);
        // A turn flattens the tangent there; one x twice makes a step.
        let peak = ToneCurve {
            keys: vec![[0.0, 0.0], [0.5, 1.0], [1.0, 0.0]],
            interpolation: Interpolation::Monotone,
        };
        assert!((0..=100).all(|i| peak.evaluate(f64::from(i) / 100.0) <= 1.0));
        let step = ToneCurve {
            keys: vec![[0.0, 0.0], [0.5, 0.0], [0.5, 1.0], [1.0, 1.0]],
            interpolation: Interpolation::Linear,
        };
        assert_eq!((step.evaluate(0.49), step.evaluate(0.5)), (0.0, 1.0));
    }
}
