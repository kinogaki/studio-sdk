//! How curves are evaluated and shown: [`Curves::point`], the one exact
//! evaluation of a curve, and [`Curves::display`], every curve as the
//! polyline the viewport draws.
//!
//! See [`CurveDisplay`] for the parameters, spans and display layout.
use super::{Basis, Curves};
use crate::column::Column;
use crate::kept::Kept;
use crate::spline::{self, homogeneous, rational};
use std::sync::Arc;

/// Every curve of a value as a polyline: what the viewport draws.
///
/// **Parameters.** A curve's parameter runs over [`Curves::domain`]: for
/// Linear, Bézier, B-spline and Catmull-Rom curves, one unit a span (`0` to
/// the span count); for NURBS, its knots from `knots[degree]` to
/// `knots[count]`.
///
/// **Spans.** Linear: each pair of consecutive points. Bézier: each run of
/// `degree + 1` points sharing end points. B-spline: open curves are
/// clamped (they start and end on their end points), `count − degree`
/// spans; periodic ones are uniform, `count` spans wrapping around.
/// Catmull-Rom: through every point, `count − 1` spans (an open curve's
/// ends reflect their neighbours: `2·p₀ − p₁`), `count` when periodic.
/// NURBS: each knot interval of non-zero length in the domain; closing a
/// NURBS curve is the knots' and vertices' business.
///
/// **Display.** Linear curves show their points. Every other span shows
/// [`CurveDisplay::SAMPLES`] points, uniform in its parameter, an open curve its last
/// point besides; periodic curves close. The layout depends on the
/// curves' structure alone, never on positions, so a display's point
/// numbering holds through a drag. Final renders take their curve detail
/// from the render's [`Detail`](crate::Detail) (chord and angle), not
/// from this display.
#[derive(Clone, Debug, PartialEq)]
pub struct CurveDisplay {
    positions: Column<[f64; 3]>,
    offsets: Column<u32>,
    spans: Column<u32>,
    closed: Vec<bool>,
}

impl CurveDisplay {
    /// Points a span of a non-linear curve shows.
    pub const SAMPLES: usize = 16;

    /// The polylines' points.
    pub fn positions(&self) -> &Column<[f64; 3]> {
        &self.positions
    }

    /// Where each curve's points start, and the point count last.
    pub fn offsets(&self) -> &Column<u32> {
        &self.offsets
    }

    /// The span of its curve each point lies on.
    pub fn spans(&self) -> &Column<u32> {
        &self.spans
    }

    /// Whether curve `i`'s polyline closes (its last point joins its
    /// first).
    pub fn closed(&self, i: usize) -> bool {
        self.closed[i]
    }

    /// Curve `i`'s segments, as pairs of point ids.
    pub fn segments(&self, i: usize) -> impl Iterator<Item = [u32; 2]> + '_ {
        let (start, end) = (self.offsets[i], self.offsets[i + 1]);
        let open = (start..end.saturating_sub(1)).map(|p| [p, p + 1]);
        let close = (self.closed[i] && end > start + 1).then_some([end - 1, start]);
        open.chain(close)
    }
}

/// What a display is derived from besides the positions: the curves'
/// structure, held so it compares by identity while kept.
struct Layout(Curves);

impl Layout {
    fn same(&self, other: &Curves) -> bool {
        let a = &self.0;
        let same_column = |x: Option<&Column<u32>>, y: Option<&Column<u32>>| match (x, y) {
            (Some(x), Some(y)) => x.ptr_eq(y),
            (None, None) => true,
            _ => false,
        };
        a.basis == other.basis
            && a.offsets.ptr_eq(&other.offsets)
            && same_column(a.points.as_ref(), other.points.as_ref())
            && a.degrees.ptr_eq(&other.degrees)
            && a.periodic.words().ptr_eq(other.periodic.words())
            && match (&a.knots, &other.knots) {
                (Some(x), Some(y)) => x.offsets.ptr_eq(&y.offsets) && x.values.ptr_eq(&y.values),
                (None, None) => true,
                _ => false,
            }
            && match (&a.weights, &other.weights) {
                (Some(x), Some(y)) => x.ptr_eq(y),
                (None, None) => true,
                _ => false,
            }
    }
}

type Displays = Kept<Layout, CurveDisplay>;

impl Curves {
    /// Curve `curve`'s parameter interval (see [`CurveDisplay`]).
    pub fn domain(&self, curve: usize) -> [f64; 2] {
        match self.basis {
            Basis::Nurbs => {
                let (knots, d, n) = (
                    self.knots_of(curve),
                    self.degree(curve) as usize,
                    self.vertices(curve).len(),
                );
                [knots[d], knots[n]]
            }
            _ => [0.0, self.spans(curve) as f64],
        }
    }

    /// The point of curve `curve` at parameter `t` (clamped to its
    /// [`domain`](Self::domain)): the one exact evaluation every display,
    /// engine and plugin shares.
    pub fn point(&self, curve: usize, t: f64) -> [f64; 3] {
        let [lo, hi] = self.domain(curve);
        let t = t.clamp(lo, hi);
        let n = self.vertices(curve).len();
        let d = self.degree(curve) as usize;
        let closed = self.periodic.contains(curve);
        let cv =
            |k: usize| self.positions[self.point_of(self.offsets[curve] as usize + k % n) as usize];
        let spans = self.spans(curve);
        let (span, u) = if self.basis == Basis::Nurbs {
            (0, t)
        } else {
            let span = (t.floor() as usize).min(spans.saturating_sub(1));
            (span, t - span as f64)
        };
        match self.basis {
            Basis::Linear => lerp(cv(span), cv(span + 1), u),
            Basis::Bezier => {
                let mut points: Vec<[f64; 3]> = (0..=d).map(|k| cv(span * d + k)).collect();
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
                        cv(k.rem_euclid(n as isize) as usize)
                    } else if k < 0 {
                        sub2(cv(0), cv(1))
                    } else if k as usize >= n {
                        sub2(cv(n - 1), cv(n - 2))
                    } else {
                        cv(k as usize)
                    }
                };
                let s = span as isize;
                catmull_rom([at(s - 1), at(s), at(s + 1), at(s + 2)], u)
            }
            Basis::BSpline => {
                if closed {
                    // Uniform knots, the vertices wrapping: span s uses
                    // vertices s..=s + d.
                    let knots: Vec<f64> = (0..=2 * d + 1).map(|k| k as f64).collect();
                    let points: Vec<[f64; 4]> =
                        (0..=d).map(|k| homogeneous(cv(span + k), 1.0)).collect();
                    rational(spline::de_boor(&knots, points, d, d as f64 + u, d))[0]
                } else {
                    let knots = clamped(n, d);
                    rational(spline::evaluate(&knots, d, n, t, |k| {
                        homogeneous(cv(k), 1.0)
                    }))[0]
                }
            }
            Basis::Nurbs => {
                let knots = self.knots_of(curve);
                let weight = |k: usize| {
                    self.weights
                        .as_ref()
                        .map_or(1.0, |w| w[self.offsets[curve] as usize + k])
                };
                rational(spline::evaluate(knots, d, n, t, |k| {
                    homogeneous(cv(k), weight(k))
                }))[0]
            }
        }
    }

    /// Every curve as the polyline the viewport draws (see
    /// [`CurveDisplay`]), derived once per positions and structure.
    pub fn display(&self) -> Arc<CurveDisplay> {
        let kept = self.positions.derived(|_| Displays::default());
        if let Some(display) = kept.find(|k| k.same(self)) {
            return display;
        }
        let built = Arc::new(self.displayed(None));
        kept.keep(Layout(self.clone()), built.clone());
        built
    }

    /// The same as [`display`](Self::display), made from `before`'s (the
    /// same structure, already displayed) by evaluating again only the
    /// curves through the points `moved` (which must include every point
    /// whose position differs). Otherwise made whole.
    pub fn display_after(&self, before: &Curves, moved: &[u32]) -> Arc<CurveDisplay> {
        let kept = self.positions.derived(|_| Displays::default());
        if let Some(display) = kept.find(|k| k.same(self)) {
            return display;
        }
        let previous = Layout(before.clone())
            .same(self)
            .then(|| before.positions.derived(|_| Displays::default()))
            .and_then(|k| k.find(|k| k.same(self)));
        let built = Arc::new(match previous {
            Some(previous) => {
                let moved: std::collections::BTreeSet<u32> = moved.iter().copied().collect();
                let touched: Vec<bool> = (0..self.curves())
                    .map(|i| self.vertices(i).any(|v| moved.contains(&self.point_of(v))))
                    .collect();
                self.displayed(Some((&previous, &touched)))
            }
            None => self.displayed(None),
        });
        kept.keep(Layout(self.clone()), built.clone());
        built
    }

    /// The display, its untouched curves' points taken from `previous`.
    fn displayed(&self, previous: Option<(&CurveDisplay, &[bool])>) -> CurveDisplay {
        let mut positions = Vec::new();
        let mut offsets = Vec::with_capacity(self.curves() + 1);
        let mut spans = Vec::new();
        let mut closed = Vec::with_capacity(self.curves());
        offsets.push(0);
        for i in 0..self.curves() {
            let periodic = self.periodic.contains(i);
            match previous {
                Some((previous, touched)) if !touched[i] => {
                    let range = previous.offsets[i] as usize..previous.offsets[i + 1] as usize;
                    positions.extend_from_slice(&previous.positions.as_slice()[range.clone()]);
                    spans.extend_from_slice(&previous.spans.as_slice()[range]);
                }
                _ => self.sample(i, periodic, &mut positions, &mut spans),
            }
            offsets.push(positions.len() as u32);
            closed.push(periodic);
        }
        CurveDisplay {
            positions: positions.into(),
            offsets: offsets.into(),
            spans: spans.into(),
            closed,
        }
    }

    /// Curve `i`'s display points and their spans.
    fn sample(
        &self,
        i: usize,
        periodic: bool,
        positions: &mut Vec<[f64; 3]>,
        spans: &mut Vec<u32>,
    ) {
        let count = self.spans(i);
        let starts = self.span_starts(i);
        let per = if self.basis == Basis::Linear {
            1
        } else {
            CurveDisplay::SAMPLES
        };
        for (s, pair) in starts.windows(2).enumerate() {
            for k in 0..per {
                let t = pair[0] + (pair[1] - pair[0]) * k as f64 / per as f64;
                positions.push(self.point(i, t));
                spans.push(s as u32);
            }
        }
        if !periodic && count > 0 {
            positions.push(self.point(i, *starts.last().expect("spans")));
            spans.push(count as u32 - 1);
        }
    }

    /// Curve `i`'s span count.
    fn spans(&self, i: usize) -> usize {
        let n = self.vertices(i).len();
        let d = self.degree(i) as usize;
        let closed = self.periodic.contains(i);
        match self.basis {
            Basis::Linear | Basis::CatmullRom => {
                if closed {
                    n
                } else {
                    n - 1
                }
            }
            Basis::Bezier => {
                if closed {
                    n / d
                } else {
                    (n - 1) / d
                }
            }
            Basis::BSpline => {
                if closed {
                    n
                } else {
                    n - d
                }
            }
            Basis::Nurbs => self.span_starts(i).len() - 1,
        }
    }

    /// The parameters where curve `i`'s spans start, and its end.
    fn span_starts(&self, i: usize) -> Vec<f64> {
        match self.basis {
            Basis::Nurbs => {
                let (knots, d, n) = (
                    self.knots_of(i),
                    self.degree(i) as usize,
                    self.vertices(i).len(),
                );
                let mut starts: Vec<f64> = vec![knots[d]];
                for &k in &knots[d + 1..=n] {
                    if k > *starts.last().expect("a start") {
                        starts.push(k);
                    }
                }
                starts
            }
            _ => (0..=self.spans(i)).map(|s| s as f64).collect(),
        }
    }

    fn knots_of(&self, i: usize) -> &[f64] {
        self.knots.as_ref().expect("NURBS curves have knots").of(i)
    }
}

/// An open, clamped, uniform knot vector for `n` vertices of degree `d`:
/// the parameter runs from 0 to `n − d`.
fn clamped(n: usize, d: usize) -> Vec<f64> {
    let interior = n - d;
    (0..n + d + 1)
        .map(|k| k.saturating_sub(d).min(interior) as f64)
        .collect()
}

fn lerp(a: [f64; 3], b: [f64; 3], u: f64) -> [f64; 3] {
    std::array::from_fn(|c| a[c] + (b[c] - a[c]) * u)
}

/// `2a − b`: `b` reflected through `a`.
fn sub2(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
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
