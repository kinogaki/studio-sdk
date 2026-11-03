//! [`CurveData`]: what curve primitives hold beside their points.
use crate::column::Column;

/// Knot vectors, one per primitive: primitive `p`'s knots are
/// `values[offsets[p]..offsets[p + 1]]` (none for a primitive that isn't
/// NURBS).
#[derive(Clone, Debug, PartialEq)]
pub struct Knots {
    offsets: Column<u32>,
    values: Column<f64>,
}

impl Knots {
    /// Knots `values`, primitive `p`'s from `offsets[p]` to
    /// `offsets[p + 1]` (checked by the value holding them).
    pub fn new(offsets: Column<u32>, values: Column<f64>) -> Self {
        Self { offsets, values }
    }

    /// Where each primitive's knots start, and the knot count last.
    pub fn offsets(&self) -> &Column<u32> {
        &self.offsets
    }

    /// Every primitive's knots, one after another.
    pub fn values(&self) -> &Column<f64> {
        &self.values
    }

    /// Primitive `p`'s knots.
    pub fn of(&self, p: usize) -> &[f64] {
        &self.values[self.offsets[p] as usize..self.offsets[p + 1] as usize]
    }
}

/// The curve data of a [`Primitives`](super::Primitives) table holding a
/// curve: each primitive's degree (1 for polygons and polylines), its
/// knots (NURBS only) and, when any NURBS curve is rational, each
/// vertex's weight (1 off NURBS curves). Checked by the table.
#[derive(Clone, Debug, PartialEq)]
pub struct CurveData {
    degrees: Column<u8>,
    knots: Knots,
    weights: Option<Column<f64>>,
}

impl CurveData {
    /// `degrees`, `knots` and `weights`, as above.
    pub fn new(degrees: Column<u8>, knots: Knots, weights: Option<Column<f64>>) -> Self {
        Self {
            degrees,
            knots,
            weights,
        }
    }

    /// Each primitive's degree.
    pub fn degrees(&self) -> &Column<u8> {
        &self.degrees
    }

    /// Each primitive's knots.
    pub fn knots(&self) -> &Knots {
        &self.knots
    }

    /// Each vertex's weight, when a curve is rational.
    pub fn weights(&self) -> Option<&Column<f64>> {
        self.weights.as_ref()
    }
}
