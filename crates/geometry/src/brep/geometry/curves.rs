//! The curve carriers in space and the [`Curve`] enum choosing among them.
use super::super::nurbs::NurbsCurve;
use super::{point, positive, unit};
use crate::Error;
use crate::frame::Frame;
use crate::vec3::Vec3;
use std::f64::consts::TAU;
use std::sync::Arc;

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
    pub fn new(origin: Vec3, direction: Vec3) -> Result<Self, Error> {
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
    pub fn new(frame: Frame, radius: f64) -> Result<Self, Error> {
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
    pub fn new(frame: Frame, semi_axes: [f64; 2]) -> Result<Self, Error> {
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
/// as read-only, as for [`Surface`](super::Surface).
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
