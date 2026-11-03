//! The carriers: [`Surface`]s and [`Curve`]s in space, [`PCurve`]s in a
//! face's chart, and the [`Geometry`] tables a body keeps them in.
//!
//! Every kind is a validated value made by its constructor; the enums
//! only choose among them. Lengths are in model units, angles in
//! radians. The parameterizations are the contract (see [`super`]).
use super::nurbs::{NurbsCurve, NurbsSurface};
use super::{CurveId, Entity, PCurveId, SurfaceId, VertexId};
use crate::column::Column;
use crate::derived::Derived;
use crate::error::GeometryError;
use crate::frame::{Frame, finite, is_unit, length_squared};
use crate::limits::MAX_COORDINATE;
use crate::vec3::Vec3;
use std::f64::consts::{FRAC_PI_2, TAU};
use std::fmt;
use std::sync::Arc;

pub(crate) fn refuse(entity: Option<Entity>, phrase: &'static str, detail: &str) -> GeometryError {
    GeometryError::brep(entity, phrase, detail.into())
}

fn positive(value: f64, what: &'static str) -> Result<f64, GeometryError> {
    if value.is_finite() && value > 0.0 && value <= MAX_COORDINATE {
        Ok(value)
    } else {
        Err(refuse(None, what, "a length that must be positive is not"))
    }
}

fn point<P: Copy + Into<[f64; N]>, const N: usize>(p: P) -> Result<P, GeometryError> {
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

fn unit<V: Copy + Into<[f64; N]>, const N: usize>(v: V) -> Result<V, GeometryError> {
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

fn nonzero2(v: [f64; 2]) -> Result<[f64; 2], GeometryError> {
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

/// `o + u·x + v·y`; normal `z`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Plane {
    frame: Frame,
}

impl Plane {
    /// The plane of `frame`'s x and y axes.
    pub fn new(frame: Frame) -> Self {
        Self { frame }
    }

    /// Its frame.
    pub fn frame(&self) -> &Frame {
        &self.frame
    }
}

/// `c + r(cos θ·x + sin θ·y) + h·z`, parameters `(θ, h)`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cylinder {
    frame: Frame,
    radius: f64,
}

impl Cylinder {
    /// The cylinder of `radius` about `frame`'s z axis.
    ///
    /// # Errors
    /// A radius that isn't positive.
    pub fn new(frame: Frame, radius: f64) -> Result<Self, GeometryError> {
        Ok(Self {
            frame,
            radius: positive(radius, "radius not positive")?,
        })
    }

    /// Its frame: the axis is `z` through the origin.
    pub fn frame(&self) -> &Frame {
        &self.frame
    }

    /// The radius.
    pub fn radius(&self) -> f64 {
        self.radius
    }
}

/// `c + (r + h·tan α)(cos θ·x + sin θ·y) + h·z`, parameters `(θ, h)`:
/// radius `r ≥ 0` at `h = 0` (zero puts the apex at the origin), half
/// angle `α` in `(0, π/2)`. The apex is at `h = −r / tan α`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cone {
    frame: Frame,
    radius: f64,
    half_angle: f64,
}

impl Cone {
    /// The cone about `frame`'s z axis, `radius` at its origin, opening by
    /// `half_angle` radians towards `+z`.
    ///
    /// # Errors
    /// A negative radius, or a half angle outside `(0, π/2)`.
    pub fn new(frame: Frame, radius: f64, half_angle: f64) -> Result<Self, GeometryError> {
        if !(0.0..=MAX_COORDINATE).contains(&radius) {
            return Err(refuse(
                None,
                "radius negative",
                "a cone's radius must be finite and not negative",
            ));
        }
        if !(half_angle > 0.0 && half_angle < FRAC_PI_2) {
            return Err(refuse(
                None,
                "angle out of range",
                "a cone's half angle must be positive and below a quarter turn",
            ));
        }
        Ok(Self {
            frame,
            radius,
            half_angle,
        })
    }

    /// Its frame: the axis is `z` through the origin.
    pub fn frame(&self) -> &Frame {
        &self.frame
    }

    /// The radius at `h = 0`.
    pub fn radius(&self) -> f64 {
        self.radius
    }

    /// The half angle, in radians.
    pub fn half_angle(&self) -> f64 {
        self.half_angle
    }
}

/// `c + r cos φ (cos θ·x + sin θ·y) + r sin φ·z`, parameters `(θ, φ)`;
/// poles at `φ = ±π/2`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sphere {
    frame: Frame,
    radius: f64,
}

impl Sphere {
    /// The sphere of `radius` about `frame`'s origin.
    ///
    /// # Errors
    /// A radius that isn't positive.
    pub fn new(frame: Frame, radius: f64) -> Result<Self, GeometryError> {
        Ok(Self {
            frame,
            radius: positive(radius, "radius not positive")?,
        })
    }

    /// Its frame: the centre is the origin, the poles on `±z`.
    pub fn frame(&self) -> &Frame {
        &self.frame
    }

    /// The radius.
    pub fn radius(&self) -> f64 {
        self.radius
    }
}

/// `c + (R + r cos φ)(cos θ·x + sin θ·y) + r sin φ·z`, parameters
/// `(θ, φ)`: major radius `R ≥ 0`, minor `r > 0`. `r ≥ R` is a degenerate
/// (apple or lemon) torus, which is allowed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Torus {
    frame: Frame,
    major: f64,
    minor: f64,
}

impl Torus {
    /// The torus about `frame`'s z axis.
    ///
    /// # Errors
    /// A negative major radius, or a minor radius that isn't positive.
    pub fn new(frame: Frame, major: f64, minor: f64) -> Result<Self, GeometryError> {
        if !(0.0..=MAX_COORDINATE).contains(&major) {
            return Err(refuse(
                None,
                "radius negative",
                "a torus's major radius must be finite and not negative",
            ));
        }
        Ok(Self {
            frame,
            major,
            minor: positive(minor, "radius not positive")?,
        })
    }

    /// Its frame: the axis is `z` through the origin.
    pub fn frame(&self) -> &Frame {
        &self.frame
    }

    /// The major radius `R`: from the axis to the tube's centre.
    pub fn major(&self) -> f64 {
        self.major
    }

    /// The minor radius `r`: the tube's.
    pub fn minor(&self) -> f64 {
        self.minor
    }
}

/// `profile(t) + v·direction`, parameters `(t, v)`, with a unit
/// direction. A straight profile along the direction is refused: it
/// sweeps no surface.
#[derive(Clone, Debug, PartialEq)]
pub struct Extrusion {
    profile: Arc<Curve>,
    direction: Vec3,
}

impl Extrusion {
    /// `profile` swept along the unit `direction`.
    ///
    /// # Errors
    /// A direction that isn't unit, or a line swept along itself.
    pub fn new(profile: Arc<Curve>, direction: Vec3) -> Result<Self, GeometryError> {
        let direction = unit(direction)?;
        if let Curve::Line(line) = profile.as_ref()
            && line.direction.cross(direction) == Vec3::ZERO
        {
            return Err(refuse(
                None,
                "extrusion sweeps nothing",
                "a line extruded along itself sweeps no surface",
            ));
        }
        Ok(Self { profile, direction })
    }

    /// The profile.
    pub fn profile(&self) -> &Arc<Curve> {
        &self.profile
    }

    /// The unit direction.
    pub fn direction(&self) -> Vec3 {
        self.direction
    }
}

/// `profile(t)` turned by θ, right-handed about the axis through `origin`
/// along the unit `axis`, parameters `(θ, t)`: with `q = profile(t) − o`,
/// `o + q∥ + cos θ·q⊥ + sin θ·(axis × q⊥)`.
#[derive(Clone, Debug, PartialEq)]
pub struct Revolution {
    profile: Arc<Curve>,
    origin: Vec3,
    axis: Vec3,
}

impl Revolution {
    /// `profile` revolved about the axis through `origin` along the unit
    /// `axis`.
    ///
    /// # Errors
    /// An origin not finite, or an axis that isn't unit.
    pub fn new(profile: Arc<Curve>, origin: Vec3, axis: Vec3) -> Result<Self, GeometryError> {
        Ok(Self {
            profile,
            origin: point(origin)?,
            axis: unit(axis)?,
        })
    }

    /// The profile.
    pub fn profile(&self) -> &Arc<Curve> {
        &self.profile
    }

    /// A point of the axis.
    pub fn origin(&self) -> Vec3 {
        self.origin
    }

    /// The axis's unit direction.
    pub fn axis(&self) -> Vec3 {
        self.axis
    }
}

/// `base(u, v) + d·n(u, v)`, `n` the base's unit normal, in the base's
/// parameters. Exact data: any approximation of it is a kernel's cache.
/// Its base is never an offset (an offset of an offset is one offset of
/// the first base, by the sum of the distances).
#[derive(Clone, Debug, PartialEq)]
pub struct Offset {
    base: Arc<Surface>,
    distance: f64,
}

impl Offset {
    /// `base` displaced by `distance` along its normal.
    ///
    /// # Errors
    /// A distance zero or not finite, or a base that is itself an offset.
    pub fn new(base: Arc<Surface>, distance: f64) -> Result<Self, GeometryError> {
        if !(distance.is_finite() && distance != 0.0 && distance.abs() <= MAX_COORDINATE) {
            return Err(refuse(
                None,
                "offset distance zero",
                "an offset's distance must be finite and not zero",
            ));
        }
        if matches!(base.as_ref(), Surface::Offset(_)) {
            return Err(refuse(
                None,
                "offset of an offset",
                "an offset's base is not an offset: offset the first base by the sum",
            ));
        }
        Ok(Self { base, distance })
    }

    /// The base surface.
    pub fn base(&self) -> &Arc<Surface> {
        &self.base
    }

    /// The signed distance along the base's normal.
    pub fn distance(&self) -> f64 {
        self.distance
    }
}

/// An unbounded carrier surface. A face is a surface trimmed by loops;
/// several faces may share one.
///
/// New kinds may be added. A reader that doesn't know a body's surface
/// kind keeps the body as it is (it stays valid and passes through
/// unchanged) and treats it as read-only; a mesher that can't evaluate it
/// reports the face as failed.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum Surface {
    /// A plane.
    Plane(Plane),
    /// A circular cylinder.
    Cylinder(Cylinder),
    /// A circular cone.
    Cone(Cone),
    /// A sphere.
    Sphere(Sphere),
    /// A torus.
    Torus(Torus),
    /// A tensor-product NURBS surface.
    Nurbs(Arc<NurbsSurface>),
    /// A profile swept along a straight direction.
    Extrusion(Extrusion),
    /// A profile revolved about an axis.
    Revolution(Revolution),
    /// A surface displaced along its normal.
    Offset(Offset),
}

/// `origin + t·direction` with a unit direction: `t` is arc length.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Line {
    origin: Vec3,
    direction: Vec3,
}

impl Line {
    /// The line through `origin` along the unit `direction`.
    ///
    /// # Errors
    /// An origin not finite, or a direction that isn't unit.
    pub fn new(origin: Vec3, direction: Vec3) -> Result<Self, GeometryError> {
        Ok(Self {
            origin: point(origin)?,
            direction: unit(direction)?,
        })
    }

    /// The point at `t = 0`.
    pub fn origin(&self) -> Vec3 {
        self.origin
    }

    /// The unit direction.
    pub fn direction(&self) -> Vec3 {
        self.direction
    }
}

/// `c + r(cos t·x + sin t·y)` in its frame (centre the origin, normal
/// `z`); `t` is the angle, in radians, period 2π.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Circle {
    frame: Frame,
    radius: f64,
}

impl Circle {
    /// The circle of `radius` in `frame`'s xy plane.
    ///
    /// # Errors
    /// A radius that isn't positive.
    pub fn new(frame: Frame, radius: f64) -> Result<Self, GeometryError> {
        Ok(Self {
            frame,
            radius: positive(radius, "radius not positive")?,
        })
    }

    /// Its frame.
    pub fn frame(&self) -> &Frame {
        &self.frame
    }

    /// The radius.
    pub fn radius(&self) -> f64 {
        self.radius
    }
}

/// `c + a cos t·x + b sin t·y` in its frame; `t` is the angle, in
/// radians, period 2π. Either semi-axis may be the larger.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ellipse {
    frame: Frame,
    semi_axes: [f64; 2],
}

impl Ellipse {
    /// The ellipse in `frame`'s xy plane with semi-axes `[a, b]` along `x`
    /// and `y`.
    ///
    /// # Errors
    /// A semi-axis that isn't positive.
    pub fn new(frame: Frame, semi_axes: [f64; 2]) -> Result<Self, GeometryError> {
        Ok(Self {
            frame,
            semi_axes: [
                positive(semi_axes[0], "semi-axis not positive")?,
                positive(semi_axes[1], "semi-axis not positive")?,
            ],
        })
    }

    /// Its frame.
    pub fn frame(&self) -> &Frame {
        &self.frame
    }

    /// `[a, b]`: the semi-axes along the frame's x and y.
    pub fn semi_axes(&self) -> [f64; 2] {
        self.semi_axes
    }
}

/// An unbounded carrier curve in space. An edge is an interval of one;
/// several edges may share one.
///
/// New kinds may be added; a reader that doesn't know one treats the body
/// as read-only, as for [`Surface`].
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum Curve {
    /// A straight line.
    Line(Line),
    /// A circle.
    Circle(Circle),
    /// An ellipse.
    Ellipse(Ellipse),
    /// A NURBS curve.
    Nurbs(Arc<NurbsCurve<3>>),
}

impl Curve {
    /// The parameter period of a closed carrier: 2π for circles and
    /// ellipses; `None` for lines and NURBS curves (a NURBS curve is clamped,
    /// so open, even when its ends meet).
    pub fn period(&self) -> Option<f64> {
        match self {
            Curve::Circle(_) | Curve::Ellipse(_) => Some(TAU),
            Curve::Line(_) | Curve::Nurbs(_) => None,
        }
    }
}

/// `origin + t·direction` in a chart; the direction need not be unit (a
/// cone's generator advances `cos α` in `h` per unit of arc length).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Line2 {
    origin: [f64; 2],
    direction: [f64; 2],
}

impl Line2 {
    /// The chart line through `origin` along `direction`.
    ///
    /// # Errors
    /// A point not finite, or a direction zero or not finite.
    pub fn new(origin: [f64; 2], direction: [f64; 2]) -> Result<Self, GeometryError> {
        Ok(Self {
            origin: point(origin)?,
            direction: nonzero2(direction)?,
        })
    }

    /// The point at `t = 0`.
    pub fn origin(&self) -> [f64; 2] {
        self.origin
    }

    /// The direction (its length is the speed in the chart).
    pub fn direction(&self) -> [f64; 2] {
        self.direction
    }
}

/// `c + r(cos t·x + sin t·y)` in a chart, `x` unit and `y` the quarter
/// turn of `x` counter-clockwise (`counter_clockwise`) or clockwise, so a
/// circle of either sense shares its edge's parameter.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Circle2 {
    center: [f64; 2],
    x: [f64; 2],
    radius: f64,
    counter_clockwise: bool,
}

impl Circle2 {
    /// The chart circle about `center` starting along the unit `x`.
    ///
    /// # Errors
    /// A centre not finite, an `x` that isn't unit, or a radius that
    /// isn't positive.
    pub fn new(
        center: [f64; 2],
        x: [f64; 2],
        radius: f64,
        counter_clockwise: bool,
    ) -> Result<Self, GeometryError> {
        Ok(Self {
            center: point(center)?,
            x: unit(x)?,
            radius: positive(radius, "radius not positive")?,
            counter_clockwise,
        })
    }

    /// The centre.
    pub fn center(&self) -> [f64; 2] {
        self.center
    }

    /// The unit direction of `t = 0`.
    pub fn x(&self) -> [f64; 2] {
        self.x
    }

    /// The radius.
    pub fn radius(&self) -> f64 {
        self.radius
    }

    /// Whether it turns counter-clockwise in the chart as `t` grows.
    pub fn counter_clockwise(&self) -> bool {
        self.counter_clockwise
    }

    /// `(x, y)`: the chart directions of `t = 0` and `t = π/2`.
    pub fn axes(&self) -> ([f64; 2], [f64; 2]) {
        (self.x, quarter(self.x, self.counter_clockwise))
    }
}

/// `c + a cos t·x + b sin t·y` in a chart, `x` unit and `y` its quarter
/// turn as for [`Circle2`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ellipse2 {
    center: [f64; 2],
    x: [f64; 2],
    semi_axes: [f64; 2],
    counter_clockwise: bool,
}

impl Ellipse2 {
    /// The chart ellipse about `center` with semi-axes `[a, b]` along the
    /// unit `x` and its quarter turn.
    ///
    /// # Errors
    /// A centre not finite, an `x` that isn't unit, or a semi-axis that
    /// isn't positive.
    pub fn new(
        center: [f64; 2],
        x: [f64; 2],
        semi_axes: [f64; 2],
        counter_clockwise: bool,
    ) -> Result<Self, GeometryError> {
        Ok(Self {
            center: point(center)?,
            x: unit(x)?,
            semi_axes: [
                positive(semi_axes[0], "semi-axis not positive")?,
                positive(semi_axes[1], "semi-axis not positive")?,
            ],
            counter_clockwise,
        })
    }

    /// The centre.
    pub fn center(&self) -> [f64; 2] {
        self.center
    }

    /// The unit direction of `t = 0`.
    pub fn x(&self) -> [f64; 2] {
        self.x
    }

    /// `[a, b]`: the semi-axes along `x` and its quarter turn.
    pub fn semi_axes(&self) -> [f64; 2] {
        self.semi_axes
    }

    /// Whether it turns counter-clockwise in the chart as `t` grows.
    pub fn counter_clockwise(&self) -> bool {
        self.counter_clockwise
    }

    /// `(x, y)`: the chart directions of `t = 0` and `t = π/2`.
    pub(crate) fn axes(&self) -> ([f64; 2], [f64; 2]) {
        (self.x, quarter(self.x, self.counter_clockwise))
    }
}

/// `x` turned a quarter turn counter-clockwise, or clockwise.
fn quarter(x: [f64; 2], counter_clockwise: bool) -> [f64; 2] {
    if counter_clockwise {
        [-x[1], x[0]]
    } else {
        [x[1], -x[0]]
    }
}

/// A coedge's curve in its face's chart `(u, v)`, sharing its edge's
/// parameter: `surface(pcurve(t)) ≈ curve(t)`.
///
/// New kinds may be added; a reader that doesn't know one treats the body
/// as read-only, as for [`Surface`].
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum PCurve {
    /// A straight line in the chart.
    Line(Line2),
    /// A circle in the chart.
    Circle(Circle2),
    /// An ellipse in the chart.
    Ellipse(Ellipse2),
    /// A NURBS curve in the chart.
    Nurbs(Arc<NurbsCurve<2>>),
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
    ) -> Result<Self, GeometryError> {
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
