//! [`WeightedMove`]: a transform applied with per-point weights and an
//! optional [`Mirror`], what a soft or symmetric move does to each point.
//! It is the one definition the viewport's preview and the edit that
//! commits the move both place points with, so the preview is the commit.
//!
//! A point of weight `w` gets the transform scaled by `w`: `w` of the
//! translation, the rotation turned `w` of its angle about the same axis,
//! and the scale moved `w` of the way from 1 (along the axes of the
//! frame). Weight 1 is the transform itself; weight 0 leaves the point
//! alone.
//!
//! With a mirror, points on the primary side move by the transform, points
//! on the other side by its mirror image (reflect, transform, reflect
//! back), and points on the plane by the transform then back onto the
//! plane, so they never leave it.
use crate::vec3::Vec3;
use crate::xform::Xform;

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
    /// `v`'s coordinate along it.
    fn of(self, v: Vec3) -> f64 {
        match self {
            Axis::X => v.x,
            Axis::Y => v.y,
            Axis::Z => v.z,
        }
    }

    /// `v` with its coordinate along it set to `c`.
    fn set(self, mut v: Vec3, c: f64) -> Vec3 {
        match self {
            Axis::X => v.x = c,
            Axis::Y => v.y = c,
            Axis::Z => v.z = c,
        }
        v
    }
}

/// Which side of a mirror plane a point lies on.
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
/// `tolerance` of it lie on it), and the side that moves by the transform
/// itself (`On`: every side).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mirror {
    axis: Axis,
    offset: f64,
    tolerance: f64,
    primary: Side,
}

impl Mirror {
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

    /// How far from the plane a point still lies on it.
    pub fn tolerance(&self) -> f64 {
        self.tolerance
    }

    /// The side moved by the transform; the other moves by its image.
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

    /// Whether `p` moves by the transform's mirror image.
    pub fn mirrored(&self, p: Vec3) -> bool {
        let side = self.side(p);
        side != Side::On && self.primary != Side::On && side != self.primary
    }

    fn reflect(&self, p: Vec3) -> Vec3 {
        self.axis.set(p, 2.0 * self.offset - self.axis.of(p))
    }
}

/// A move of a translation, then a rotation and a scale about a pivot
/// (`p ↦ pivot + translate + rotation · (scale ⊙ (p − pivot))`, the scale
/// along the frame's axes), applied to each point by its weight. Built
/// only through [`new`](Self::new), so its rotation is always a unit axis
/// and an angle in `[-π, π]`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WeightedMove {
    translate: Vec3,
    axis: Vec3,
    angle: f64,
    scale: Vec3,
    pivot: Vec3,
    frame: [f64; 4],
    mirror: Option<Mirror>,
}

impl WeightedMove {
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
            frame: [0.0, 0.0, 0.0, 1.0],
            mirror: None,
        })
    }

    /// Scaled along the axes of `frame` (a quaternion, normalized here)
    /// instead of the value's own; `None` when it isn't a finite non-zero
    /// rotation.
    pub fn with_frame(mut self, frame: [f64; 4]) -> Option<Self> {
        if !frame.iter().all(|c| c.is_finite()) {
            return None;
        }
        self.frame = normalized(frame)?;
        Some(self)
    }

    /// Mirrored across `mirror` (`None`: not mirrored).
    pub fn with_mirror(mut self, mirror: Option<Mirror>) -> Self {
        self.mirror = mirror;
        self
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

    /// The scale at full weight, along the frame's axes.
    pub fn scale(&self) -> Vec3 {
        self.scale
    }

    /// The rotation and scale pivot.
    pub fn pivot(&self) -> Vec3 {
        self.pivot
    }

    /// The frame the scale acts in, a unit quaternion `[x, y, z, w]`.
    pub fn frame(&self) -> [f64; 4] {
        self.frame
    }

    /// The mirror, if any.
    pub fn mirror(&self) -> Option<Mirror> {
        self.mirror
    }

    /// The full move as a placement (weight 1, primary side).
    pub fn xform(&self) -> Xform {
        let origin = self.weighted(Vec3::ZERO, 1.0);
        let axis = |e: Vec3| self.weighted(e, 1.0) - origin;
        Xform::from_axes(axis(Vec3::X), axis(Vec3::Y), axis(Vec3::Z), origin)
    }

    /// Where point `p` of weight `w` goes: identity at weight 0, and on the
    /// mirrored side the move's image (reflect, move, reflect back); on a
    /// mirror's plane, back onto the plane.
    pub fn at(&self, p: Vec3, w: f64) -> Vec3 {
        if w <= 0.0 {
            return p;
        }
        let q = self.image(p, w, self.mirrored(p));
        match self.mirror.filter(|m| m.side(p) == Side::On) {
            Some(mirror) => mirror.axis.set(q, mirror.offset),
            None => q,
        }
    }

    /// Whether every point moves rigidly with its side: no scale, or a
    /// uniform one.
    pub fn uniform_scale(&self) -> bool {
        let s = self.scale;
        (s.x - s.y).abs() <= s.x.abs() * 1e-12 && (s.x - s.z).abs() <= s.x.abs() * 1e-12
    }

    fn mirrored(&self, p: Vec3) -> bool {
        self.mirror.is_some_and(|m| m.mirrored(p))
    }

    /// `q` moved at weight `w` by the move, or by its mirror image.
    fn image(&self, q: Vec3, w: f64, mirrored: bool) -> Vec3 {
        match self.mirror {
            Some(mirror) if mirrored => mirror.reflect(self.weighted(mirror.reflect(q), w)),
            _ => self.weighted(q, w),
        }
    }

    fn weighted(&self, p: Vec3, w: f64) -> Vec3 {
        let s = self.scale;
        let scale = Vec3::new(
            1.0 + (s.x - 1.0) * w,
            1.0 + (s.y - 1.0) * w,
            1.0 + (s.z - 1.0) * w,
        );
        let local = rotate(conjugate(self.frame), p - self.pivot);
        let stretched = Vec3::new(scale.x * local.x, scale.y * local.y, scale.z * local.z);
        let scaled = rotate(self.frame, stretched);
        let turned = rotate(axis_angle(self.axis, self.angle * w), scaled);
        self.pivot + self.translate * w + turned
    }
}

/// `a × b`, in this module's order of operations.
fn cross(a: Vec3, b: Vec3) -> Vec3 {
    Vec3::new(
        a.y * b.z - a.z * b.y,
        a.z * b.x - a.x * b.z,
        a.x * b.y - a.y * b.x,
    )
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
    let t = cross(u, v) * 2.0;
    let ut = cross(u, t);
    v + t * q[3] + ut
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: Vec3, b: Vec3) -> bool {
        (a - b).length() < 1e-12
    }

    fn v(x: f64, y: f64, z: f64) -> Vec3 {
        Vec3::new(x, y, z)
    }

    const ONES: Vec3 = Vec3::new(1.0, 1.0, 1.0);

    /// A quarter turn about Z.
    fn quarter() -> [f64; 4] {
        axis_angle(Vec3::Z, std::f64::consts::FRAC_PI_2)
    }

    #[test]
    fn full_weight_is_the_matrix_and_none_is_still() {
        let m = WeightedMove::new(
            v(1.0, 2.0, 3.0),
            quarter(),
            v(2.0, 1.0, 1.0),
            v(0.5, 0.0, 0.0),
        )
        .unwrap();
        for p in [Vec3::ZERO, Vec3::X, v(0.3, -2.0, 4.0)] {
            assert!(close(m.at(p, 1.0), m.xform().apply_point(p)), "{p:?}");
            assert_eq!(m.at(p, 0.0), p);
        }
        // A quarter turn about the pivot, then the translation.
        assert!(close(m.at(v(1.5, 0.0, 0.0), 1.0), v(1.5, 2.0 + 2.0, 3.0)));
    }

    #[test]
    fn half_weight_goes_half_way() {
        let m = WeightedMove::new(v(2.0, 0.0, 0.0), quarter(), ONES, Vec3::ZERO).unwrap();
        // Half the translation and an eighth turn.
        let eighth = std::f64::consts::FRAC_PI_4;
        let q = m.at(Vec3::X, 0.5);
        assert!(close(q, v(1.0 + eighth.cos(), eighth.sin(), 0.0)));
        assert!((m.angle() - std::f64::consts::FRAC_PI_2).abs() < 1e-12);
    }

    #[test]
    fn a_rotation_and_its_negation_move_alike() {
        let q = quarter();
        let a = WeightedMove::new(Vec3::ZERO, q, ONES, Vec3::ZERO).unwrap();
        let b = WeightedMove::new(Vec3::ZERO, q.map(|c| -c), ONES, Vec3::ZERO).unwrap();
        assert!(close(
            a.at(v(1.0, 2.0, 3.0), 0.3),
            b.at(v(1.0, 2.0, 3.0), 0.3)
        ));
        assert!(WeightedMove::new(v(f64::NAN, 0.0, 0.0), q, ONES, Vec3::ZERO).is_none());
        assert!(WeightedMove::new(Vec3::ZERO, [0.0; 4], ONES, Vec3::ZERO).is_none());
    }

    #[test]
    fn mirrored_sides_move_as_images_and_the_plane_holds() {
        let mirror = Mirror::new(Axis::X, 0.0, 1e-6, Side::Positive).unwrap();
        let m = WeightedMove::new(v(1.0, 0.5, 0.0), [0.0, 0.0, 0.0, 1.0], ONES, Vec3::ZERO)
            .unwrap()
            .with_mirror(Some(mirror));
        // The primary side moves by the transform, the other by its image.
        assert_eq!(m.at(v(2.0, 0.0, 0.0), 1.0), v(3.0, 0.5, 0.0));
        assert_eq!(m.at(v(-2.0, 0.0, 0.0), 1.0), v(-3.0, 0.5, 0.0));
        // On the plane: moved, then back onto it.
        assert_eq!(m.at(Vec3::ZERO, 1.0), v(0.0, 0.5, 0.0));
        assert!(Mirror::new(Axis::Y, f64::INFINITY, 0.0, Side::On).is_none());
        assert!(Mirror::new(Axis::Y, 0.0, -1.0, Side::On).is_none());
    }

    #[test]
    fn scale_acts_along_the_frame() {
        // Stretch 2× along the frame's X, the frame turned a quarter about Z:
        // the value's Y.
        let m = WeightedMove::new(
            Vec3::ZERO,
            [0.0, 0.0, 0.0, 1.0],
            v(2.0, 1.0, 1.0),
            Vec3::ZERO,
        )
        .unwrap()
        .with_frame(quarter())
        .unwrap();
        assert!(close(m.at(Vec3::Y, 1.0), v(0.0, 2.0, 0.0)));
        assert!(close(m.at(Vec3::X, 1.0), Vec3::X));
        assert!(!m.uniform_scale());
    }
}
