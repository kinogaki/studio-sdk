//! Clamped NURBS curves (in space and in a chart) and tensor-product NURBS
//! surfaces, with the one evaluation the contract is stated through.
use super::geometry::refuse;
use crate::Error;
use crate::column::Column;
use crate::limits::MAX_COORDINATE;
use crate::spline::{self, KnotFault};

/// How smoothly a closed NURBS direction continues across its seam.
///
/// New kinds may be added; a reader that doesn't know one treats the
/// seam as [`Position`](SeamContinuity::Position) only.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum SeamContinuity {
    /// The first and last rows meet; the tangent planes may not (a crease
    /// along the seam).
    Position,
    /// The tangent planes agree across the seam (G1); the first
    /// derivative across it may change in length.
    Tangent,
    /// The first derivative across the seam is the same on both sides
    /// (C1): the parameterization continues as if periodic.
    Parametric,
}

/// Checks a clamped knot vector for `count` control points of `degree`
/// by the one NURBS rule ([`spline::nurbs_check`]): finite, never
/// decreasing, the end knots each exactly `degree + 1` times, interior
/// knots at most `degree` times, a domain of positive width.
fn check_knots(degree: u8, knots: &[f64], count: usize) -> Result<(), Error> {
    let Err(fault) = spline::nurbs_check(degree, knots, count, true) else {
        return Ok(());
    };
    let (phrase, detail) = match fault {
        KnotFault::Degree => (
            "degree out of range",
            "a NURBS degree must be positive and at most the degree limit",
        ),
        KnotFault::Few => (
            "too few control points",
            "a NURBS curve needs at least its degree plus one control points",
        ),
        KnotFault::Count => (
            "wrong knot count",
            "a NURBS curve has its control point count plus its degree plus one knots",
        ),
        KnotFault::Order => (
            "knots not clamped",
            "knots must be finite and never decrease",
        ),
        KnotFault::Span => ("knots not clamped", "the knots must span an interval"),
        KnotFault::Ends => (
            "knots not clamped",
            "each end knot must be repeated exactly degree plus one times",
        ),
        KnotFault::Interior => (
            "knots not clamped",
            "an interior knot may be repeated at most degree times",
        ),
    };
    Err(refuse(None, phrase, detail))
}

fn check_points<const D: usize>(points: &[[f64; D]]) -> Result<(), Error> {
    if points
        .iter()
        .any(|p| !p.iter().all(|c| c.abs() <= MAX_COORDINATE))
    {
        return Err(refuse(
            None,
            "control point not finite",
            "a control point is not finite, or is beyond the coordinate limit",
        ));
    }
    Ok(())
}

fn check_weights(weights: Option<&[f64]>, count: usize) -> Result<(), Error> {
    let Some(weights) = weights else {
        return Ok(());
    };
    if weights.len() != count {
        return Err(refuse(
            None,
            "wrong weight count",
            "a rational NURBS has one weight a control point",
        ));
    }
    if weights
        .iter()
        .any(|w| !(w.is_finite() && *w > 0.0 && *w <= MAX_COORDINATE))
    {
        return Err(refuse(
            None,
            "weight not positive",
            "NURBS weights must be finite and positive",
        ));
    }
    Ok(())
}

/// A clamped, possibly rational B-spline curve in `D` dimensions: a
/// carrier in space (`D = 3`) or a p-curve in a chart (`D = 2`).
///
/// **Rules** ([`NurbsCurve::new`]): degree 1 to
/// [`MAX_DEGREE`](crate::limits::MAX_DEGREE); `count + degree + 1` knots,
/// finite and never decreasing, each end knot exactly `degree + 1` times,
/// each interior one at most `degree` times, spanning an interval; points
/// finite within 1e100; weights, when rational, one a point, finite and
/// above 0. Its parameter runs over the domain `knots[degree]` to
/// `knots[count]`.
///
/// ```
/// use kinogaki_geometry::brep::NurbsCurve;
///
/// // A quadratic Bézier arc from (0, 0) to (2, 0).
/// let arc = NurbsCurve::new(2, vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
///     vec![[0.0, 0.0], [1.0, 1.0], [2.0, 0.0]], None).unwrap();
/// assert_eq!(arc.point(0.5), [1.0, 0.5]);
/// assert!(NurbsCurve::new(2, vec![0.0, 0.0, 1.0, 1.0, 1.0, 1.0],
///     vec![[0.0, 0.0], [1.0, 1.0], [2.0, 0.0]], None).is_err());
/// ```
///
/// `D` is 2 or 3: the evaluator works in homogeneous `[x, y, z, w]`, so a
/// curve of any other dimension doesn't build.
///
/// ```compile_fail
/// use kinogaki_geometry::brep::NurbsCurve;
///
/// let line = NurbsCurve::new(1, vec![0.0, 0.0, 1.0, 1.0], vec![[0.0; 4], [1.0; 4]], None);
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct NurbsCurve<const D: usize> {
    degree: u8,
    knots: Column<f64>,
    points: Column<[f64; D]>,
    weights: Option<Column<f64>>,
}

impl<const D: usize> NurbsCurve<D> {
    /// The dimensions the evaluator holds in its homogeneous `[x, y, z, w]`:
    /// a chart's 2 or space's 3. Asked by [`new`](Self::new), so a curve of
    /// another `D` fails to compile.
    const DIMENSION: () = assert!(D == 2 || D == 3, "a NURBS curve is of 2 or 3 dimensions");

    /// The curve of `degree` over `knots` with control `points` and, when
    /// rational, their `weights`.
    ///
    /// # Errors
    /// The first rule broken.
    pub fn new(
        degree: u8,
        knots: impl Into<Column<f64>>,
        points: impl Into<Column<[f64; D]>>,
        weights: Option<Column<f64>>,
    ) -> Result<Self, Error> {
        let () = Self::DIMENSION;
        let (knots, points) = (knots.into(), points.into());
        check_knots(degree, &knots, points.len())?;
        check_points(&points)?;
        check_weights(weights.as_deref(), points.len())?;
        Ok(Self {
            degree,
            knots,
            points,
            weights,
        })
    }

    /// The degree.
    pub fn degree(&self) -> u8 {
        self.degree
    }

    /// The knots.
    pub fn knots(&self) -> &Column<f64> {
        &self.knots
    }

    /// The control points.
    pub fn points(&self) -> &Column<[f64; D]> {
        &self.points
    }

    /// The weights, one a point; `None` when polynomial.
    pub fn weights(&self) -> Option<&Column<f64>> {
        self.weights.as_ref()
    }

    /// The parameter domain, `[knots[degree], knots[count]]`.
    pub fn domain(&self) -> [f64; 2] {
        let d = self.degree as usize;
        [self.knots[d], self.knots[self.points.len()]]
    }

    /// The point at `t`: in the domain, de Boor's algorithm on the span
    /// holding `t` (the span above at an interior knot); outside it, the
    /// polynomial of the end span continued.
    pub fn point(&self, t: f64) -> [f64; D] {
        self.jet(t)[0]
    }

    /// The point and first derivative at `t`.
    pub(crate) fn jet(&self, t: f64) -> [[f64; D]; 2] {
        let weight = |i: usize| self.weights.as_ref().map_or(1.0, |w| w[i]);
        spline::rational(spline::evaluate(
            &self.knots,
            self.degree as usize,
            self.points.len(),
            t,
            |i| spline::homogeneous(self.points[i], weight(i)),
        ))
    }
}

/// A clamped, possibly rational tensor-product B-spline surface.
///
/// Control points form a `count_u × count_v` grid, `u`-major: point
/// `(i, j)` is `points[i·count_v + j]`, so a row of constant `i` is
/// contiguous; weights, when rational, follow the same layout. Each
/// direction follows [`NurbsCurve`]'s rules for its degree and knots.
///
/// **Closed directions** (`closed`) are a declaration: in that direction
/// the first and last control rows coincide within the body's tolerance,
/// with the stated [`SeamContinuity`]. The chart is then periodic in
/// position with the domain's width as its period (as a cylinder's angle
/// is): evaluation wraps a parameter outside the domain back into it by
/// whole periods. A body's builder checks the rows coincide.
#[derive(Clone, Debug, PartialEq)]
pub struct NurbsSurface {
    degrees: [u8; 2],
    knots: [Column<f64>; 2],
    counts: [usize; 2],
    points: Column<[f64; 3]>,
    weights: Option<Column<f64>>,
    closed: [Option<SeamContinuity>; 2],
}

impl NurbsSurface {
    /// The surface of `degrees` `[u, v]` over `knots` `[u, v]` with the
    /// `u`-major control grid `points` (and `weights`, when rational),
    /// closed in the directions `closed` declares.
    ///
    /// # Errors
    /// The first rule broken, or a point count that isn't the grid's.
    pub fn new(
        degrees: [u8; 2],
        knots: [Column<f64>; 2],
        points: impl Into<Column<[f64; 3]>>,
        weights: Option<Column<f64>>,
        closed: [Option<SeamContinuity>; 2],
    ) -> Result<Self, Error> {
        let points = points.into();
        let counts = [0, 1].map(|k| (knots[k].len()).saturating_sub(degrees[k] as usize + 1));
        for k in 0..2 {
            check_knots(degrees[k], &knots[k], counts[k])?;
        }
        if counts[0].checked_mul(counts[1]) != Some(points.len()) {
            return Err(refuse(
                None,
                "wrong control point count",
                "a NURBS surface's control grid does not match its knots",
            ));
        }
        check_points(&points)?;
        check_weights(weights.as_deref(), points.len())?;
        Ok(Self {
            degrees,
            knots,
            counts,
            points,
            weights,
            closed,
        })
    }

    /// The degrees `[u, v]`.
    pub fn degrees(&self) -> [u8; 2] {
        self.degrees
    }

    /// The knots `[u, v]`.
    pub fn knots(&self) -> &[Column<f64>; 2] {
        &self.knots
    }

    /// The grid's size `[count_u, count_v]`.
    pub fn counts(&self) -> [usize; 2] {
        self.counts
    }

    /// The control points, `u`-major.
    pub fn points(&self) -> &Column<[f64; 3]> {
        &self.points
    }

    /// Control point `(i, j)`.
    pub(crate) fn point_at(&self, i: usize, j: usize) -> [f64; 3] {
        self.points[i * self.counts[1] + j]
    }

    /// The weights, `u`-major; `None` when polynomial.
    pub fn weights(&self) -> Option<&Column<f64>> {
        self.weights.as_ref()
    }

    /// The closed directions `[u, v]` and their seams' continuity.
    pub fn closed(&self) -> [Option<SeamContinuity>; 2] {
        self.closed
    }

    /// The domain `[[u0, u1], [v0, v1]]`.
    pub fn domain(&self) -> [[f64; 2]; 2] {
        [0, 1].map(|k| {
            let d = self.degrees[k] as usize;
            [self.knots[k][d], self.knots[k][self.counts[k]]]
        })
    }

    /// The width of the domain in each closed direction: the chart's
    /// period there.
    pub(crate) fn periods(&self) -> [Option<f64>; 2] {
        let domain = self.domain();
        [0, 1].map(|k| self.closed[k].map(|_| domain[k][1] - domain[k][0]))
    }

    /// `(u, v)` with each closed direction wrapped into the domain by
    /// whole periods; parameters inside it are kept as they are.
    pub(crate) fn wrap(&self, uv: [f64; 2]) -> [f64; 2] {
        let domain = self.domain();
        [0, 1].map(|k| match self.closed[k] {
            Some(_) => wrap_into(uv[k], domain[k]),
            None => uv[k],
        })
    }

    /// The point and first partials `[S, S_u, S_v]` at `uv`.
    pub(crate) fn jet(&self, uv: [f64; 2]) -> [[f64; 3]; 3] {
        let [u, v] = self.wrap(uv);
        let [du, dv] = self.degrees.map(|d| d as usize);
        let [nu, nv] = self.counts;
        // Each row of constant i evaluated along v, then those along u.
        let su = spline::span(&self.knots[0], du, nu, u);
        let rows: Vec<[[f64; 4]; 2]> = (su - du..=su)
            .map(|i| {
                spline::evaluate(&self.knots[1], dv, nv, v, |j| {
                    let w = self.weights.as_ref().map_or(1.0, |w| w[i * nv + j]);
                    spline::homogeneous(self.points[i * nv + j], w)
                })
            })
            .collect();
        let along_u = |which: usize| {
            spline::evaluate(&self.knots[0], du, nu, u, |i| {
                if (su - du..=su).contains(&i) {
                    rows[i + du - su][which]
                } else {
                    [0.0; 4]
                }
            })
        };
        let [p, pu] = along_u(0);
        let [pv, _] = along_u(1);
        let [point, s_u] = spline::rational([p, pu]);
        let [_, s_v] = spline::rational([p, pv]);
        [point, s_u, s_v]
    }
}

/// `x` reduced into `[a, b]` by whole periods `b − a`; values inside (the
/// ends included) and non-finite values are returned unchanged.
pub(crate) fn wrap_into(x: f64, [a, b]: [f64; 2]) -> f64 {
    if !(x < a || x > b) || !x.is_finite() {
        return x;
    }
    let r = (x - a).rem_euclid(b - a);
    (a + r).min(b)
}
