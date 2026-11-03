//! How curves are evaluated and shown: [`Curves::point`], the one exact
//! evaluation of a curve, and [`Curves::display`], every curve as the
//! polyline the viewport draws.
//!
//! See [`CurveDisplay`] for the parameters, spans and display layout.
use super::{Basis, Curves, Knots};
use crate::column::Column;
use crate::kept::Kept;
use crate::limits::MAX_DEGREE;
use crate::spline::{self, homogeneous, rational};
use crate::vec3::Vec3;
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
/// 16 points, uniform in its parameter, an open curve its last
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
    pub(crate) const SAMPLES: usize = 16;

    /// The polylines' points.
    pub fn positions(&self) -> &Column<[f64; 3]> {
        &self.positions
    }

    /// Where each curve's points start, and the point count last.
    pub fn offsets(&self) -> &Column<u32> {
        &self.offsets
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
/// structure, its columns held so they compare by identity while kept.
/// Never the positions: the key lives in the positions column's own cache,
/// so holding them would keep that column alive forever.
struct Layout {
    basis: Basis,
    offsets: Column<u32>,
    points: Option<Column<u32>>,
    degrees: Column<u8>,
    periodic: crate::Elements,
    knots: Option<Knots>,
    weights: Option<Column<f64>>,
}

impl Layout {
    fn of(curves: &Curves) -> Self {
        Layout {
            basis: curves.basis,
            offsets: curves.offsets.clone(),
            points: curves.points.clone(),
            degrees: curves.degrees.clone(),
            periodic: curves.periodic.clone(),
            knots: curves.knots.clone(),
            weights: curves.weights.clone(),
        }
    }

    fn same(&self, other: &Layout) -> bool {
        fn same<T>(x: Option<&Column<T>>, y: Option<&Column<T>>) -> bool {
            match (x, y) {
                (Some(x), Some(y)) => x.ptr_eq(y),
                (None, None) => true,
                _ => false,
            }
        }
        self.basis == other.basis
            && self.offsets.ptr_eq(&other.offsets)
            && same(self.points.as_ref(), other.points.as_ref())
            && self.degrees.ptr_eq(&other.degrees)
            && self.periodic == other.periodic
            && same(
                self.knots.as_ref().map(|k| &k.offsets),
                other.knots.as_ref().map(|k| &k.offsets),
            )
            && same(
                self.knots.as_ref().map(|k| &k.values),
                other.knots.as_ref().map(|k| &k.values),
            )
            && same(self.weights.as_ref(), other.weights.as_ref())
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
    pub fn point(&self, curve: usize, t: f64) -> Vec3 {
        let [lo, hi] = self.domain(curve);
        let t = t.clamp(lo, hi);
        if self.basis == Basis::Nurbs {
            let (d, n) = (self.degree(curve) as usize, self.vertices(curve).len());
            let first = spline::span(self.knots_of(curve), d, n, t) - d;
            return self.nurbs_at(curve, first, t).into();
        }
        let span = (t.floor() as usize).min(self.spans(curve).saturating_sub(1));
        self.on_span(curve, span, t - span as f64).into()
    }

    /// The point of span `span` of curve `curve` (not NURBS) at `u`, `0`
    /// to `1` over the span: only the span's control points are read.
    fn on_span(&self, curve: usize, span: usize, u: f64) -> [f64; 3] {
        let n = self.vertices(curve).len();
        let d = self.degree(curve) as usize;
        let closed = self.periodic.contains(curve as u32);
        let cv =
            |k: usize| self.positions[self.point_of(self.offsets[curve] as usize + k % n) as usize];
        match self.basis {
            Basis::Linear => lerp(cv(span), cv(span + 1), u),
            Basis::Bezier => {
                let mut points = [[0.0; 3]; MAX_DEGREE as usize + 1];
                for (k, p) in points[..=d].iter_mut().enumerate() {
                    *p = cv(span * d + k);
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
                let value = |j: usize| homogeneous(cv(span + j), 1.0);
                if closed {
                    // Uniform knots, the vertices wrapping: span s uses
                    // vertices s..=s + d.
                    rational(spline::de_boor(d, d as f64 + u, |i| i as f64, value))[0]
                } else {
                    // Clamped uniform knots, global knot g being
                    // g − d within 0..=n − d; span s starts at knot s + d.
                    let knot = |i: usize| (span + i).saturating_sub(d).min(n - d) as f64;
                    rational(spline::de_boor(d, span as f64 + u, knot, value))[0]
                }
            }
            Basis::Nurbs => unreachable!("NURBS curves evaluate by knot"),
        }
    }

    /// The point of NURBS curve `curve` at `t` on the span whose control
    /// vertices start at `first`.
    fn nurbs_at(&self, curve: usize, first: usize, t: f64) -> [f64; 3] {
        let (knots, offset) = (self.knots_of(curve), self.offsets[curve] as usize);
        let value = |j: usize| {
            let v = offset + first + j;
            let weight = self.weights.as_ref().map_or(1.0, |w| w[v]);
            homogeneous(self.positions[self.point_of(v) as usize], weight)
        };
        rational(spline::de_boor(
            self.degree(curve) as usize,
            t,
            |i| knots[first + i],
            value,
        ))[0]
    }

    /// Every curve as the polyline the viewport draws (see
    /// [`CurveDisplay`]), derived once per positions and structure.
    pub fn display(&self) -> Arc<CurveDisplay> {
        let layout = Layout::of(self);
        let kept = self.positions.derived(|_| Displays::default());
        if let Some(display) = kept.find(|k| k.same(&layout)) {
            return display;
        }
        let built = Arc::new(self.displayed(None));
        kept.keep(layout, built.clone());
        built
    }

    /// The same as [`display`](Self::display), made from `before`'s (the
    /// same structure, already displayed) by evaluating again only the
    /// curves through the points `moved` (which must include every point
    /// whose position differs). Otherwise made whole.
    pub fn display_after(&self, before: &Curves, moved: &[u32]) -> Arc<CurveDisplay> {
        let layout = Layout::of(self);
        let kept = self.positions.derived(|_| Displays::default());
        if let Some(display) = kept.find(|k| k.same(&layout)) {
            return display;
        }
        let previous = Layout::of(before)
            .same(&layout)
            .then(|| before.positions.derived(|_| Displays::default()))
            .and_then(|k| k.find(|k| k.same(&layout)));
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
        kept.keep(layout, built.clone());
        built
    }

    /// The control points whose position differs from `before`'s bit for
    /// bit, ascending (as [`Polygons::moved_points`](crate::Polygons::moved_points)),
    /// when the two are one structure (the same basis, offsets, point
    /// index, degrees, closed curves, knots and weights columns: a
    /// [`with_positions`](Self::with_positions) edit); `None` otherwise.
    /// What a display patches, and the `moved` that
    /// [`display_after`](Self::display_after) takes.
    pub fn moved_points(&self, before: &Curves) -> Option<Vec<u32>> {
        if !Layout::of(self).same(&Layout::of(before)) {
            return None;
        }
        if self.positions.ptr_eq(&before.positions) {
            return Some(Vec::new());
        }
        Some(crate::positions::moved(&before.positions, &self.positions))
    }

    /// The display, its untouched curves' points taken from `previous`.
    fn displayed(&self, previous: Option<(&CurveDisplay, &[bool])>) -> CurveDisplay {
        let mut positions = Vec::new();
        let mut offsets = Vec::with_capacity(self.curves() + 1);
        let mut spans = Vec::new();
        let mut closed = Vec::with_capacity(self.curves());
        offsets.push(0);
        for i in 0..self.curves() {
            let periodic = self.periodic.contains(i as u32);
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

    /// Curve `i`'s display points and their spans: each span evaluated
    /// on its own control points and knots, so a curve shows in time
    /// linear in its points.
    fn sample(
        &self,
        i: usize,
        periodic: bool,
        positions: &mut Vec<[f64; 3]>,
        spans: &mut Vec<u32>,
    ) {
        let per = if self.basis == Basis::Linear {
            1
        } else {
            CurveDisplay::SAMPLES
        };
        let fraction = |k: usize| k as f64 / per as f64;
        if self.basis == Basis::Nurbs {
            let (knots, d) = (self.knots_of(i), self.degree(i) as usize);
            let mut last = None;
            for (s, k) in self.nurbs_spans(i).enumerate() {
                let (lo, hi) = (knots[k], knots[k + 1]);
                for j in 0..per {
                    positions.push(self.nurbs_at(i, k - d, lo + (hi - lo) * fraction(j)));
                    spans.push(s as u32);
                }
                last = Some((s, k, hi));
            }
            if let Some((s, k, end)) = last.filter(|_| !periodic) {
                positions.push(self.nurbs_at(i, k - d, end));
                spans.push(s as u32);
            }
            return;
        }
        let count = self.spans(i);
        for s in 0..count {
            for k in 0..per {
                positions.push(self.on_span(i, s, fraction(k)));
                spans.push(s as u32);
            }
        }
        if !periodic && count > 0 {
            positions.push(self.on_span(i, count - 1, 1.0));
            spans.push(count as u32 - 1);
        }
    }

    /// Curve `i`'s span count.
    fn spans(&self, i: usize) -> usize {
        let n = self.vertices(i).len();
        let d = self.degree(i) as usize;
        let closed = self.periodic.contains(i as u32);
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
            Basis::Nurbs => self.nurbs_spans(i).count(),
        }
    }

    /// NURBS curve `i`'s spans, each as the knot `k` it starts at: the
    /// intervals `knots[k]..knots[k + 1]` of non-zero length in its domain.
    fn nurbs_spans(&self, i: usize) -> impl Iterator<Item = usize> + '_ {
        let (knots, d, n) = (
            self.knots_of(i),
            self.degree(i) as usize,
            self.vertices(i).len(),
        );
        (d..n).filter(move |&k| knots[k + 1] > knots[k])
    }

    fn knots_of(&self, i: usize) -> &[f64] {
        self.knots.as_ref().expect("NURBS curves have knots").of(i)
    }
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

#[cfg(test)]
mod tests {
    use super::{Basis, Curves};
    use crate::spline::KNOTS_BUILT;

    /// One open cubic curve of `n` points of `basis` (NURBS clamped
    /// uniform).
    fn curve(basis: Basis, n: usize) -> Curves {
        let points: Vec<[f64; 3]> = (0..n).map(|k| [k as f64, (k % 3) as f64, 0.0]).collect();
        let mut builder = Curves::builder(basis, points.into(), vec![0, n as u32].into()).degree(3);
        if basis == Basis::Nurbs {
            let knots: Vec<f64> = (0..n + 4)
                .map(|k| k.saturating_sub(3).min(n - 3) as f64)
                .collect();
            builder = builder.knots(vec![0, n as u32 + 4].into(), knots.into());
        }
        builder.build().unwrap()
    }

    /// The knot values showing `curves` builds, and the points it shows.
    fn work(curves: &Curves) -> (usize, usize) {
        let before = KNOTS_BUILT.with(|built| built.get());
        let shown = curves.displayed(None).positions.len();
        (KNOTS_BUILT.with(|built| built.get()) - before, shown)
    }

    #[test]
    fn a_shown_point_builds_only_its_spans_knots() {
        for basis in [Basis::BSpline, Basis::Nurbs] {
            for n in [1000, 4000] {
                let (knots, points) = work(&curve(basis, n));
                // A cubic span reads 2·3 + 1 knots a point, however long
                // the curve.
                assert!(
                    knots <= 7 * points,
                    "{basis:?} of {n} points: {knots} knots for {points} shown points"
                );
            }
        }
    }
}
