//! [`Sampled`]: a value over a frame's shutter.
use crate::{Error, Vec3, Xform};
use std::sync::Arc;

/// A value at one or more moments around a frame: `values[k]` at
/// `times[k]`, in frames from the frame (fractional: a shutter's samples),
/// ascending. One sample means it holds still: what most values are. A
/// render prim's placements across the shutter are one ([`Sampled::still`]
/// for a prim that doesn't move).
///
/// The samples are shared (`Arc`), so sending a sampled value again costs
/// a reference count, never a copy.
///
/// ```
/// use kinogaki_geometry::Sampled;
///
/// let still = Sampled::still(1.0);
/// assert!(still.is_still() && still.values() == [1.0]);
/// let moving = Sampled::new(vec![-0.25, 0.25], vec![1.0, 3.0]).unwrap();
/// assert_eq!(moving.times(), [-0.25, 0.25]);
/// assert!(Sampled::new(vec![0.5, 0.0], vec![1.0, 2.0]).is_err());
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct Sampled<T> {
    times: Arc<[f32]>,
    values: Arc<[T]>,
}

impl<T> Sampled<T> {
    /// `value` holding still: one sample, at the frame.
    pub fn still(value: T) -> Self {
        Self {
            times: Arc::from([0.0]),
            values: Arc::from([value]),
        }
    }

    /// `values` at `times` (frames from the frame).
    ///
    /// # Errors
    /// No samples, a time for each value missing, or times not finite and
    /// ascending.
    pub fn new(times: impl Into<Arc<[f32]>>, values: impl Into<Arc<[T]>>) -> Result<Self, Error> {
        let (times, values) = (times.into(), values.into());
        let ascending = times.windows(2).all(|t| t[0] < t[1]);
        let finite = times.iter().all(|t| t.is_finite());
        if times.is_empty() || times.len() != values.len() || !(ascending && finite) {
            return Err(Error::invalid(
                "samples need one finite, ascending time a value, and at least one value",
            ));
        }
        Ok(Self { times, values })
    }

    /// The sample times, in frames from the frame, ascending.
    pub fn times(&self) -> &[f32] {
        &self.times
    }

    /// The values, one a time.
    pub fn values(&self) -> &[T] {
        &self.values
    }

    /// Whether it holds one sample: it doesn't change around the frame.
    pub fn is_still(&self) -> bool {
        self.values.len() == 1
    }
}

impl Sampled<Xform> {
    /// The placement at `time`: the two samples around it blended, each split
    /// into its turn, its stretch and its origin, the turn along the shorter
    /// arc and the rest in a line, so a turning placement keeps its shape
    /// between samples. Before the first sample it is the first; past the
    /// last, the last.
    pub fn at(&self, time: f32) -> Xform {
        let (times, values) = (self.times(), self.values());
        let next = times.partition_point(|&t| t <= time);
        if next == 0 {
            return values[0];
        }
        if next == times.len() {
            return values[next - 1];
        }
        let (from, to) = (times[next - 1], times[next]);
        let u = f64::from((time - from) / (to - from));
        blend(&values[next - 1], &values[next], u)
    }
}

/// A 3×3 matrix by its columns.
type Columns = [Vec3; 3];

/// `a` and `b` blended at `u`: turns along the shorter arc, stretches and
/// origins in a line. Placements that don't split (a flat one, or one
/// mirrored where the other isn't) blend number by number.
fn blend(a: &Xform, b: &Xform, u: f64) -> Xform {
    let split = |x: &Xform| polar(x.axes());
    let (Some((ta, sa)), Some((tb, sb))) = (split(a), split(b)) else {
        return blend_numbers(a, b, u);
    };
    if ta.1 != tb.1 {
        return blend_numbers(a, b, u);
    }
    let turn = slerp(quaternion(ta.0), quaternion(tb.0), u);
    let stretch: Columns = std::array::from_fn(|k| sa[k] + (sb[k] - sa[k]) * u);
    let turned = matrix(turn).map(|axis| axis * ta.1);
    // The turn after the stretch: each column of the product.
    let axes: Columns = std::array::from_fn(|k| {
        turned[0] * stretch[k].x + turned[1] * stretch[k].y + turned[2] * stretch[k].z
    });
    let origin = a.origin() + (b.origin() - a.origin()) * u;
    Xform::from_axes(axes[0], axes[1], axes[2], origin)
}

/// `a` and `b` blended number by number.
fn blend_numbers(a: &Xform, b: &Xform, u: f64) -> Xform {
    let (a, b) = (a.rows(), b.rows());
    Xform::from_rows(std::array::from_fn(|r| {
        std::array::from_fn(|c| a[r][c] + (b[r][c] - a[r][c]) * u)
    }))
}

/// `m` as a turn and a stretch, `m = sign · turn · stretch`: the turn a
/// rotation (its columns), `sign` -1 when `m` mirrors, the stretch
/// symmetric. `None` when `m` is flat.
fn polar(m: Columns) -> Option<((Columns, f64), Columns)> {
    let det = m[0].dot(m[1].cross(m[2]));
    if !det.is_finite() || det.abs() < 1e-300 {
        return None;
    }
    let sign = det.signum();
    let [mut a, mut b, mut c] = m.map(|v| v * sign);
    // Newton's iteration toward the nearest rotation: the mean of the
    // matrix and its inverse's transpose (the cofactors over the
    // determinant).
    for _ in 0..32 {
        let (x, y, z) = (b.cross(c), c.cross(a), a.cross(b));
        let det = a.dot(x);
        if det.abs() < 1e-300 {
            return None;
        }
        let half = 0.5 / det;
        let next = (a * 0.5 + x * half, b * 0.5 + y * half, c * 0.5 + z * half);
        let moved = (next.0 - a).length() + (next.1 - b).length() + (next.2 - c).length();
        (a, b, c) = next;
        if moved < 1e-15 {
            break;
        }
    }
    let turn = [a, b, c];
    // The stretch: the turn's transpose times the (unmirrored) matrix.
    let stretch = m.map(|v| {
        let v = v * sign;
        Vec3::new(a.dot(v), b.dot(v), c.dot(v))
    });
    Some(((turn, sign), stretch))
}

/// The unit quaternion (x y z w) of rotation `m` (its columns).
fn quaternion(m: Columns) -> [f64; 4] {
    let [x, y, z] = m;
    let trace = x.x + y.y + z.z;
    let q = if trace > 0.0 {
        let s = (trace + 1.0).sqrt() * 2.0;
        [(y.z - z.y) / s, (z.x - x.z) / s, (x.y - y.x) / s, 0.25 * s]
    } else if x.x > y.y && x.x > z.z {
        let s = (1.0 + x.x - y.y - z.z).sqrt() * 2.0;
        [0.25 * s, (y.x + x.y) / s, (z.x + x.z) / s, (y.z - z.y) / s]
    } else if y.y > z.z {
        let s = (1.0 + y.y - x.x - z.z).sqrt() * 2.0;
        [(y.x + x.y) / s, 0.25 * s, (z.y + y.z) / s, (z.x - x.z) / s]
    } else {
        let s = (1.0 + z.z - x.x - y.y).sqrt() * 2.0;
        [(z.x + x.z) / s, (z.y + y.z) / s, 0.25 * s, (x.y - y.x) / s]
    };
    let length = q.iter().map(|c| c * c).sum::<f64>().sqrt();
    q.map(|c| c / length)
}

/// Rotation `q`'s columns.
fn matrix(q: [f64; 4]) -> Columns {
    let [x, y, z, w] = q;
    [
        Vec3::new(
            1.0 - 2.0 * (y * y + z * z),
            2.0 * (x * y + w * z),
            2.0 * (x * z - w * y),
        ),
        Vec3::new(
            2.0 * (x * y - w * z),
            1.0 - 2.0 * (x * x + z * z),
            2.0 * (y * z + w * x),
        ),
        Vec3::new(
            2.0 * (x * z + w * y),
            2.0 * (y * z - w * x),
            1.0 - 2.0 * (x * x + y * y),
        ),
    ]
}

/// From `a` to `b` along the shorter arc, at `u`.
fn slerp(a: [f64; 4], b: [f64; 4], u: f64) -> [f64; 4] {
    let mut dot: f64 = (0..4).map(|k| a[k] * b[k]).sum();
    let b = if dot < 0.0 {
        dot = -dot;
        b.map(|c| -c)
    } else {
        b
    };
    let (wa, wb) = if dot > 1.0 - 1e-12 {
        (1.0 - u, u)
    } else {
        let angle = dot.acos();
        let sin = angle.sin();
        (((1.0 - u) * angle).sin() / sin, (u * angle).sin() / sin)
    };
    let q: [f64; 4] = std::array::from_fn(|k| a[k] * wa + b[k] * wb);
    let length = q.iter().map(|c| c * c).sum::<f64>().sqrt();
    q.map(|c| c / length)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn samples_are_checked() {
        let s = Sampled::new(vec![-0.5, 0.0, 0.5], vec!['a', 'b', 'c']).unwrap();
        assert_eq!(s.times(), [-0.5, 0.0, 0.5]);
        assert_eq!(s.values(), ['a', 'b', 'c']);
        assert!(!s.is_still());
        assert!(Sampled::<u8>::new(vec![], vec![]).is_err());
        assert!(Sampled::new(vec![0.0], vec![1, 2]).is_err());
        assert!(Sampled::new(vec![0.0, 0.0], vec![1, 2]).is_err());
        assert!(Sampled::new(vec![f32::NAN], vec![1]).is_err());
    }

    fn turn(degrees: f64) -> Xform {
        let (s, c) = degrees.to_radians().sin_cos();
        Xform::from_axes(
            Vec3::new(c, s, 0.0),
            Vec3::new(-s, c, 0.0),
            Vec3::Z,
            Vec3::ZERO,
        )
    }

    fn near(a: Vec3, b: Vec3) -> bool {
        (a - b).length() < 1e-12
    }

    /// Between a turn's samples the placement turns along the arc, its axes
    /// keeping their length (a blend number by number would shrink them).
    #[test]
    fn a_turn_blends_along_its_arc() {
        let sampled = Sampled::new(vec![0.0, 1.0], vec![turn(0.0), turn(90.0)]).unwrap();
        let half = sampled.at(0.5);
        let [x, y, z] = half.axes();
        let h = std::f64::consts::FRAC_1_SQRT_2;
        assert!(near(x, Vec3::new(h, h, 0.0)), "{x:?}");
        assert!(near(y, Vec3::new(-h, h, 0.0)) && near(z, Vec3::Z));
        assert!(near(sampled.at(0.25).axes()[0], turn(22.5).axes()[0]));
        assert_eq!(sampled.at(-1.0), turn(0.0), "before the first: the first");
        assert_eq!(sampled.at(2.0), turn(90.0), "past the last: the last");
    }

    /// A stretch and an origin blend in a line; a mirrored placement blends
    /// with a mirrored one; a flat one blends number by number.
    #[test]
    fn stretches_origins_and_mirrors_blend() {
        let scaled = |s: f64, o: f64| {
            Xform::from_axes(
                Vec3::X * s,
                Vec3::Y * s,
                Vec3::Z * s,
                Vec3::new(o, 0.0, 0.0),
            )
        };
        let sampled =
            Sampled::new(vec![0.0, 2.0], vec![scaled(2.0, 0.0), scaled(4.0, 2.0)]).unwrap();
        let half = sampled.at(1.0);
        assert!(near(half.axes()[0], Vec3::X * 3.0) && near(half.origin(), Vec3::X));
        let mirrored = |d: f64| {
            let [x, y, z] = turn(d).axes();
            Xform::from_axes(x * -1.0, y, z, Vec3::ZERO)
        };
        let both = Sampled::new(vec![0.0, 1.0], vec![mirrored(0.0), mirrored(90.0)]).unwrap();
        let half = both.at(0.5).axes();
        assert!(half[0].dot(half[1].cross(half[2])) < 0.0, "still mirrored");
        assert!((half[0].length() - 1.0).abs() < 1e-12);
        let flat = Xform::from_axes(Vec3::X, Vec3::Y, Vec3::ZERO, Vec3::ZERO);
        let some = Sampled::new(vec![0.0, 1.0], vec![flat, scaled(1.0, 0.0)]).unwrap();
        assert!(near(some.at(0.5).axes()[2], Vec3::Z * 0.5));
    }
}
