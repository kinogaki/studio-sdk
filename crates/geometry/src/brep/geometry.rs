//! The carriers: [`Surface`]s and [`Curve`]s in space, [`PCurve`]s in a
//! face's chart, and the [`Geometry`] tables a body keeps them in.
//!
//! Every kind is a validated value made by its constructor; the enums
//! only choose among them. Lengths are in model units, angles in
//! radians. The parameterizations are the contract (see [`super`]).
use super::{CurveId, Entity, PCurveId, SurfaceId, VertexId};
use crate::Error;
use crate::column::Column;
use crate::derived::Derived;
use crate::frame::{finite, is_unit, length_squared};
use crate::limits::MAX_COORDINATE;
use crate::vec3::Vec3;
use std::fmt;
use std::sync::Arc;

mod curves;
mod pcurves;
mod surfaces;

pub use curves::{Circle, Curve, Ellipse, Line};
pub use pcurves::{Circle2, Ellipse2, Line2, PCurve};
pub use surfaces::{Cone, Cylinder, Extrusion, Offset, Plane, Revolution, Sphere, Surface, Torus};

pub(crate) fn refuse(entity: Option<Entity>, phrase: &'static str, detail: &str) -> Error {
    crate::brep::refused(entity, phrase, detail.into())
}

fn positive(value: f64, what: &'static str) -> Result<f64, Error> {
    if value.is_finite() && value > 0.0 && value <= MAX_COORDINATE {
        Ok(value)
    } else {
        Err(refuse(None, what, "a length that must be positive is not"))
    }
}

fn point<P: Copy + Into<[f64; N]>, const N: usize>(p: P) -> Result<P, Error> {
    if finite(p.into()) {
        Ok(p)
    } else {
        Err(refuse(
            None,
            "point not finite",
            "a point is not finite, or is beyond the coordinate limit",
        ))
    }
}

fn unit<V: Copy + Into<[f64; N]>, const N: usize>(v: V) -> Result<V, Error> {
    if is_unit(v.into()) {
        Ok(v)
    } else {
        Err(refuse(
            None,
            "direction not unit",
            "a direction must be a finite unit vector",
        ))
    }
}

fn nonzero2(v: [f64; 2]) -> Result<[f64; 2], Error> {
    if finite(v) && length_squared(v) > 0.0 {
        Ok(v)
    } else {
        Err(refuse(
            None,
            "direction is zero",
            "a direction must be finite and not zero",
        ))
    }
}

/// A body's geometry: its vertices' points and its carriers (surfaces,
/// curves and p-curves), each table by its id ([`VertexId`],
/// [`SurfaceId`], [`CurveId`], [`PCurveId`]). Shared by every body that
/// shares it; a body's [`with_geometry`](super::Body::with_geometry)
/// replaces it whole (a CV drag, corners moving with it), keeping the
/// topology.
///
/// It has typed cache slots of its own ([`derived`](Self::derived)), for
/// what a kernel derives from the carriers alone (an offset's
/// approximation, a closed surface's seam gap, a carrier's bounds).
pub struct Geometry {
    points: Column<[f64; 3]>,
    surfaces: Column<Surface>,
    curves: Column<Curve>,
    pcurves: Column<PCurve>,
    derived: Derived,
}

impl Geometry {
    /// The vertices' `points` and the carrier tables. Each carrier was
    /// checked when made; a body built on them checks the tables' sizes
    /// and how its topology uses them.
    ///
    /// # Errors
    /// A point not finite, or beyond 1e100.
    pub fn new(
        points: impl Into<Column<[f64; 3]>>,
        surfaces: impl Into<Column<Surface>>,
        curves: impl Into<Column<Curve>>,
        pcurves: impl Into<Column<PCurve>>,
    ) -> Result<Self, Error> {
        let points = points.into();
        if let Some(v) = points.iter().position(|p| !finite(*p)) {
            return Err(refuse(
                Some(Entity::Vertex(VertexId::new(v))),
                "vertex not finite",
                "a vertex is not finite, or is beyond the coordinate limit",
            ));
        }
        Ok(Self {
            points,
            surfaces: surfaces.into(),
            curves: curves.into(),
            pcurves: pcurves.into(),
            derived: Derived::default(),
        })
    }

    /// Where vertex `v` is.
    ///
    /// # Panics
    /// When `v` is not of a body sharing these tables.
    pub fn point(&self, v: VertexId) -> Vec3 {
        Vec3::from(self.points[v.index()])
    }

    /// Every vertex's point, by id.
    pub fn points(&self) -> &Column<[f64; 3]> {
        &self.points
    }

    /// Surface `id`.
    ///
    /// # Panics
    /// When `id` is not of a body sharing these tables.
    pub fn surface(&self, id: SurfaceId) -> &Surface {
        &self.surfaces[id.index()]
    }

    /// Curve `id`.
    ///
    /// # Panics
    /// As [`surface`](Self::surface).
    pub fn curve(&self, id: CurveId) -> &Curve {
        &self.curves[id.index()]
    }

    /// P-curve `id`.
    ///
    /// # Panics
    /// As [`surface`](Self::surface).
    pub fn pcurve(&self, id: PCurveId) -> &PCurve {
        &self.pcurves[id.index()]
    }

    /// Every surface, by id.
    pub fn surfaces(&self) -> &Column<Surface> {
        &self.surfaces
    }

    /// Every curve, by id.
    pub fn curves(&self) -> &Column<Curve> {
        &self.curves
    }

    /// Every p-curve, by id.
    pub fn pcurves(&self) -> &Column<PCurve> {
        &self.pcurves
    }

    /// The `D` derived from these tables, built by `build` the first time
    /// any holder asks and dropped with them (the crate's cache rule; only
    /// the crate that defines `D` asks for it).
    pub fn derived<D: Send + Sync + 'static>(&self, build: impl FnOnce(&Geometry) -> D) -> Arc<D> {
        self.derived.get_or_build(|| build(self))
    }
}

/// Geometries are equal when their tables are.
impl PartialEq for Geometry {
    fn eq(&self, other: &Self) -> bool {
        self.points == other.points
            && self.surfaces == other.surfaces
            && self.curves == other.curves
            && self.pcurves == other.pcurves
    }
}

impl fmt::Debug for Geometry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Geometry")
            .field("points", &self.points)
            .field("surfaces", &self.surfaces)
            .field("curves", &self.curves)
            .field("pcurves", &self.pcurves)
            .finish()
    }
}
