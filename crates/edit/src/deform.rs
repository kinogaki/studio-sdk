//! A move as data: [`Deform`], a [`Move`] applied to each point by its
//! weight, mirrored by an optional [`Symmetry`]. It is the one definition
//! the viewport's preview and the edit that commits the move both place
//! points with ([`Deform::place`], [`Editor::deform`](crate::editor::Editor::deform)), so the preview is
//! the commit.
//!
//! A point of weight `w` gets the move scaled by `w`: `w` of the
//! translation, the rotation turned `w` of its angle about the same axis,
//! and the scale moved `w` of the way from 1 (along the move's axes).
//! Weight 1 is the move itself; weight 0 leaves the point alone.
//!
//! With a symmetry, points on the primary side move by the move, points
//! on the other side by its mirror image (reflect, move, reflect back),
//! and points on the plane by the move then back onto the plane, so they
//! never leave it.
//!
//! ```
//! use kinogaki_edit::deform::{Axis, Deform, Move, Side, Symmetry};
//! use kinogaki_geometry::Vec3;
//!
//! let lift = Move::new(Vec3::new(1.0, 0.0, 0.0), [0.0, 0.0, 0.0, 1.0], Vec3::new(1.0, 1.0, 1.0), Vec3::ZERO).unwrap();
//! let mirrored = Deform {
//!     step: lift,
//!     weights: None,
//!     symmetry: Symmetry::new(Axis::X, 0.0, 1e-9, Side::Positive),
//! };
//! assert_eq!(mirrored.place(Vec3::new(2.0, 0.0, 0.0), 1.0), Vec3::new(3.0, 0.0, 0.0));
//! assert_eq!(mirrored.place(Vec3::new(-2.0, 0.0, 0.0), 1.0), Vec3::new(-3.0, 0.0, 0.0));
//! assert_eq!(mirrored.place(Vec3::new(2.0, 0.0, 0.0), 0.5), Vec3::new(2.5, 0.0, 0.0));
//! ```
use kinogaki_geometry::{Vec3, Xform};
use std::sync::Arc;

/// A coordinate axis.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Axis {
    /// X.
    X,
    /// Y.
    Y,
    /// Z.
    Z,
}

impl Axis {
    /// Its coordinate's index in a point: 0, 1 or 2.
    pub fn index(self) -> usize {
        match self {
            Axis::X => 0,
            Axis::Y => 1,
            Axis::Z => 2,
        }
    }

    /// `v`'s coordinate along it.
    fn of(self, v: Vec3) -> f64 {
        v.to_array()[self.index()]
    }

    /// `v` with its coordinate along it set to `c`.
    fn set(self, v: Vec3, c: f64) -> Vec3 {
        let mut a = v.to_array();
        a[self.index()] = c;
        Vec3::from(a)
    }
}

/// Which side of a symmetry plane a point lies on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Side {
    /// Below the plane.
    Negative,
    /// On it, within the tolerance.
    On,
    /// Above it.
    Positive,
}

/// The plane where a point's `axis` coordinate is `offset` (points within
/// `tolerance` of it lie on it), and the side that moves by the move
/// itself (`On`: every side).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Symmetry {
    axis: Axis,
    offset: f64,
    tolerance: f64,
    primary: Side,
}

impl Symmetry {
    /// The plane across `axis` at `offset`; `None` when `offset` isn't
    /// finite or `tolerance` isn't a finite non-negative distance.
    pub fn new(axis: Axis, offset: f64, tolerance: f64, primary: Side) -> Option<Self> {
        (offset.is_finite() && tolerance.is_finite() && tolerance >= 0.0).then_some(Self {
            axis,
            offset,
            tolerance,
            primary,
        })
    }

    /// The axis the plane crosses.
    pub fn axis(&self) -> Axis {
        self.axis
    }

    /// Where the plane crosses its axis.
    pub fn offset(&self) -> f64 {
        self.offset
    }

    /// How near the plane a point lies on it.
    pub fn tolerance(&self) -> f64 {
        self.tolerance
    }

    /// The side that moves by the move itself.
    pub fn primary(&self) -> Side {
        self.primary
    }

    /// The side `p` lies on.
    pub fn side(&self, p: Vec3) -> Side {
        let d = self.axis.of(p) - self.offset;
        if d.abs() <= self.tolerance {
            Side::On
        } else if d > 0.0 {
            Side::Positive
        } else {
            Side::Negative
        }
    }

    /// Whether `p` moves by the move's mirror image.
    pub fn mirrored(&self, p: Vec3) -> bool {
        let side = self.side(p);
        side != Side::On && self.primary != Side::On && side != self.primary
    }

    fn reflect(&self, p: Vec3) -> Vec3 {
        self.axis.set(p, 2.0 * self.offset - self.axis.of(p))
    }
}

/// A translation, then a rotation and a scale about a pivot
/// (`p ↦ pivot + translate + rotation · (scale ⊙ (p − pivot))`, the scale
/// along the move's axes): the move a weight of 1 makes. Built only
/// through [`new`](Self::new), so its rotation is always a unit axis and
/// an angle in `[-π, π]`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Move {
    translate: Vec3,
    axis: Vec3,
    angle: f64,
    scale: Vec3,
    pivot: Vec3,
    axes: [f64; 4],
}

impl Move {
    /// The move of `translate`, then `rotation` (a quaternion `[x, y, z,
    /// w]`, normalized here) and `scale` about `pivot`; `None` when any
    /// input isn't finite or the rotation is zero.
    pub fn new(translate: Vec3, rotation: [f64; 4], scale: Vec3, pivot: Vec3) -> Option<Self> {
        let finite = rotation.iter().all(|c| c.is_finite());
        if !(finite && translate.is_finite() && scale.is_finite() && pivot.is_finite()) {
            return None;
        }
        let q = normalized(rotation)?;
        // The shortest arc: q and −q are one rotation.
        let q = if q[3] < 0.0 { q.map(|c| -c) } else { q };
        let s = (1.0 - q[3] * q[3]).max(0.0).sqrt();
        let angle = 2.0 * q[3].clamp(-1.0, 1.0).acos();
        let (axis, angle) = if s < 1e-15 || angle.abs() < 1e-15 {
            (Vec3::Z, 0.0)
        } else {
            (Vec3::new(q[0] / s, q[1] / s, q[2] / s), angle)
        };
        Some(Self {
            translate,
            axis,
            angle,
            scale,
            pivot,
            axes: [0.0, 0.0, 0.0, 1.0],
        })
    }

    /// Scaled along the axes of `axes` (a quaternion, normalized here)
    /// instead of the value's own; `None` when it isn't a finite non-zero
    /// rotation.
    pub fn with_axes(mut self, axes: [f64; 4]) -> Option<Self> {
        if !axes.iter().all(|c| c.is_finite()) {
            return None;
        }
        self.axes = normalized(axes)?;
        Some(self)
    }

    /// The translation at full weight.
    pub fn translate(&self) -> Vec3 {
        self.translate
    }

    /// The unit rotation axis.
    pub fn axis(&self) -> Vec3 {
        self.axis
    }

    /// The rotation angle at full weight, radians in `[-π, π]`.
    pub fn angle(&self) -> f64 {
        self.angle
    }

    /// The scale at full weight, along [`axes`](Self::axes).
    pub fn scale(&self) -> Vec3 {
        self.scale
    }

    /// The rotation and scale pivot.
    pub fn pivot(&self) -> Vec3 {
        self.pivot
    }

    /// The axes the scale acts along, a unit quaternion `[x, y, z, w]`.
    pub fn axes(&self) -> [f64; 4] {
        self.axes
    }

    /// The full move as a placement (weight 1).
    pub fn xform(&self) -> Xform {
        let origin = self.weighted(Vec3::ZERO, 1.0);
        let axis = |e: Vec3| self.weighted(e, 1.0) - origin;
        Xform::from_axes(axis(Vec3::X), axis(Vec3::Y), axis(Vec3::Z), origin)
    }

    /// Whether every point moves rigidly with its side: no scale, or a
    /// uniform one.
    pub fn uniform_scale(&self) -> bool {
        let s = self.scale;
        (s.x - s.y).abs() <= s.x.abs() * 1e-12 && (s.x - s.z).abs() <= s.x.abs() * 1e-12
    }

    /// `p` moved at weight `w`.
    fn weighted(&self, p: Vec3, w: f64) -> Vec3 {
        let s = self.scale;
        let scale = Vec3::new(
            1.0 + (s.x - 1.0) * w,
            1.0 + (s.y - 1.0) * w,
            1.0 + (s.z - 1.0) * w,
        );
        let local = rotate(conjugate(self.axes), p - self.pivot);
        let stretched = Vec3::new(scale.x * local.x, scale.y * local.y, scale.z * local.z);
        let scaled = rotate(self.axes, stretched);
        let turned = rotate(axis_angle(self.axis, self.angle * w), scaled);
        self.pivot + self.translate * w + turned
    }
}

/// A move as data: the step, each moved point's weight and the symmetry.
/// A drag's preview and its commit place every point with
/// [`place`](Self::place), so what the preview shows is what the commit
/// makes.
#[derive(Clone, Debug, PartialEq)]
pub struct Deform {
    /// The move at full weight.
    pub step: Move,
    /// Each moved point's weight (0 fixed, 1 moving fully), in the order
    /// of the points it moves ([`Editor::deform`](crate::editor::Editor::deform)'s, or
    /// [`Moving::points`](crate::preview::Moving::points)): a soft move.
    /// `None`: every point moves fully.
    pub weights: Option<Arc<[f32]>>,
    /// Mirrored across this plane: a symmetric move.
    pub symmetry: Option<Symmetry>,
}

impl Deform {
    /// The move of `step` alone: every point fully, no symmetry.
    pub fn rigid(step: Move) -> Self {
        Self {
            step,
            weights: None,
            symmetry: None,
        }
    }

    /// Where point `p` of weight `weight` goes: unmoved at weight 0; on
    /// the mirrored side by the move's image (reflect, move, reflect
    /// back); on the symmetry plane, moved then back onto it.
    pub fn place(&self, p: Vec3, weight: f32) -> Vec3 {
        if weight <= 0.0 {
            return p;
        }
        let w = f64::from(weight);
        match self.symmetry {
            Some(s) if s.side(p) == Side::On => s.axis.set(self.step.weighted(p, w), s.offset),
            Some(s) if s.mirrored(p) => s.reflect(self.step.weighted(s.reflect(p), w)),
            _ => self.step.weighted(p, w),
        }
    }

    /// The full move as a placement (weight 1, without the symmetry).
    pub fn xform(&self) -> Xform {
        self.step.xform()
    }
}

/// `q` of unit length; `None` for a zero quaternion.
fn normalized(q: [f64; 4]) -> Option<[f64; 4]> {
    let length = q.iter().map(|c| c * c).sum::<f64>().sqrt();
    (length > 0.0).then(|| q.map(|c| c / length))
}

fn conjugate(q: [f64; 4]) -> [f64; 4] {
    [-q[0], -q[1], -q[2], q[3]]
}

fn axis_angle(axis: Vec3, angle: f64) -> [f64; 4] {
    let (s, c) = (angle * 0.5).sin_cos();
    [axis.x * s, axis.y * s, axis.z * s, c]
}

/// `v` turned by the unit quaternion `q`.
fn rotate(q: [f64; 4], v: Vec3) -> Vec3 {
    let u = Vec3::new(q[0], q[1], q[2]);
    let t = u.cross(v) * 2.0;
    let ut = u.cross(t);
    v + t * q[3] + ut
}

#[cfg(test)]
mod tests;
