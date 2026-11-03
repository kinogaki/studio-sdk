//! [`Curves`]: polylines, Bézier, B-spline, Catmull-Rom and NURBS curves
//! over shared control points — the standard value `kinogaki.curves`.
mod build;
mod display;
mod polylines;
mod validate;

pub use build::CurvesBuilder;
pub use display::CurveDisplay;

use crate::attributes::{Attributes, Domain};
use crate::column::Column;
use crate::elements::Elements;
use crate::error::GeometryError;
use crate::positions;
use std::ops::Range;
use std::sync::Arc;

/// How a value's curves pass through their control points: one basis for
/// every curve of a value.
///
/// New bases may be added. A reader that doesn't know a value's basis
/// treats the value as read-only: it passes through unchanged, displays as
/// its control polygon and renders as nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum Basis {
    /// Straight segments between consecutive points: polylines. Degree 1.
    Linear,
    /// Bézier segments of the curve's degree, sharing end points: an open
    /// curve has `k·degree + 1` control points, a periodic one
    /// `k·degree`.
    Bezier,
    /// Uniform, non-rational B-splines of the curve's degree: clamped when
    /// open (they start and end on their end points), wrapping when
    /// periodic.
    BSpline,
    /// Catmull-Rom splines through every point (an open curve's ends
    /// reflect their neighbours). Degree 3.
    CatmullRom,
    /// Rational B-splines with explicit knots and optional weights.
    Nurbs,
}

impl Basis {
    /// Every basis, in order.
    pub const ALL: [Basis; 5] = [
        Basis::Linear,
        Basis::Bezier,
        Basis::BSpline,
        Basis::CatmullRom,
        Basis::Nurbs,
    ];

    /// Its name in messages and files: `linear`, `bezier`, `bspline`,
    /// `catmull_rom`, `nurbs`. Never renamed.
    pub fn name(self) -> &'static str {
        match self {
            Basis::Linear => "linear",
            Basis::Bezier => "bezier",
            Basis::BSpline => "bspline",
            Basis::CatmullRom => "catmull_rom",
            Basis::Nurbs => "nurbs",
        }
    }

    /// The basis named `name` ([`Basis::name`]).
    pub fn from_name(name: &str) -> Option<Basis> {
        Basis::ALL.into_iter().find(|b| b.name() == name)
    }
}

/// The knot vectors of a NURBS value: curve `i`'s knots are
/// `values[offsets[i]..offsets[i + 1]]`.
#[derive(Clone, Debug, PartialEq)]
pub struct Knots {
    offsets: Column<u32>,
    values: Column<f64>,
}

impl Knots {
    /// Where each curve's knots start, and the knot count last.
    pub fn offsets(&self) -> &Column<u32> {
        &self.offsets
    }

    /// Every curve's knots, one after another.
    pub fn values(&self) -> &Column<f64> {
        &self.values
    }

    /// Curve `i`'s knots.
    pub(crate) fn of(&self, i: usize) -> &[f64] {
        &self.values[self.offsets[i] as usize..self.offsets[i + 1] as usize]
    }
}

/// Curves of one [`Basis`] over shared control points, with attributes.
///
/// Curve `i`'s control vertices are `offsets[i]..offsets[i + 1]`. Each
/// vertex names a point through `points` when the value has a point index
/// (curves sharing points: chains of edges, the polylines of a
/// [`Polygons`](crate::Polygons) value, [`Curves::from_polylines`]), else
/// vertex `v` is point `v`.
///
/// **Domains** ([`count`](Self::count)): Point, Vertex, Primitive (each
/// curve) and Detail. The standard `width` is a Point or Primitive f32.
///
/// **Rules** ([`CurvesBuilder::build`] checks them):
/// - positions finite and at most 1e100 in each component;
/// - offsets start at 0, never decrease and end at the vertex count; every
///   point index names a position;
/// - each curve's degree is 1 to [`MAX_DEGREE`](crate::limits::MAX_DEGREE):
///   1 for Linear, 3 for Catmull-Rom;
/// - each curve has at least 2 control vertices, and `degree + 1` for
///   B-spline and NURBS; an open Bézier curve has `k·degree + 1`, a
///   periodic one `k·degree` (k ≥ 1);
/// - knots exactly for NURBS: `count + degree + 1` a curve, finite, never
///   decreasing, spanning an interval; weights only for NURBS, one a
///   vertex, finite and above 0;
/// - `periodic` covers the curves; attributes are sized to their domain.
///
/// ```
/// use kinogaki_geometry::{Basis, Curves};
///
/// let points = vec![[0.0, 0.0, 0.0], [1.0, 1.0, 0.0], [2.0, 0.0, 0.0], [3.0, 1.0, 0.0]];
/// let curve = Curves::builder(Basis::Bezier, points.into(), vec![0, 4].into())
///     .degree(3)
///     .build()
///     .unwrap();
/// assert_eq!((curve.curves(), curve.degree(0)), (1, 3));
/// // A cubic Bézier needs 3k + 1 points.
/// let short = Curves::builder(Basis::Bezier, vec![[0.0; 3]; 3].into(), vec![0, 3].into())
///     .degree(3)
///     .build();
/// assert!(short.is_err());
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct Curves {
    positions: Column<[f64; 3]>,
    offsets: Column<u32>,
    points: Option<Column<u32>>,
    basis: Basis,
    degrees: Column<u8>,
    periodic: Elements,
    knots: Option<Knots>,
    weights: Option<Column<f64>>,
    attributes: Arc<Attributes>,
}

impl Curves {
    /// A builder for curves of `basis` with control points `positions`,
    /// curve `i` taking vertices `offsets[i]..offsets[i + 1]`.
    pub fn builder(
        basis: Basis,
        positions: Column<[f64; 3]>,
        offsets: Column<u32>,
    ) -> CurvesBuilder {
        CurvesBuilder::new(basis, positions, offsets)
    }

    /// No curves.
    pub fn empty() -> Self {
        Self::builder(Basis::Linear, Column::default(), Column::new(vec![0]))
            .build()
            .expect("no curves are valid")
    }

    /// The control points' positions.
    pub fn positions(&self) -> &Column<[f64; 3]> {
        &self.positions
    }

    /// Where each curve's vertices start, and the vertex count last.
    pub fn offsets(&self) -> &Column<u32> {
        &self.offsets
    }

    /// The point each vertex names, when the value has a point index;
    /// `None`: vertex `v` is point `v`.
    pub fn points(&self) -> Option<&Column<u32>> {
        self.points.as_ref()
    }

    /// The basis.
    pub fn basis(&self) -> Basis {
        self.basis
    }

    /// Each curve's degree.
    pub fn degrees(&self) -> &Column<u8> {
        &self.degrees
    }

    /// Curve `i`'s degree.
    pub fn degree(&self, i: usize) -> u8 {
        self.degrees[i]
    }

    /// The closed curves.
    pub fn periodic(&self) -> &Elements {
        &self.periodic
    }

    /// The knots (NURBS only).
    pub fn knots(&self) -> Option<&Knots> {
        self.knots.as_ref()
    }

    /// Each vertex's weight (NURBS only; `None`: non-rational).
    pub fn weights(&self) -> Option<&Column<f64>> {
        self.weights.as_ref()
    }

    /// The attributes and groups.
    pub fn attributes(&self) -> &Arc<Attributes> {
        &self.attributes
    }

    /// How many curves.
    pub fn curves(&self) -> usize {
        self.offsets.len() - 1
    }

    /// Curve `i`'s vertex ids.
    pub fn vertices(&self, i: usize) -> Range<usize> {
        self.offsets[i] as usize..self.offsets[i + 1] as usize
    }

    /// The point vertex `v` names.
    #[inline]
    pub fn point_of(&self, v: usize) -> u32 {
        match &self.points {
            Some(points) => points[v],
            None => v as u32,
        }
    }

    /// How many elements `domain` has: points, vertices, curves (for
    /// Primitive), 1 for Detail; `None` for a domain it doesn't have.
    pub fn count(&self, domain: Domain) -> Option<usize> {
        count(
            self.positions.len(),
            *self.offsets.last().expect("offsets"),
            self.curves(),
            domain,
        )
    }

    /// The smallest box holding every control point; `None` without
    /// points. Cached on the positions column.
    pub fn bounds(&self) -> Option<crate::Bounds> {
        positions::cached_bounds(&self.positions)
    }

    /// This value with its control points at `positions`, sharing the
    /// rest. Only the positions are checked.
    ///
    /// # Errors
    /// A bad position, or a different number of them.
    pub fn with_positions(&self, positions: Column<[f64; 3]>) -> Result<Self, GeometryError> {
        if positions.len() != self.positions.len() {
            return Err(GeometryError::whole(
                crate::ErrorKind::Position,
                format!(
                    "{} positions for curves over {} points",
                    positions.len(),
                    self.positions.len()
                ),
            ));
        }
        positions::check(&positions)?;
        Ok(Self {
            positions,
            ..self.clone()
        })
    }

    /// This value with `attributes`, sharing the rest. Only the entries
    /// this value doesn't already hold are checked.
    ///
    /// # Errors
    /// A missized or misplaced attribute.
    pub fn with_attributes(&self, attributes: Attributes) -> Result<Self, GeometryError> {
        let attributes = Arc::new(attributes);
        check_attributes(&attributes, Some(&self.attributes), |d| self.count(d))?;
        Ok(Self {
            attributes,
            ..self.clone()
        })
    }
}

impl Default for Curves {
    fn default() -> Self {
        Self::empty()
    }
}

fn count(points: usize, vertices: u32, curves: usize, domain: Domain) -> Option<usize> {
    Some(match domain {
        Domain::Point => points,
        Domain::Vertex => vertices as usize,
        Domain::Primitive => curves,
        Domain::Detail => 1,
        _ => return None,
    })
}

fn check_attributes(
    attributes: &Attributes,
    previous: Option<&Attributes>,
    count: impl FnMut(Domain) -> Option<usize>,
) -> Result<(), GeometryError> {
    attributes.check("curves", previous, count)?;
    Ok(())
}
