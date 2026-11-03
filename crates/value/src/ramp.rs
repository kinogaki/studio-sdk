//! Ramps: a value along 0–1 through keys, a number (a float ramp: a
//! profile, a falloff) or a color (a color ramp: a gradient), as Houdini's
//! ramp parameters have them. This is the one definition: a node's ramp
//! parameter, Studio's ramp widget and a render engine's color ramps hold
//! a [`Ramp`] ([`Value::Ramp`](crate::Value::Ramp)) and evaluate it with
//! [`Ramp::evaluate`] or [`Ramp::evaluate_channel`], getting the same
//! values.
//!
//! Keys sit at positions in 0–1, sorted. Each key's [`Interpolation`] says
//! how the ramp runs from it to the next key; before the first key and past
//! the last the ramp holds their values. A color ramp interpolates each
//! channel the same way. A ramp holds any number of keys; an engine that
//! takes fewer says so in its kind ([`ValueKind::Ramp`](crate::ValueKind::Ramp)'s
//! `most`), and a longer ramp is refused, never cut short.

/// How a ramp runs from a key to the next.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Interpolation {
    /// The key's value up to the next key.
    Constant,
    /// A straight line.
    #[default]
    Linear,
    /// A Catmull-Rom spline through the keys: smooth, flat at the ends.
    CatmullRom,
    /// A monotone cubic through the keys: smooth, never overshooting.
    Monotone,
    /// A uniform cubic B-spline over the keys' values: smoothest, passing
    /// near the keys rather than through them.
    BSpline,
}

impl Interpolation {
    /// Every interpolation, by code.
    pub const ALL: [Interpolation; 5] = [
        Interpolation::Constant,
        Interpolation::Linear,
        Interpolation::CatmullRom,
        Interpolation::Monotone,
        Interpolation::BSpline,
    ];

    /// What a UI calls it.
    pub fn label(self) -> &'static str {
        match self {
            Interpolation::Constant => "Constant",
            Interpolation::Linear => "Linear",
            Interpolation::CatmullRom => "Catmull-Rom",
            Interpolation::Monotone => "Monotone",
            Interpolation::BSpline => "B-Spline",
        }
    }

    /// Its key in files (`catmull_rom`).
    pub fn key(self) -> &'static str {
        match self {
            Interpolation::Constant => "constant",
            Interpolation::Linear => "linear",
            Interpolation::CatmullRom => "catmull_rom",
            Interpolation::Monotone => "monotone",
            Interpolation::BSpline => "bspline",
        }
    }

    /// The interpolation whose [`key`](Self::key) is `key`.
    pub fn of_key(key: &str) -> Option<Interpolation> {
        Self::ALL.into_iter().find(|i| i.key() == key)
    }
}

/// One key: where it sits, its value (a float ramp's in the first channel,
/// the others 0) and how the ramp runs from it to the next key.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Key {
    /// Its position, 0–1.
    pub position: f64,
    /// Its value: a float ramp's number first, or a color's r, g, b.
    pub value: [f64; 3],
    /// How the ramp runs from it to the next key.
    pub interpolation: Interpolation,
}

impl Key {
    /// A float ramp's key.
    pub fn float(position: f64, value: f64, interpolation: Interpolation) -> Self {
        Self {
            position,
            value: [value, 0.0, 0.0],
            interpolation,
        }
    }

    /// A color ramp's key.
    pub fn color(position: f64, rgb: [f64; 3], interpolation: Interpolation) -> Self {
        Self {
            position,
            value: rgb,
            interpolation,
        }
    }
}

/// A ramp: a float or a color ramp, and its keys by position (at least
/// one).
#[derive(Clone, Debug, PartialEq)]
pub struct Ramp {
    /// A color ramp (three channels) rather than a float ramp (one).
    pub color: bool,
    /// The keys, by position.
    pub keys: Vec<Key>,
}

impl Ramp {
    /// The ramp from 0 to 1 (a float ramp) or black to white (a color
    /// ramp), linear.
    pub fn default_of(color: bool) -> Self {
        let (low, high) = if color {
            ([0.0; 3], [1.0; 3])
        } else {
            ([0.0, 0.0, 0.0], [1.0, 0.0, 0.0])
        };
        Self {
            color,
            keys: vec![
                Key::color(0.0, low, Interpolation::Linear),
                Key::color(1.0, high, Interpolation::Linear),
            ],
        }
    }

    /// The channels a key's value uses: 1 or 3.
    pub fn channels(&self) -> usize {
        if self.color { 3 } else { 1 }
    }

    /// The ramp at `t`: each channel (a float ramp's in the first, the
    /// others 0).
    pub fn evaluate(&self, t: f64) -> [f64; 3] {
        let mut out = [0.0; 3];
        for (c, v) in out.iter_mut().enumerate().take(self.channels()) {
            *v = self.evaluate_channel(t, c);
        }
        out
    }

    /// Insert a key at `position` (clamped to 0–1) holding the ramp's value
    /// there and the interpolation of the key before it (Linear before the
    /// first); returns its index.
    pub fn insert(&mut self, position: f64) -> usize {
        let position = position.clamp(0.0, 1.0);
        let value = self.evaluate(position);
        let at = self.keys.partition_point(|k| k.position <= position);
        let interpolation = at
            .checked_sub(1)
            .map_or(Interpolation::Linear, |b| self.keys[b].interpolation);
        self.keys.insert(
            at,
            Key {
                position,
                value,
                interpolation,
            },
        );
        at
    }

    /// Remove key `k`, unless it is the last one left; whether it went.
    pub fn remove(&mut self, k: usize) -> bool {
        if self.keys.len() < 2 || k >= self.keys.len() {
            return false;
        }
        self.keys.remove(k);
        true
    }
}

impl Ramp {
    /// The ramp's `channel` at `t`: the first key's value at and before
    /// its position (and for a `t` that is NaN), the last key's at and
    /// past its position, and between two keys as the first one's
    /// interpolation runs. Keys at one position after the first make a
    /// step there: the ramp takes the last of them from that position on.
    pub fn evaluate_channel(&self, t: f64, channel: usize) -> f64 {
        let keys = &self.keys[..];
        let count = keys.len();
        if count == 0 {
            return 0.0;
        }
        if t.is_nan() || t <= position(keys, 0) {
            return value(keys, 0, channel);
        }
        let last = count - 1;
        if t >= position(keys, last) {
            return value(keys, last, channel);
        }
        // The span [lo, lo + 1] holding t.
        let (mut lo, mut hi) = (0, last);
        while hi - lo > 1 {
            let m = lo + (hi - lo) / 2;
            if position(keys, m) <= t {
                lo = m;
            } else {
                hi = m;
            }
        }
        let span = position(keys, lo + 1) - position(keys, lo);
        let u = (t - position(keys, lo)) / span;
        let start = value(keys, lo, channel);
        let end = value(keys, lo + 1, channel);
        match keys[lo].interpolation {
            Interpolation::Constant => start,
            Interpolation::Linear => start + (end - start) * u,
            i @ (Interpolation::CatmullRom | Interpolation::Monotone) => {
                let slope = |p| {
                    if i == Interpolation::Monotone {
                        monotone_slope(keys, p, channel)
                    } else {
                        smooth_slope(keys, p, channel)
                    }
                };
                let u2 = u * u;
                let u3 = u2 * u;
                (2.0 * u3 - 3.0 * u2 + 1.0) * start
                    + (u3 - 2.0 * u2 + u) * slope(lo) * span
                    + (3.0 * u2 - 2.0 * u3) * end
                    + (u3 - u2) * slope(lo + 1) * span
            }
            Interpolation::BSpline => {
                // The ends are mirrored, so the curve meets the end keys.
                let a = if before(keys, lo) {
                    value(keys, lo - 1, channel)
                } else {
                    2.0 * start - end
                };
                let d = if after(keys, lo + 1) {
                    value(keys, lo + 2, channel)
                } else {
                    2.0 * end - start
                };
                let u2 = u * u;
                let u3 = u2 * u;
                let v = 1.0 - u;
                (v * v * v * a
                    + (3.0 * u3 - 6.0 * u2 + 4.0) * start
                    + (-3.0 * u3 + 3.0 * u2 + 3.0 * u + 1.0) * end
                    + u3 * d)
                    / 6.0
            }
        }
    }
}

fn position(keys: &[Key], k: usize) -> f64 {
    keys[k].position
}

fn value(keys: &[Key], k: usize, channel: usize) -> f64 {
    keys[k].value[channel]
}

/// Whether key `p` has a distinct key before it.
fn before(keys: &[Key], p: usize) -> bool {
    p > 0 && position(keys, p - 1) < position(keys, p)
}

/// Whether key `p` has a distinct key after it.
fn after(keys: &[Key], p: usize) -> bool {
    p + 1 < keys.len() && position(keys, p + 1) > position(keys, p)
}

/// The slope of the line from key `p` to the next.
fn secant(keys: &[Key], p: usize, c: usize) -> f64 {
    (value(keys, p + 1, c) - value(keys, p, c)) / (position(keys, p + 1) - position(keys, p))
}

/// Catmull-Rom's tangent at key `p`: along its neighbours, flat at an end.
fn smooth_slope(keys: &[Key], p: usize, c: usize) -> f64 {
    if !before(keys, p) || !after(keys, p) {
        0.0
    } else {
        (value(keys, p + 1, c) - value(keys, p - 1, c))
            / (position(keys, p + 1) - position(keys, p - 1))
    }
}

/// The monotone cubic's tangent at key `p` (Fritsch–Butland inside, a
/// shape-preserving three-point estimate at the ends).
fn monotone_slope(keys: &[Key], p: usize, c: usize) -> f64 {
    let pos = |k| position(keys, k);
    match (before(keys, p), after(keys, p)) {
        (true, true) => {
            let left = secant(keys, p - 1, c);
            let right = secant(keys, p, c);
            if left * right <= 0.0 {
                return 0.0;
            }
            let ls = pos(p) - pos(p - 1);
            let rs = pos(p + 1) - pos(p);
            let leading = 2.0 * rs + ls;
            let trailing = rs + 2.0 * ls;
            (leading + trailing) / (leading / left + trailing / right)
        }
        (false, true) => {
            let near = secant(keys, p, c);
            if !after(keys, p + 1) {
                near
            } else {
                end_slope(
                    near,
                    secant(keys, p + 1, c),
                    pos(p + 1) - pos(p),
                    pos(p + 2) - pos(p + 1),
                )
            }
        }
        (true, false) => {
            let near = secant(keys, p - 1, c);
            if !before(keys, p - 1) {
                near
            } else {
                end_slope(
                    near,
                    secant(keys, p - 2, c),
                    pos(p) - pos(p - 1),
                    pos(p - 1) - pos(p - 2),
                )
            }
        }
        (false, false) => 0.0,
    }
}

/// An end key's tangent from the `near` and `far` secants (spans `ns`,
/// `fs`), kept from overshooting.
fn end_slope(near: f64, far: f64, ns: f64, fs: f64) -> f64 {
    let slope = ((2.0 * ns + fs) * near - ns * far) / (ns + fs);
    if slope.signum() != near.signum() {
        0.0
    } else if near.signum() != far.signum() && slope.abs() > 3.0 * near.abs() {
        3.0 * near
    } else {
        slope
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn float(keys: &[(f64, f64, Interpolation)]) -> Ramp {
        Ramp {
            color: false,
            keys: keys.iter().map(|&(p, v, i)| Key::float(p, v, i)).collect(),
        }
    }

    #[test]
    fn interpolations_meet_the_keys_and_hold_past_the_ends() {
        for i in Interpolation::ALL {
            assert_eq!(Interpolation::of_key(i.key()), Some(i));
            let r = float(&[(0.2, 0.0, i), (0.6, 1.0, i), (0.9, 0.5, i)]);
            assert_eq!(r.evaluate(0.0)[0], 0.0, "{i:?} holds before the first key");
            assert_eq!(r.evaluate(1.0)[0], 0.5, "{i:?} holds past the last key");
            if i != Interpolation::BSpline {
                assert!(
                    (r.evaluate(0.6)[0] - 1.0).abs() < 1e-12,
                    "{i:?} meets its keys"
                );
            }
        }
        assert_eq!(Interpolation::of_key("cubic"), None);
        let r = float(&[
            (0.0, 0.0, Interpolation::Constant),
            (1.0, 1.0, Interpolation::Linear),
        ]);
        assert_eq!(r.evaluate(0.99)[0], 0.0);
        let r = float(&[
            (0.0, 0.0, Interpolation::Linear),
            (1.0, 1.0, Interpolation::Linear),
        ]);
        assert!((r.evaluate(0.25)[0] - 0.25).abs() < 1e-12);
        // A monotone cubic over rising keys never overshoots; Catmull-Rom
        // through a sharp step does.
        let keys = [(0.0, 0.0), (0.45, 0.0), (0.55, 1.0), (1.0, 1.0)];
        let with = |i| float(&keys.map(|(p, v)| (p, v, i)));
        let samples = |r: Ramp| -> Vec<f64> {
            (0..=200)
                .map(|s| r.evaluate(f64::from(s) / 200.0)[0])
                .collect()
        };
        assert!(
            samples(with(Interpolation::Monotone))
                .iter()
                .all(|v| (0.0..=1.0).contains(v))
        );
        assert!(
            samples(with(Interpolation::CatmullRom))
                .iter()
                .any(|v| !(0.0..=1.0).contains(v))
        );
    }

    #[test]
    fn color_ramps_interpolate_each_channel() {
        let r = Ramp {
            color: true,
            keys: vec![
                Key::color(0.0, [0.0, 1.0, 0.5], Interpolation::Linear),
                Key::color(1.0, [1.0, 0.0, 0.5], Interpolation::Linear),
            ],
        };
        let mid = r.evaluate(0.5);
        assert!(mid.iter().all(|&v| (v - 0.5).abs() < 1e-12), "{mid:?}");
    }

    #[test]
    fn keys_insert_and_remove_in_order() {
        let mut r = Ramp::default_of(false);
        let k = r.insert(0.5);
        assert_eq!(k, 1);
        assert!(
            (r.keys[1].value[0] - 0.5).abs() < 1e-12,
            "it takes the ramp's value"
        );
        assert!(r.remove(0));
        assert!(r.remove(0));
        assert!(!r.remove(0), "the last key stays");
        assert_eq!(r.keys.len(), 1);
    }

    /// Keys at 0, 1/3, 2/3 and 1 on the line v = t, all `i`.
    fn line(i: Interpolation) -> Ramp {
        float(&[0.0, 1.0 / 3.0, 2.0 / 3.0, 1.0].map(|p| (p, p, i)))
    }

    #[test]
    fn each_interpolation_takes_its_exact_values() {
        let near = |a: f64, b: f64| (a - b).abs() < 1e-12;
        // Constant: the key's value up to the next key, which takes over at
        // its position.
        let r = float(&[
            (0.0, 0.2, Interpolation::Constant),
            (0.5, 0.7, Interpolation::Constant),
            (1.0, 0.1, Interpolation::Constant),
        ]);
        assert_eq!(r.evaluate(0.4999)[0], 0.2);
        assert_eq!(r.evaluate(0.5)[0], 0.7);
        assert_eq!(r.evaluate(0.9999)[0], 0.7);
        // Linear, monotone and B-spline keep a line a line everywhere;
        // Catmull-Rom between its inner keys (its ends are flat).
        for i in [
            Interpolation::Linear,
            Interpolation::Monotone,
            Interpolation::BSpline,
        ] {
            let r = line(i);
            for k in 0..=60 {
                let t = f64::from(k) / 60.0;
                assert!(near(r.evaluate(t)[0], t), "{i:?} at {t}");
            }
        }
        let r = line(Interpolation::CatmullRom);
        for k in 20..=40 {
            let t = f64::from(k) / 60.0;
            assert!(near(r.evaluate(t)[0], t), "Catmull-Rom at {t}");
        }
        // Flat at the first key: half way to the second it is below the line.
        assert!(r.evaluate(1.0 / 6.0)[0] < 1.0 / 6.0);
        // A peak: Catmull-Rom's tangent there is flat, so half way up it is
        // the Hermite blend's 1/2; B-spline averages the neighbours (1 4 1)/6
        // at a key and mirrors past the ends.
        let peak = |i| float(&[(0.0, 0.0, i), (0.5, 1.0, i), (1.0, 0.0, i)]);
        assert!(near(peak(Interpolation::CatmullRom).evaluate(0.25)[0], 0.5));
        assert!(near(peak(Interpolation::CatmullRom).evaluate(0.5)[0], 1.0));
        assert!(near(
            peak(Interpolation::BSpline).evaluate(0.5)[0],
            4.0 / 6.0
        ));
        assert!(near(
            peak(Interpolation::BSpline).evaluate(0.25)[0],
            11.0 / 24.0
        ));
        // The monotone cubic holds a flat run flat.
        let r = float(&[
            (0.0, 0.0, Interpolation::Monotone),
            (0.3, 0.5, Interpolation::Monotone),
            (0.7, 0.5, Interpolation::Monotone),
            (1.0, 1.0, Interpolation::Monotone),
        ]);
        assert!((30..=70).all(|k| near(r.evaluate(f64::from(k) / 100.0)[0], 0.5)));
    }

    #[test]
    fn steps_single_keys_and_strange_places() {
        // Two keys at one place: a step, the later one's value from there.
        let r = float(&[
            (0.0, 0.0, Interpolation::Linear),
            (0.5, 1.0, Interpolation::Linear),
            (0.5, 3.0, Interpolation::Linear),
            (1.0, 3.0, Interpolation::Linear),
        ]);
        assert!((r.evaluate(0.25)[0] - 0.5).abs() < 1e-12);
        assert_eq!(r.evaluate(0.5)[0], 3.0);
        // One key: its value everywhere; NaN takes the first key's.
        let one = float(&[(0.3, 0.8, Interpolation::BSpline)]);
        for t in [-1.0, 0.0, 0.3, 0.9, 2.0, f64::NAN] {
            assert_eq!(one.evaluate(t)[0], 0.8);
        }
        assert_eq!(r.evaluate(f64::NAN)[0], 0.0);
        // A float ramp's other channels stay 0; a color ramp's run apart.
        assert_eq!(r.evaluate(0.7)[1..], [0.0, 0.0]);
        let c = Ramp {
            color: true,
            keys: vec![
                Key::color(0.0, [0.0, 1.0, 2.0], Interpolation::Linear),
                Key::color(1.0, [1.0, 1.0, 0.0], Interpolation::Linear),
            ],
        };
        assert_eq!(c.evaluate(0.25), [0.25, 1.0, 1.5]);
    }
}
