//! [`Brep`]: boundary-representation bodies with exact geometry, free
//! curves and datums — the standard value `kinogaki.brep`.
//!
//! A [`Body`] is an immutable arena of tables: its [`Geometry`] (the
//! vertices' points, and carrier surfaces, curves and p-curves), its
//! [`BrepTopology`] (vertices, edges, coedges, loops, faces, shells and
//! lumps), the entities' persistent [`Names`] and its [`Attributes`]. Ids
//! are dense typed `u32` indices ([`FaceId`], [`EdgeId`]…), each table
//! holding at most `MAX_ENTITIES` (2²⁸) entities. Any CAD kernel can
//! produce and consume one: the conventions below are the contract two
//! kernels agree on, and [`BrepBuilder::build`] refuses a body that
//! breaks the ones it can check.
//!
//! The value carries no algorithms beyond evaluating a carrier's point
//! ([`Curve::point`], [`PCurve::point`]): no closest points, intersections, bounds, meshing
//! or healing. A kernel's caches (a vertex → edges index, bounds, an
//! offset's approximation, a closed seam's gap, a mesher's output) live
//! in `BrepTopology::derived` and [`Geometry::derived`] under the
//! crate's cache rule. Display triangles are a kernel's mesher's output,
//! not part of the value.
//!
//! # Units
//!
//! Positions, lengths, every tolerance and precision are in model units
//! (the project's unit is `kinogaki.tolerances`', [`Tolerances`], and the
//! value doesn't repeat it). Angles are in radians; degrees appear only in
//! user-facing settings such as [`Tolerances`]' T.
//!
//! # Conventions
//!
//! **Charts.** Each surface's parameters `(u, v)`, θ and φ in radians, θ
//! unbounded with period 2π:
//!
//! | surface | parameters | point |
//! |---|---|---|
//! | [`Plane`] | `(u, v)` | `o + u·x + v·y` |
//! | [`Cylinder`] | `(θ, h)` | `c + r(cos θ·x + sin θ·y) + h·z` |
//! | [`Cone`] | `(θ, h)` | `c + (r + h·tan α)(cos θ·x + sin θ·y) + h·z`; apex at `h = −r/tan α` |
//! | [`Sphere`] | `(θ, φ)` | `c + r cos φ (cos θ·x + sin θ·y) + r sin φ·z`; poles at `φ = ±π/2` |
//! | [`Torus`] | `(θ, φ)` | `c + (R + r cos φ)(cos θ·x + sin θ·y) + r sin φ·z` |
//! | [`Extrusion`] | `(t, v)` | `profile(t) + v·direction` |
//! | [`Revolution`] | `(θ, t)` | `profile(t)` turned by θ, right-handed about the axis |
//! | [`Offset`] | the base's | `base(u, v) + d·n_base(u, v)` |
//! | [`NurbsSurface`] | its knots' domain | the clamped tensor-product B-spline |
//!
//! **Surface normal.** `∂u × ∂v`, normalized ([`Surface::normal`]): the
//! cylinder's, cone's, sphere's and torus's point away from the axis or
//! centre. The chart is oriented so that `u × v` is the normal.
//!
//! **Curve parameters.** A [`Line`] in space is unit speed, so `t` is arc
//! length; a chart line ([`Line2`]) may have any non-zero speed. Circles
//! and ellipses take `t` as the angle; a chart circle's or ellipse's
//! `counter_clockwise` says which way it turns in the chart, so it shares
//! its edge's parameter. NURBS curves run over their knots' domain.
//!
//! **Periods.** 2π in θ, and in a closed NURBS
//! direction the domain's width (evaluation wraps there). A parameter of
//! a periodic direction may lie in any lift: `S(u + period) = S(u)`.
//!
//! **Face sense.** `Same` when the face's outward normal (out of the
//! material, for a solid) is the surface's normal.
//!
//! **Edges.** An edge is the interval `[t0, t1]` of its carrier, `t0 <
//! t1`, from `vertices[0]` (at `t0`) to `vertices[1]` (at `t1`).
//!
//! **Coedge sense.** `Same` when the coedge runs from its edge's start to
//! its end. A coedge's p-curve shares its edge's parameter:
//! `surface(pcurve(t)) ≈ curve(t)` over `[t0, t1]`.
//!
//! **Loops.** A loop is a cycle of coedges, each ending where the next
//! starts. A face's outer loop comes first. Material is on the left of
//! the loop's direction in the face's chart (oriented so `u × v` is the
//! surface normal) when the face's sense is `Same`, on the right when
//! `Reversed`. A face on a closed surface (a whole sphere or torus) may
//! have no loops.
//!
//! **Seams.** Periodic surfaces carry no seam edges: a loop may wrap a
//! period, and its p-curves record the lift (the chart window they are
//! drawn in); consecutive p-curves of a loop share a lift, and only the
//! step closing a loop may jump whole periods. A **seam edge** (one edge,
//! two coedges on one face) is allowed where a kernel wants one, as on a
//! closed NURBS surface.
//!
//! **Pole edges.** An edge without a curve is a pole: both its vertices
//! are one, and every coedge of it has a p-curve, the pole's line in the
//! chart. A pole edge used once leaves its shell closed.
//!
//! **Radial rings.** An edge has one coedge or two; its ring runs from
//! [`Edge::coedge`] ([`BrepTopology::edge_coedges`]; a boundary edge's
//! ring is itself). Two coedges of an edge run opposite ways, in one shell.
//!
//! **Shells and lumps.** A shell is a connected set of faces, each face in
//! exactly one shell; it is closed when every edge of it but a pole has
//! two coedges. A lump is its outer shell, then its voids. A body is
//! `Solid` when every shell is closed, else `Sheet` ([`BodyKind`]).
//!
//! **Tolerance.** A body carries the positional tolerance P it was built
//! to ([`Body::tolerance`]), which lies in [`Tolerances`]' positional
//! range once converted to millimeters. A vertex, edge, coedge or face
//! may carry a precision: it is tolerant within that distance, a measured
//! gap the data admits. Two points are one when they are within the
//! largest of P and the precisions of the entities a decision involves.
//! Nothing else about tolerancing is standard: fit, angle and curvature
//! are kernel policy.
//!
//! **Closed NURBS directions** ([`NurbsSurface::closed`]) are a
//! declaration: the first and last control rows coincide within P, with
//! the stated seam continuity. A reader may rely on it.
//!
//! **Offsets** are exact data; any approximation of one is a kernel's
//! cache.
//!
//! **Names.** Within a body, the faces' names are distinct, and so are
//! the edges' and the vertices'. [`Name`]s are data: which names answer a
//! reference after splits and merges is a kernel's policy.
//!
//! # What the builder checks
//!
//! [`BrepBuilder::build`] checks structure, and geometric consistency at
//! the points the data names, and nothing more:
//!
//! - **Structure:** every id in range; each table within the limits; no
//!   empty loops, shells or lumps; loops close in topology; each face in
//!   one shell and each shell in one lump; rings as above; intervals
//!   finite with `t0 < t1`; pole edges well formed; precisions finite and
//!   not negative; names distinct; placements rigid; attributes sized to
//!   their classes.
//! - **Consistency, within P:** each edge's carrier at `t0` and `t1` is at
//!   its vertices; each coedge's p-curve, at its edge's ends, maps through
//!   the surface onto the edge there (onto its vertex for a pole);
//!   consecutive p-curves in a loop meet, in one lift but at the closing
//!   step; closed NURBS rows coincide.
//!
//! **A producer's contract beyond that:** edges lie on their faces along
//! their whole length (within P or their precision), loops wind as the
//! convention says, faces don't self-intersect or overlap, and a solid's
//! shells enclose volume (outer shells positive, voids negative). These
//! need sampling densities, a kernel's policy, so the builder leaves them
//! to the producing kernel's own checks and the conformance suite every
//! kernel's output runs through.
//!
//! # Unknown kinds
//!
//! [`Surface`], [`Curve`], [`PCurve`], [`SeamContinuity`], [`Role`] and
//! [`DatumKind`] are `#[non_exhaustive]`. A value holding a kind a reader
//! doesn't know stays valid and passes through unchanged; to that reader
//! the body is read-only, and a mesher that can't evaluate a face reports
//! the face as failed.
//!
//! ```
//! use kinogaki_geometry::brep::{
//!     BodyKind, BrepBuilder, Circle, Circle2, CoedgeSpec, Curve, EdgeSpec, FaceSpec, Name,
//!     OperationId, PCurve, Plane, Role, Sense, Surface,
//! };
//! use kinogaki_geometry::{Attributes, Frame};
//! use std::f64::consts::TAU;
//!
//! // A unit disc: one plane face bounded by one circular edge.
//! let name = |index| Name::created(OperationId(1), Role::Primitive { index });
//! let mut b = BrepBuilder::new(name(0), 0.01);
//! let plane = b.surface(Surface::Plane(Plane::new(Frame::WORLD)));
//! let circle = b.curve(Curve::Circle(Circle::new(Frame::WORLD, 1.0).unwrap()));
//! let chart = b.pcurve(PCurve::Circle(Circle2::new([0.0; 2], [1.0, 0.0], 1.0, true).unwrap()));
//! let v = b.vertex([1.0, 0.0, 0.0], None, name(1));
//! let rim = b.edge(EdgeSpec { curve: Some(circle), interval: [0.0, TAU], vertices: [v, v], precision: None }, name(2));
//! let side = CoedgeSpec { edge: rim, sense: Sense::Same, pcurve: Some(chart), precision: None };
//! let disc = b.face(FaceSpec { surface: plane, sense: Sense::Same, precision: None }, vec![vec![side]], name(3));
//! let shell = b.shell(vec![disc]);
//! b.lump(vec![shell]);
//! let body = b.build(Attributes::new()).unwrap();
//! assert_eq!((body.kind(), body.topology().counts().edges), (BodyKind::Sheet, 1));
//! ```
mod body;
mod builder;
mod consistency;
mod eval;
#[cfg(test)]
mod eval_tests;
mod fingerprint;
mod geometry;
mod ids;
mod names;
mod numbering;
mod nurbs;
#[cfg(test)]
mod tests;
mod topology;

pub use body::{Body, Placement};
pub use builder::{BrepBuilder, CoedgeSpec, EdgeSpec, FaceSpec};
pub use geometry::{
    Circle, Circle2, Cone, Curve, Cylinder, Ellipse, Ellipse2, Extrusion, Geometry, Line, Line2,
    Offset, PCurve, Plane, Revolution, Sphere, Surface, Torus,
};
pub(crate) use ids::Entity;
pub use ids::{
    CoedgeId, CurveId, EdgeId, FaceId, LoopId, LumpId, PCurveId, ShellId, SurfaceId, VertexId,
};
pub use names::{End, History, Name, NameNode, OperationId, Role};
pub use numbering::Element;
pub use nurbs::{NurbsCurve, NurbsSurface, SeamContinuity};
pub use topology::{
    BodyKind, BrepTopology, Coedge, Counts, Edge, Face, Loop, Names, Sense, Shell, Vertex,
};

use crate::Frame;
#[cfg(doc)]
use crate::Tolerances;
use crate::attributes::{Attributes, Class};
use crate::column::Column;
use crate::curves::Curves;
use crate::error::GeometryError;
use crate::limits::MAX_COORDINATE;
use std::sync::Arc;

/// The most entities of one kind a body holds (2²⁸), and the most bodies
/// or datums a [`Brep`] holds.
pub(crate) const MAX_ENTITIES: usize = 1 << 28;

/// What a datum is.
///
/// New kinds may be added; a reader that doesn't know one passes it
/// through and shows nothing for it.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub enum DatumKind {
    /// A plane: the frame's origin lies on it, `z` is its normal, `x` and
    /// `y` its in-plane directions.
    Plane(Frame),
    /// An axis: a line through `origin` along the unit `direction`.
    Axis {
        /// A point on it.
        origin: [f64; 3],
        /// Its unit direction.
        direction: [f64; 3],
    },
    /// A point.
    Point([f64; 3]),
}

/// How a datum is drawn: around `center` (on the datum), `size` the half
/// side of a plane's square or half an axis's length. A size of zero
/// leaves it to the host.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DatumHint {
    /// The point it is drawn around.
    pub center: [f64; 3],
    /// Its half size, not negative.
    pub size: f64,
}

/// A named construction plane, axis or point, beside a value's bodies.
/// Plain data, checked by [`Brep::new`].
#[derive(Clone, Debug, PartialEq)]
pub struct Datum {
    /// Its persistent name.
    pub name: Name,
    /// What it is.
    pub kind: DatumKind,
    /// How it is drawn.
    pub hint: DatumHint,
}

/// A B-rep value: bodies, free curves and datums, with the value's own
/// Detail attributes and the record of the operation that made it.
///
/// **Classes:** Detail (one element) on the value; each body has its own
/// ([`Body`]). Free curves are [`Curves`] (exact by definition: rational
/// NURBS hold arcs exactly), each named by its Primitive `name`.
/// **Numbering:** the value's faces, edges and vertices are numbered as
/// one, across its bodies, free curves and datums ([`Brep::count`],
/// [`Brep::element`]); a selection keeps [`Brep::fingerprint`].
///
/// **Rules** ([`Brep::new`]): at most `MAX_ENTITIES` bodies and datums;
/// Detail attributes only, one element each; each datum finite, an axis's
/// direction unit and a hint's size not negative.
#[derive(Clone, Debug, PartialEq)]
pub struct Brep {
    bodies: Column<Body>,
    curves: Curves,
    datums: Column<Datum>,
    detail: Arc<Attributes>,
    history: Arc<History>,
}

impl Brep {
    /// The value of `bodies`, `curves` and `datums`, with `detail`
    /// attributes and `history`, checked.
    ///
    /// # Errors
    /// The first rule broken.
    pub fn new(
        bodies: Vec<Body>,
        curves: Curves,
        datums: Vec<Datum>,
        detail: Attributes,
        history: History,
    ) -> Result<Self, GeometryError> {
        let value = Self {
            bodies: bodies.into(),
            curves,
            datums: datums.into(),
            detail: Arc::new(detail),
            history: Arc::new(history),
        };
        value.check()?;
        Ok(value)
    }

    /// Nothing: no bodies, curves or datums.
    pub fn empty() -> Self {
        Self::new(
            Vec::new(),
            Curves::empty(),
            Vec::new(),
            Attributes::new(),
            History::default(),
        )
        .expect("an empty value is valid")
    }

    fn check(&self) -> Result<(), GeometryError> {
        if self.bodies.len() > MAX_ENTITIES || self.datums.len() > MAX_ENTITIES {
            return Err(GeometryError::brep(
                None,
                "too many entities",
                "a B-rep value holds at most the entity limit of bodies and of datums".into(),
            ));
        }
        for (i, d) in self.datums.iter().enumerate() {
            let finite = |p: [f64; 3]| p.iter().all(|c| c.abs() <= MAX_COORDINATE);
            let unit = |v: [f64; 3]| {
                finite(v)
                    && ((v[0] * v[0] + v[1] * v[1] + v[2] * v[2]) - 1.0).abs()
                        <= 16.0 * f64::EPSILON
            };
            let kind = match d.kind {
                DatumKind::Plane(_) => true,
                DatumKind::Axis { origin, direction } => finite(origin) && unit(direction),
                DatumKind::Point(p) => finite(p),
            };
            let hint = finite(d.hint.center)
                && d.hint.size.is_finite()
                && d.hint.size >= 0.0
                && d.hint.size <= MAX_COORDINATE;
            if !(kind && hint) {
                return Err(GeometryError::brep(
                    Some(Entity::Datum(i)),
                    "datum not finite",
                    "a datum is finite, an axis's direction unit and a hint's size not negative"
                        .into(),
                ));
            }
        }
        self.detail.check("B-rep values", |class| {
            (class == Class::Detail).then_some(1)
        })
    }

    /// The bodies.
    pub fn bodies(&self) -> &Column<Body> {
        &self.bodies
    }

    /// The free curves.
    pub fn curves(&self) -> &Curves {
        &self.curves
    }

    /// The datums.
    pub fn datums(&self) -> &Column<Datum> {
        &self.datums
    }

    /// The value's Detail attributes.
    pub fn detail(&self) -> &Arc<Attributes> {
        &self.detail
    }

    /// What the operation that made it did to its input's names.
    pub fn history(&self) -> &Arc<History> {
        &self.history
    }

    /// This value with `bodies`, sharing the rest.
    ///
    /// # Errors
    /// Too many bodies.
    pub fn with_bodies(&self, bodies: Vec<Body>) -> Result<Self, GeometryError> {
        let value = Self {
            bodies: bodies.into(),
            ..self.clone()
        };
        value.check()?;
        Ok(value)
    }

    /// This value with `curves`, sharing the rest.
    pub fn with_curves(&self, curves: Curves) -> Self {
        Self {
            curves,
            ..self.clone()
        }
    }

    /// This value with `detail` attributes, sharing the rest.
    ///
    /// # Errors
    /// An attribute of another class than Detail, or missized.
    pub fn with_detail(&self, detail: Attributes) -> Result<Self, GeometryError> {
        let value = Self {
            detail: Arc::new(detail),
            ..self.clone()
        };
        value.check()?;
        Ok(value)
    }
}

impl Default for Brep {
    fn default() -> Self {
        Self::empty()
    }
}
