//! The p-curves: curves in a face's chart `(u, v)`, and the [`PCurve`]
//! enum choosing among them.
use super::super::nurbs::NurbsCurve;
use super::{nonzero2, point, positive, unit};
use crate::Error;
use std::sync::Arc;

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
    pub fn new(origin: [f64; 2], direction: [f64; 2]) -> Result<Self, Error> {
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
    ) -> Result<Self, Error> {
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
    ) -> Result<Self, Error> {
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
/// as read-only, as for [`Surface`](super::Surface).
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
