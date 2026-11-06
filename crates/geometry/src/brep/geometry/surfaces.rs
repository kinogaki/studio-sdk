//! The surface carriers: analytic surfaces, sweeps and offsets, and the
//! [`Surface`] enum choosing among them.
use super::super::nurbs::NurbsSurface;
use super::{Curve, point, positive, refuse, unit};
use crate::Error;
use crate::frame::Frame;
use crate::limits::MAX_COORDINATE;
use crate::vec3::Vec3;
use std::f64::consts::FRAC_PI_2;
use std::sync::Arc;

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
    pub fn new(frame: Frame, radius: f64) -> Result<Self, Error> {
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
    pub fn new(frame: Frame, radius: f64, half_angle: f64) -> Result<Self, Error> {
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
    pub fn new(frame: Frame, radius: f64) -> Result<Self, Error> {
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
    pub fn new(frame: Frame, major: f64, minor: f64) -> Result<Self, Error> {
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
    pub fn new(profile: Arc<Curve>, direction: Vec3) -> Result<Self, Error> {
        let direction = unit(direction)?;
        if let Curve::Line(line) = profile.as_ref()
            && line.direction().cross(direction) == Vec3::ZERO
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
    pub fn new(profile: Arc<Curve>, origin: Vec3, axis: Vec3) -> Result<Self, Error> {
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
    pub fn new(base: Arc<Surface>, distance: f64) -> Result<Self, Error> {
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
