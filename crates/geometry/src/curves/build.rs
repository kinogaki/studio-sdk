//! [`CurvesBuilder`]: the one way to make [`Curves`].
use super::{Basis, Curves, Knots, validate};
use crate::attributes::{Attributes, Group};
use crate::column::Column;
use crate::error::GeometryError;
use std::sync::Arc;

/// Collects a [`Curves`] value's parts; [`build`](Self::build) checks them
/// all ([`Curves`]' rules). Made by [`Curves::builder`].
///
/// What isn't given defaults: no point index (vertex `v` is point `v`),
/// every degree 1 for Linear and 3 otherwise, no curve periodic, no
/// weights, no attributes. NURBS curves have no default knots.
#[derive(Clone, Debug)]
#[must_use]
pub struct CurvesBuilder {
    positions: Column<[f64; 3]>,
    offsets: Column<u32>,
    points: Option<Column<u32>>,
    basis: Basis,
    degrees: Option<Column<u8>>,
    periodic: Option<Group>,
    knots: Option<(Column<u32>, Column<f64>)>,
    weights: Option<Column<f64>>,
    attributes: Arc<Attributes>,
}

impl CurvesBuilder {
    pub(super) fn new(basis: Basis, positions: Column<[f64; 3]>, offsets: Column<u32>) -> Self {
        Self {
            positions,
            offsets,
            points: None,
            basis,
            degrees: None,
            periodic: None,
            knots: None,
            weights: None,
            attributes: Arc::default(),
        }
    }

    /// Vertex `v` names point `points[v]`.
    pub fn points(mut self, points: Column<u32>) -> Self {
        self.points = Some(points);
        self
    }

    /// Each curve's degree.
    pub fn degrees(mut self, degrees: Column<u8>) -> Self {
        self.degrees = Some(degrees);
        self
    }

    /// Every curve of degree `degree`.
    pub fn degree(mut self, degree: u8) -> Self {
        let curves = self.offsets.len().saturating_sub(1);
        self.degrees = Some(Column::new(vec![degree; curves]));
        self
    }

    /// The closed curves.
    pub fn periodic(mut self, periodic: Group) -> Self {
        self.periodic = Some(periodic);
        self
    }

    /// NURBS knots: curve `i`'s are `values[offsets[i]..offsets[i + 1]]`.
    pub fn knots(mut self, offsets: Column<u32>, values: Column<f64>) -> Self {
        self.knots = Some((offsets, values));
        self
    }

    /// NURBS weights, one a vertex.
    pub fn weights(mut self, weights: Column<f64>) -> Self {
        self.weights = Some(weights);
        self
    }

    /// The attributes and groups.
    pub fn attributes(self, attributes: Attributes) -> Self {
        self.shared_attributes(Arc::new(attributes))
    }

    /// The attributes and groups, shared with another value.
    pub fn shared_attributes(mut self, attributes: Arc<Attributes>) -> Self {
        self.attributes = attributes;
        self
    }

    /// The curves, checked.
    ///
    /// # Errors
    /// The first rule broken, naming the curve (or point, or vertex).
    pub fn build(self) -> Result<Curves, GeometryError> {
        let curves = self.offsets.len().saturating_sub(1);
        let degrees = self.degrees.unwrap_or_else(|| {
            let degree = if self.basis == Basis::Linear { 1 } else { 3 };
            Column::new(vec![degree; curves])
        });
        let periodic = self.periodic.unwrap_or_else(|| Group::none(curves));
        let value = Curves {
            positions: self.positions,
            offsets: self.offsets,
            points: self.points,
            basis: self.basis,
            degrees,
            periodic,
            knots: self
                .knots
                .map(|(offsets, values)| Knots { offsets, values }),
            weights: self.weights,
            attributes: self.attributes,
        };
        validate::curves(&value)?;
        Ok(value)
    }
}
