//! [`Curve`]: one primitive of a [`Geometry`] read as the curve it is —
//! its parameter domain, its spans and the one exact evaluation every
//! display, engine and kernel shares.
use crate::geometry::Geometry;
use crate::limits::MAX_DEGREE;
use crate::primitives::{Basis, Kind};
use crate::spline::{self, homogeneous, rational};
use crate::vec3::Vec3;

/// A primitive of a value as a curve through (or near) its points
/// ([`Geometry::curve`]).
///
/// **Parameters.** NURBS curves run over their knots, `knots[degree]` to
/// `knots[count]`; every other basis one unit a span, `0` to the span
/// count.
///
/// **Spans** ([`spans`](Self::spans)). Linear: each pair of consecutive
/// points (and the last to the first when closed). Bézier: each run of
/// `degree + 1` points sharing end points. B-spline: open curves are
/// clamped (they start and end on their end points), `count − degree`
/// spans; closed ones are uniform, `count` spans wrapping around.
/// Catmull-Rom: through every point, `count − 1` spans (an open curve's
/// ends reflect their neighbours: `2·p₀ − p₁`), `count` when closed. NURBS:
/// each knot interval of non-zero length in the domain; closing a NURBS
/// curve is its knots' and points' business.
///
/// ```
/// use kinogaki_geometry::{Attributes, Basis, Column, CurveData, Geometry, Kind, Knots, Primitives};
///
/// // One open cubic Bézier.
/// let positions = vec![[0.0, 0.0, 0.0], [1.0, 2.0, 0.0], [3.0, 2.0, 0.0], [4.0, 0.0, 0.0]];
/// let curves = CurveData::new(vec![3].into(), Knots::new(vec![0, 0].into(), Column::default()), None);
/// let kinds = vec![Kind::curve(Basis::Bezier, false)];
/// let table = Primitives::new(4, vec![0, 4].into(), vec![0, 1, 2, 3].into(), kinds.into(), Some(curves)).unwrap();
/// let value = Geometry::new(positions.into(), table, Attributes::new()).unwrap();
/// let curve = value.curve(0);
/// assert_eq!((curve.domain(), curve.spans().count()), ([0.0, 1.0], 1));
/// assert_eq!(curve.point(0.5).to_array(), [2.0, 1.5, 0.0]);
/// ```
#[derive(Clone, Copy, Debug)]
pub struct Curve<'a> {
    positions: &'a [[f64; 3]],
    /// Its vertices' points.
    points: &'a [u32],
    /// Its vertices' weights, when the value's curves are rational.
    weights: Option<&'a [f64]>,
    /// Its knots (NURBS only).
    knots: &'a [f64],
    kind: Kind,
    degree: usize,
}

/// One span of a [`Curve`]: the parameters it runs over, and where its
/// control points start.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Span {
    /// Its first parameter.
    pub start: f64,
    /// Its last parameter.
    pub end: f64,
    /// Its first control vertex (NURBS), else its number.
    first: usize,
}

impl Geometry {
    /// Primitive `p` as a curve: a polygon or a polyline as its straight
    /// segments, a curve primitive by its basis.
    ///
    /// # Panics
    /// When `p` is not a primitive.
    pub fn curve(&self, p: usize) -> Curve<'_> {
        let table = self.primitives();
        let vertices = table.primitive_vertices(p);
        let data = table.curves();
        Curve {
            positions: self.positions().as_slice(),
            points: table.primitive(p),
            weights: (data.and_then(|c| c.weights())).map(|w| &w[vertices]),
            knots: data.map_or(&[][..], |c| c.knots().of(p)),
            kind: table.kind(p),
            degree: data.map_or(1, |c| c.degrees()[p] as usize),
        }
    }
}

impl<'a> Curve<'a> {
    /// The same curve over `positions` in place of its value's points
    /// (in the value's numbering): where a drag shows them, so a preview
    /// evaluates exactly as the moved value will.
    ///
    /// # Panics
    /// Evaluating it panics when `positions` misses one of its points.
    pub fn over<'b>(self, positions: &'b [[f64; 3]]) -> Curve<'b>
    where
        'a: 'b,
    {
        let curve: Curve<'b> = self;
        Curve { positions, ..curve }
    }
}

impl Curve<'_> {
    /// Its kind.
    pub fn kind(&self) -> Kind {
        self.kind
    }

    /// Its degree (1 for a linear primitive).
    pub fn degree(&self) -> u8 {
        self.degree as u8
    }

    /// Its parameter interval.
    pub fn domain(&self) -> [f64; 2] {
        match self.kind.basis() {
            Basis::Nurbs => [self.knots[self.degree], self.knots[self.points.len()]],
            _ => [0.0, self.span_count() as f64],
        }
    }

    /// Its spans in parameter order.
    pub fn spans(&self) -> impl Iterator<Item = Span> + '_ {
        let nurbs = self.kind.basis() == Basis::Nurbs;
        let (d, n) = (self.degree, self.points.len());
        let knot_spans = (d..n)
            .filter(move |&k| nurbs && self.knots[k + 1] > self.knots[k])
            .map(move |k| Span {
                start: self.knots[k],
                end: self.knots[k + 1],
                first: k - d,
            });
        let unit_spans = (0..if nurbs { 0 } else { self.span_count() }).map(|s| Span {
            start: s as f64,
            end: s as f64 + 1.0,
            first: s,
        });
        knot_spans.chain(unit_spans)
    }

    /// Its point at parameter `t`, clamped to its [`domain`](Self::domain).
    pub fn point(&self, t: f64) -> Vec3 {
        let [lo, hi] = self.domain();
        let t = t.clamp(lo, hi);
        if self.kind.basis() == Basis::Nurbs {
            let (d, n) = (self.degree, self.points.len());
            let first = spline::span(self.knots, d, n, t) - d;
            return self.nurbs_at(first, t).into();
        }
        let s = (t.floor() as usize).min(self.span_count().saturating_sub(1));
        self.unit_at(s, t - s as f64).into()
    }

    /// Its point at parameter `t` of `span` (one of [`spans`](Self::spans)),
    /// read from that span's control points alone: what a display samples
    /// a span with, in time linear in the curve's points.
    pub fn on(&self, span: Span, t: f64) -> Vec3 {
        match self.kind.basis() {
            Basis::Nurbs => self.nurbs_at(span.first, t).into(),
            _ => self.unit_at(span.first, t - span.start).into(),
        }
    }

    /// How many spans a curve of a basis other than NURBS has.
    fn span_count(&self) -> usize {
        let (n, d, closed) = (self.points.len(), self.degree, self.kind.closed());
        match (self.kind.basis(), closed) {
            (Basis::Linear | Basis::CatmullRom | Basis::BSpline, true) => n,
            (Basis::Linear | Basis::CatmullRom, false) => n - 1,
            (Basis::Bezier, true) => n / d,
            (Basis::Bezier, false) => (n - 1) / d,
            (Basis::BSpline, false) => n - d,
            (Basis::Nurbs, _) => self.spans().count(),
        }
    }

    /// Control point `k` (wrapping).
    fn cv(&self, k: usize) -> [f64; 3] {
        self.positions[self.points[k % self.points.len()] as usize]
    }

    /// The point of unit span `s` (not NURBS) at `u`, `0` to `1` over it.
    fn unit_at(&self, s: usize, u: f64) -> [f64; 3] {
        let (n, d, closed) = (self.points.len(), self.degree, self.kind.closed());
        match self.kind.basis() {
            Basis::Linear => lerp(self.cv(s), self.cv(s + 1), u),
            Basis::Bezier => {
                let mut points = [[0.0; 3]; MAX_DEGREE as usize + 1];
                for (k, p) in points[..=d].iter_mut().enumerate() {
                    *p = self.cv(s * d + k);
                }
                for level in 1..=d {
                    for k in 0..=d - level {
                        points[k] = lerp(points[k], points[k + 1], u);
                    }
                }
                points[0]
            }
            Basis::CatmullRom => {
                let at = |k: isize| -> [f64; 3] {
                    if closed {
                        self.cv(k.rem_euclid(n as isize) as usize)
                    } else if k < 0 {
                        reflect(self.cv(0), self.cv(1))
                    } else if k as usize >= n {
                        reflect(self.cv(n - 1), self.cv(n - 2))
                    } else {
                        self.cv(k as usize)
                    }
                };
                let s = s as isize;
                catmull_rom([at(s - 1), at(s), at(s + 1), at(s + 2)], u)
            }
            Basis::BSpline => {
                let value = |j: usize| homogeneous(self.cv(s + j), 1.0);
                if closed {
                    // Uniform knots, the points wrapping: span s uses
                    // points s..=s + d.
                    rational(spline::de_boor(d, d as f64 + u, |i| i as f64, value))[0]
                } else {
                    // Clamped uniform knots, global knot g being g − d
                    // within 0..=n − d; span s starts at knot s + d.
                    let knot = |i: usize| (s + i).saturating_sub(d).min(n - d) as f64;
                    rational(spline::de_boor(d, s as f64 + u, knot, value))[0]
                }
            }
            Basis::Nurbs => unreachable!("NURBS curves evaluate by knot"),
        }
    }

    /// The point at `t` of the NURBS span whose control points start at
    /// `first`.
    fn nurbs_at(&self, first: usize, t: f64) -> [f64; 3] {
        let value = |j: usize| {
            let v = first + j;
            let weight = self.weights.map_or(1.0, |w| w[v]);
            homogeneous(self.positions[self.points[v] as usize], weight)
        };
        let knot = |i: usize| self.knots[first + i];
        rational(spline::de_boor(self.degree, t, knot, value))[0]
    }
}

fn lerp(a: [f64; 3], b: [f64; 3], u: f64) -> [f64; 3] {
    std::array::from_fn(|c| a[c] + (b[c] - a[c]) * u)
}

/// `2a − b`: `b` reflected through `a`.
fn reflect(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|c| 2.0 * a[c] - b[c])
}

/// The uniform Catmull-Rom span from `p[1]` to `p[2]` at `u`.
fn catmull_rom(p: [[f64; 3]; 4], u: f64) -> [f64; 3] {
    let (u2, u3) = (u * u, u * u * u);
    std::array::from_fn(|c| {
        let [a, b, e, f] = [p[0][c], p[1][c], p[2][c], p[3][c]];
        0.5 * (2.0 * b
            + (e - a) * u
            + (2.0 * a - 5.0 * b + 4.0 * e - f) * u2
            + (3.0 * b - a - 3.0 * e + f) * u3)
    })
}

#[cfg(test)]
mod tests {
    use crate::spline::KNOTS_BUILT;
    use crate::{Attributes, Basis, CurveData, Geometry, Kind, Knots, Primitives};

    /// One open cubic curve of `n` points of `basis` (NURBS clamped
    /// uniform).
    fn curve(basis: Basis, n: usize) -> Geometry {
        let points: Vec<[f64; 3]> = (0..n).map(|k| [k as f64, (k % 3) as f64, 0.0]).collect();
        let knots: Vec<f64> = match basis {
            Basis::Nurbs => (0..n + 4)
                .map(|k| k.saturating_sub(3).min(n - 3) as f64)
                .collect(),
            _ => Vec::new(),
        };
        let data = CurveData::new(
            vec![3].into(),
            Knots::new(vec![0, knots.len() as u32].into(), knots.into()),
            None,
        );
        let kinds = vec![Kind::curve(basis, false)];
        let vertices: Vec<u32> = (0..n as u32).collect();
        let table = Primitives::new(
            n,
            vec![0, n as u32].into(),
            vertices.into(),
            kinds.into(),
            Some(data),
        )
        .unwrap();
        Geometry::new(points.into(), table, Attributes::new()).unwrap()
    }

    #[test]
    fn a_span_reads_only_its_own_knots() {
        for basis in [Basis::BSpline, Basis::Nurbs] {
            for n in [1000, 4000] {
                let value = curve(basis, n);
                let curve = value.curve(0);
                let before = KNOTS_BUILT.with(|built| built.get());
                let mut shown = 0;
                for span in curve.spans() {
                    for k in 0..16 {
                        let t = span.start + (span.end - span.start) * k as f64 / 16.0;
                        std::hint::black_box(curve.on(span, t));
                        shown += 1;
                    }
                }
                let knots = KNOTS_BUILT.with(|built| built.get()) - before;
                // A cubic span reads 2·3 + 1 knots a point, however long
                // the curve.
                assert!(
                    knots <= 7 * shown,
                    "{basis:?} of {n} points: {knots} knots for {shown} points"
                );
            }
        }
    }
}
