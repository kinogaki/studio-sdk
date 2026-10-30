//! Ramps: a value along 0–1 through keys, a number (a float ramp: a
//! profile, a falloff) or a color (a color ramp: a gradient), as Houdini's
//! ramp parameters have them: their maths and their encoding in a list of
//! numbers. This is the one definition: a node's ramp parameter (the
//! API's `params::ramp`, a `Value::Numbers` of `ParameterKind::Ramp`),
//! Studio's ramp widget (`sheet::ramp`) and a render engine's color ramps
//! (a `SettingValue::ColorRamp`'s keys, [`color_at`]) evaluate a ramp with
//! [`Ramp::evaluate`] or [`Keys::evaluate_channel`] and get the same
//! values.
//!
//! Keys sit at positions in 0–1, sorted. Each key's [`Interpolation`] says
//! how the ramp runs from it to the next key; before the first key and past
//! the last the ramp holds their values. A color ramp interpolates each
//! channel the same way.
//!
//! The encoding is the key count, then each key's position, value (one
//! number, or r, g, b) and interpolation code, up to a fixed capacity: the
//! keys past the count are zeros. [`encode`] writes it, [`decode`] reads
//! it back, and [`Keys`] evaluates it in place (a kernel reading a slot).

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

    /// Its code in an encoding.
    pub fn code(self) -> f64 {
        Self::ALL.iter().position(|&i| i == self).unwrap_or(1) as f64
    }

    /// The interpolation with `code`; `None` for a code no interpolation
    /// has.
    pub fn from_code(code: f64) -> Option<Interpolation> {
        (code.fract() == 0.0 && (0.0..5.0).contains(&code)).then(|| Self::ALL[code as usize])
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
        channels(self.color)
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

    /// Move key `k` to `position` (clamped to 0–1), keeping the keys sorted
    /// (it passes the keys it crosses); returns its index after.
    pub fn move_key(&mut self, k: usize, position: f64) -> usize {
        let Some(key) = self.keys.get_mut(k) else {
            return k;
        };
        key.position = position.clamp(0.0, 1.0);
        let mut k = k;
        while k > 0 && self.keys[k].position < self.keys[k - 1].position {
            self.keys.swap(k, k - 1);
            k -= 1;
        }
        while k + 1 < self.keys.len() && self.keys[k].position > self.keys[k + 1].position {
            self.keys.swap(k, k + 1);
            k += 1;
        }
        k
    }
}

impl Keys for Ramp {
    fn count(&self) -> usize {
        self.keys.len()
    }
    fn position(&self, k: usize) -> f64 {
        self.keys[k].position
    }
    fn value(&self, k: usize, channel: usize) -> f64 {
        self.keys[k].value[channel]
    }
    fn interpolation(&self, k: usize) -> Interpolation {
        self.keys[k].interpolation
    }
}

impl Keys for [Key] {
    fn count(&self) -> usize {
        self.len()
    }
    fn position(&self, k: usize) -> f64 {
        self[k].position
    }
    fn value(&self, k: usize, channel: usize) -> f64 {
        self[k].value[channel]
    }
    fn interpolation(&self, k: usize) -> Interpolation {
        self[k].interpolation
    }
}

/// Color ramp `keys` (by position) at `t`: black when there are none.
pub fn color_at(keys: &[Key], t: f64) -> [f64; 3] {
    [0, 1, 2].map(|c| keys.evaluate_channel(t, c))
}

/// The channels a key of a color (3) or float (1) ramp holds.
pub fn channels(color: bool) -> usize {
    if color { 3 } else { 1 }
}

/// The numbers one key takes in an encoding: position, value, code.
pub fn stride(color: bool) -> usize {
    channels(color) + 2
}

/// The numbers an encoding of `capacity` keys takes.
pub fn width(color: bool, capacity: usize) -> usize {
    1 + capacity * stride(color)
}

/// The most keys an encoding `len` numbers long holds.
pub fn capacity(color: bool, len: usize) -> usize {
    len.saturating_sub(1) / stride(color)
}

/// `ramp` as numbers holding `capacity` keys: the count (at most
/// `capacity`), then position, value and interpolation code a key, the
/// keys past the count zeros.
pub fn encode(ramp: &Ramp, capacity: usize) -> Vec<f64> {
    let count = ramp.keys.len().min(capacity);
    let channels = ramp.channels();
    let mut out = Vec::with_capacity(width(ramp.color, capacity));
    out.push(count as f64);
    for key in &ramp.keys[..count] {
        out.push(key.position);
        out.extend_from_slice(&key.value[..channels]);
        out.push(key.interpolation.code());
    }
    out.resize(width(ramp.color, capacity), 0.0);
    out
}

/// The ramp [`encode`] wrote. The count is clamped to what the numbers
/// hold; keys that are not finite are dropped, positions clamped to 0–1
/// and sorted, unknown interpolation codes read as Linear; a ramp left
/// without keys is [`Ramp::default_of`].
pub fn decode(numbers: &[f64], color: bool) -> Ramp {
    let stride = stride(color);
    let held = capacity(color, numbers.len());
    let count = match numbers.first() {
        Some(c) if c.is_finite() => (c.max(0.0).round() as usize).min(held),
        _ => 0,
    };
    let mut keys: Vec<Key> = numbers
        .get(1..1 + count * stride)
        .unwrap_or_default()
        .chunks_exact(stride)
        .filter(|k| k.iter().all(|v| v.is_finite()))
        .map(|k| {
            let mut value = [0.0; 3];
            value[..stride - 2].copy_from_slice(&k[1..stride - 1]);
            Key {
                position: k[0].clamp(0.0, 1.0),
                value,
                interpolation: Interpolation::from_code(k[stride - 1]).unwrap_or_default(),
            }
        })
        .collect();
    if keys.is_empty() {
        return Ramp::default_of(color);
    }
    keys.sort_by(|a, b| a.position.total_cmp(&b.position));
    Ramp { color, keys }
}

/// A ramp's keys, however they are held ([`Ramp`], or an encoding read in
/// place), and the ramp they make. Keys are sorted by position; there is
/// at least one.
pub trait Keys {
    /// How many keys.
    fn count(&self) -> usize;
    /// Key `k`'s position.
    fn position(&self, k: usize) -> f64;
    /// Key `k`'s value in `channel`.
    fn value(&self, k: usize, channel: usize) -> f64;
    /// How the ramp runs from key `k` to the next.
    fn interpolation(&self, k: usize) -> Interpolation;

    /// The ramp's `channel` at `t`: the first key's value at and before
    /// its position (and for a `t` that is NaN), the last key's at and
    /// past its position, and between two keys as the first one's
    /// interpolation runs. Keys at one position after the first make a
    /// step there: the ramp takes the last of them from that position on.
    fn evaluate_channel(&self, t: f64, channel: usize) -> f64 {
        let count = self.count();
        if count == 0 {
            return 0.0;
        }
        if t.is_nan() || t <= self.position(0) {
            return self.value(0, channel);
        }
        let last = count - 1;
        if t >= self.position(last) {
            return self.value(last, channel);
        }
        // The span [lo, lo + 1] holding t.
        let (mut lo, mut hi) = (0, last);
        while hi - lo > 1 {
            let m = lo + (hi - lo) / 2;
            if self.position(m) <= t {
                lo = m;
            } else {
                hi = m;
            }
        }
        let span = self.position(lo + 1) - self.position(lo);
        let u = (t - self.position(lo)) / span;
        let start = self.value(lo, channel);
        let end = self.value(lo + 1, channel);
        match self.interpolation(lo) {
            Interpolation::Constant => start,
            Interpolation::Linear => start + (end - start) * u,
            i @ (Interpolation::CatmullRom | Interpolation::Monotone) => {
                let slope = |p| {
                    if i == Interpolation::Monotone {
                        monotone_slope(self, p, channel)
                    } else {
                        smooth_slope(self, p, channel)
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
                let a = if before(self, lo) {
                    self.value(lo - 1, channel)
                } else {
                    2.0 * start - end
                };
                let d = if after(self, lo + 1) {
                    self.value(lo + 2, channel)
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

/// Whether key `p` has a distinct key before it.
fn before<K: Keys + ?Sized>(keys: &K, p: usize) -> bool {
    p > 0 && keys.position(p - 1) < keys.position(p)
}

/// Whether key `p` has a distinct key after it.
fn after<K: Keys + ?Sized>(keys: &K, p: usize) -> bool {
    p + 1 < keys.count() && keys.position(p + 1) > keys.position(p)
}

/// The slope of the line from key `p` to the next.
fn secant<K: Keys + ?Sized>(keys: &K, p: usize, c: usize) -> f64 {
    (keys.value(p + 1, c) - keys.value(p, c)) / (keys.position(p + 1) - keys.position(p))
}

/// Catmull-Rom's tangent at key `p`: along its neighbours, flat at an end.
fn smooth_slope<K: Keys + ?Sized>(keys: &K, p: usize, c: usize) -> f64 {
    if !before(keys, p) || !after(keys, p) {
        0.0
    } else {
        (keys.value(p + 1, c) - keys.value(p - 1, c))
            / (keys.position(p + 1) - keys.position(p - 1))
    }
}

/// The monotone cubic's tangent at key `p` (Fritsch–Butland inside, a
/// shape-preserving three-point estimate at the ends).
fn monotone_slope<K: Keys + ?Sized>(keys: &K, p: usize, c: usize) -> f64 {
    let pos = |k| keys.position(k);
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
    fn encoding_round_trips_and_tolerates_garbage() {
        let ramp = Ramp {
            color: true,
            keys: vec![
                Key::color(0.0, [0.1, 0.2, 0.3], Interpolation::Constant),
                Key::color(0.4, [1.0, 0.5, 0.0], Interpolation::BSpline),
                Key::color(1.0, [0.0, 0.0, 1.0], Interpolation::Linear),
            ],
        };
        let numbers = encode(&ramp, 4);
        assert_eq!(numbers.len(), width(true, 4));
        assert_eq!(numbers[0], 3.0);
        assert!(numbers[1 + 3 * 5..].iter().all(|&v| v == 0.0));
        assert_eq!(decode(&numbers, true), ramp);
        // Keys past the capacity are left out.
        assert_eq!(decode(&encode(&ramp, 2), true).keys.len(), 2);
        // A wild count is clamped, NaN keys dropped, keys sorted, unknown
        // codes linear.
        let mut bad = numbers.clone();
        bad[0] = 99.0;
        bad[1 + 5] = f64::NAN;
        bad[1 + 2 * 5] = -1.0;
        bad[1 + 2 * 5 + 4] = 7.5;
        let read = decode(&bad, true);
        assert_eq!(
            read.keys.len(),
            3,
            "the fourth zero key counts once clamped"
        );
        assert!(read.keys.windows(2).all(|k| k[0].position <= k[1].position));
        assert!(
            read.keys
                .iter()
                .all(|k| k.value.iter().all(|v| v.is_finite()))
        );
        assert!(
            read.keys
                .iter()
                .any(|k| k.position == 0.0 && k.interpolation == Interpolation::Linear)
        );
        // Nothing usable: the default ramp.
        assert_eq!(decode(&[f64::NAN], false), Ramp::default_of(false));
        assert_eq!(decode(&[], true), Ramp::default_of(true));
    }

    #[test]
    fn interpolations_meet_the_keys_and_hold_past_the_ends() {
        for i in Interpolation::ALL {
            assert_eq!(Interpolation::from_code(i.code()), Some(i));
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
        assert_eq!(Interpolation::from_code(2.5), None);
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
    fn keys_insert_move_and_remove_in_order() {
        let mut r = Ramp::default_of(false);
        let k = r.insert(0.5);
        assert_eq!(k, 1);
        assert!(
            (r.keys[1].value[0] - 0.5).abs() < 1e-12,
            "it takes the ramp's value"
        );
        assert_eq!(r.move_key(0, 0.7), 1, "it passes the key it crosses");
        assert_eq!(
            r.move_key(1, 1.5),
            1,
            "clamped to 1, it stays before the end key"
        );
        assert_eq!(r.keys[1].position, 1.0);
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
        // The encoding read in place evaluates as the decoded ramp.
        struct Encoded<'a>(&'a [f64]);
        impl Keys for Encoded<'_> {
            fn count(&self) -> usize {
                self.0[0] as usize
            }
            fn position(&self, k: usize) -> f64 {
                self.0[1 + k * 3]
            }
            fn value(&self, k: usize, _: usize) -> f64 {
                self.0[2 + k * 3]
            }
            fn interpolation(&self, k: usize) -> Interpolation {
                Interpolation::from_code(self.0[3 + k * 3]).unwrap()
            }
        }
        let numbers = encode(&line(Interpolation::BSpline), 6);
        let read = decode(&numbers, false);
        for k in 0..=20 {
            let t = f64::from(k) / 20.0;
            assert_eq!(
                Encoded(&numbers).evaluate_channel(t, 0),
                read.evaluate(t)[0]
            );
        }
    }
}
