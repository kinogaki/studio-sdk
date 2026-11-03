//! The one B-spline evaluator every value shares: span search, de Boor's
//! algorithm with the first derivative, and rational (homogeneous)
//! points. [`Curve`](crate::Curve)s and the B-rep's NURBS
//! ([`crate::brep`]) both evaluate through it.
//!
//! Control values are homogeneous `[x·w, y·w, z·w, w]` (a chart point
//! leaves `z` zero; a polynomial spline has `w = 1`).
use crate::limits::MAX_DEGREE;

/// A homogeneous control value: a point of `D ≤ 3` dimensions and its
/// weight.
pub(crate) type Homogeneous = [f64; 4];

/// `p` of weight `w` in homogeneous form.
pub(crate) fn homogeneous<const D: usize>(p: [f64; D], w: f64) -> Homogeneous {
    let mut h = [0.0; 4];
    for (h, c) in h.iter_mut().zip(p) {
        *h = c * w;
    }
    h[3] = w;
    h
}

/// The point and first derivative of a homogeneous point and derivative
/// (the quotient rule).
pub(crate) fn rational<const D: usize>([p, d]: [Homogeneous; 2]) -> [[f64; D]; 2] {
    let point: [f64; D] = std::array::from_fn(|c| p[c] / p[3]);
    let derivative = std::array::from_fn(|c| (d[c] - point[c] * d[3]) / p[3]);
    [point, derivative]
}

/// The span `k` of `count` control values of `degree` holding `t`: the
/// last in `degree..count` with `knots[k] <= t` (the first for `t` before
/// it), stepped back over zero-length spans so the domain's end lies in
/// the last span of non-zero length.
pub(crate) fn span(knots: &[f64], degree: usize, count: usize, t: f64) -> usize {
    let mut k = degree + knots[degree + 1..count].partition_point(|&x| x <= t);
    while k > degree && knots[k] == knots[k + 1] {
        k -= 1;
    }
    k
}

/// The most control values a span has: every degree is at most
/// [`MAX_DEGREE`].
const MOST: usize = MAX_DEGREE as usize + 1;

#[cfg(test)]
thread_local! {
    /// How many knot values this thread's evaluations have built: the
    /// work a test can bound without timing it.
    pub(crate) static KNOTS_BUILT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// De Boor's recursion at `t` over one span of `degree`, on the stack: its
/// `degree + 1` homogeneous control values `value(0..=degree)` and the
/// knots around it, `knot(i)` the span's knot `i` of `0..=2·degree`, the
/// span itself running from `knot(degree)` to `knot(degree + 1)`. The point
/// and the first derivative (from the last two values one level short,
/// NURBS Book eq. 3.4; zero on a span of no length). Outside the span it
/// continues the span's polynomial.
pub(crate) fn de_boor(
    degree: usize,
    t: f64,
    knot: impl Fn(usize) -> f64,
    value: impl Fn(usize) -> Homogeneous,
) -> [Homogeneous; 2] {
    let d = degree;
    #[cfg(test)]
    KNOTS_BUILT.with(|built| built.set(built.get() + 2 * d + 1));
    let mut knots = [0.0; 2 * MOST - 1];
    for (i, k) in knots[..=2 * d].iter_mut().enumerate() {
        *k = knot(i);
    }
    let mut points = [[0.0; 4]; MOST];
    for (j, p) in points[..=d].iter_mut().enumerate() {
        *p = value(j);
    }
    let mut derivative = [0.0; 4];
    for r in 1..=d {
        if r == d {
            let gap = knots[d + 1] - knots[d];
            if gap > 0.0 {
                let (a, b) = (points[d - 1], points[d]);
                derivative = std::array::from_fn(|c| (b[c] - a[c]) * d as f64 / gap);
            }
        }
        for j in (r..=d).rev() {
            let (lo, hi) = (knots[j], knots[j + 1 + d - r]);
            let alpha = if hi > lo { (t - lo) / (hi - lo) } else { 0.0 };
            let (before, here) = (points[j - 1], points[j]);
            points[j] = std::array::from_fn(|c| (1.0 - alpha) * before[c] + alpha * here[c]);
        }
    }
    [points[d], derivative]
}

/// The homogeneous point and first derivative at `t` of the spline of
/// `degree` over `knots` whose control value `i` (of `count`) is
/// `value(i)`.
pub(crate) fn evaluate(
    knots: &[f64],
    degree: usize,
    count: usize,
    t: f64,
    value: impl Fn(usize) -> Homogeneous,
) -> [Homogeneous; 2] {
    let first = span(knots, degree, count, t) - degree;
    de_boor(degree, t, |i| knots[first + i], |j| value(first + j))
}

/// What breaks the one NURBS knot rule ([`nurbs_check`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum KnotFault {
    /// The degree is 0 or over [`MAX_DEGREE`].
    Degree,
    /// Fewer than `degree + 1` control values.
    Few,
    /// Not `count + degree + 1` knots.
    Count,
    /// A knot decreases or isn't finite.
    Order,
    /// The domain `knots[degree]..knots[count]` is empty.
    Span,
    /// Clamped only: an end knot not repeated exactly `degree + 1` times.
    Ends,
    /// Clamped only: an interior knot repeated more than `degree` times.
    Interior,
}

/// The one knot rule of every NURBS curve, a B-rep's and a geometry
/// value's alike: `knots` for `count` control values of `degree` are
/// `count + degree + 1`, finite, never decreasing, over a domain of
/// positive width; `clamped` (a B-rep's), also each end knot exactly
/// `degree + 1` times and interior knots at most `degree` times.
pub(crate) fn nurbs_check(
    degree: u8,
    knots: &[f64],
    count: usize,
    clamped: bool,
) -> Result<(), KnotFault> {
    if degree == 0 || degree > MAX_DEGREE {
        return Err(KnotFault::Degree);
    }
    let d = degree as usize;
    if count < d + 1 {
        return Err(KnotFault::Few);
    }
    if knots.len() != count + d + 1 {
        return Err(KnotFault::Count);
    }
    if knots.iter().any(|k| !k.is_finite()) || knots.windows(2).any(|w| w[1] < w[0]) {
        return Err(KnotFault::Order);
    }
    if knots[d] >= knots[count] {
        return Err(KnotFault::Span);
    }
    if !clamped {
        return Ok(());
    }
    let (n, first, last) = (knots.len(), knots[0], knots[knots.len() - 1]);
    if knots[d] != first
        || knots[n - 1 - d] != last
        || knots[d + 1] == first
        || knots[n - 2 - d] == last
    {
        return Err(KnotFault::Ends);
    }
    let mut run = 1;
    for w in knots[d + 1..n - 1 - d].windows(2) {
        run = if w[1] == w[0] { run + 1 } else { 1 };
        if run > d {
            return Err(KnotFault::Interior);
        }
    }
    Ok(())
}
