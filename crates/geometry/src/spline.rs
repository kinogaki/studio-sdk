//! The one B-spline evaluator every value shares: span search, de Boor's
//! algorithm with the first derivative, and rational (homogeneous)
//! points. [`Curves`](crate::Curves) and the B-rep's NURBS
//! ([`crate::brep`]) both evaluate through it.
//!
//! Control values are homogeneous `[x·w, y·w, z·w, w]` (a chart point
//! leaves `z` zero; a polynomial spline has `w = 1`).

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

/// De Boor's recursion over the `degree + 1` homogeneous control values of
/// span `k`, at `t`: the point and the first derivative (from the last two
/// values one level short, NURBS Book eq. 3.4; zero on a span of no
/// length). Outside the span it continues the span's polynomial.
pub(crate) fn de_boor(
    knots: &[f64],
    mut points: Vec<Homogeneous>,
    degree: usize,
    t: f64,
    k: usize,
) -> [Homogeneous; 2] {
    let d = degree;
    let mut derivative = [0.0; 4];
    for r in 1..=d {
        if r == d {
            let gap = knots[k + 1] - knots[k];
            if gap > 0.0 {
                let (a, b) = (points[d - 1], points[d]);
                derivative = std::array::from_fn(|c| (b[c] - a[c]) * d as f64 / gap);
            }
        }
        for j in (r..=d).rev() {
            let (lo, hi) = (knots[j + k - d], knots[j + 1 + k - r]);
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
    let k = span(knots, degree, count, t);
    let points = (k - degree..=k).map(value).collect();
    de_boor(knots, points, degree, t, k)
}
