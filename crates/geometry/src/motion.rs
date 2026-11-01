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
    fn index(self) -> usize {
        match self {
            Axis::X => 0,
            Axis::Y => 1,
            Axis::Z => 2,
        }
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
    pub fn side(&self, p: [f64; 3]) -> Side {
        let d = p[self.axis.index()] - self.offset;
        if d.abs() <= self.tolerance {
            Side::On
        } else if d > 0.0 {
            Side::Positive
        } else {
            Side::Negative
        }
    }

    /// Whether `p` moves by the transform's mirror image.
    pub fn mirrored(&self, p: [f64; 3]) -> bool {
        let side = self.side(p);
        side != Side::On && self.primary != Side::On && side != self.primary
    }

    fn reflect(&self, mut p: [f64; 3]) -> [f64; 3] {
        let a = self.axis.index();
        p[a] = 2.0 * self.offset - p[a];
        p
    }
}

/// A move of a translation, then a rotation and a scale about a pivot
/// (`p ↦ pivot + translate + rotation · (scale ⊙ (p − pivot))`, the scale
/// along the frame's axes), applied to each point by its weight. Built
/// only through [`new`](Self::new), so its rotation is always a unit axis
/// and an angle in `[-π, π]`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WeightedMove {
    translate: [f64; 3],
    axis: [f64; 3],
    angle: f64,
    scale: [f64; 3],
    pivot: [f64; 3],
    frame: [f64; 4],
    mirror: Option<Mirror>,
}

impl WeightedMove {
    /// The move of `translate`, then `rotation` (a quaternion `[x, y, z,
    /// w]`, normalized here) and `scale` about `pivot`; `None` when any
    /// input isn't finite or the rotation is zero.
    pub fn new(
        translate: [f64; 3],
        rotation: [f64; 4],
        scale: [f64; 3],
        pivot: [f64; 3],
    ) -> Option<Self> {
        let finite = |v: &[f64]| v.iter().all(|c| c.is_finite());
        if !(finite(&translate) && finite(&rotation) && finite(&scale) && finite(&pivot)) {
            return None;
        }
        let q = normalized(rotation)?;
        // The shortest arc: q and −q are one rotation.
        let q = if q[3] < 0.0 { q.map(|c| -c) } else { q };
        let s = (1.0 - q[3] * q[3]).max(0.0).sqrt();
        let angle = 2.0 * q[3].clamp(-1.0, 1.0).acos();
        let (axis, angle) = if s < 1e-15 || angle.abs() < 1e-15 {
            ([0.0, 0.0, 1.0], 0.0)
        } else {
            ([q[0] / s, q[1] / s, q[2] / s], angle)
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
    pub fn translate(&self) -> [f64; 3] {
        self.translate
    }

    /// The unit rotation axis.
    pub fn axis(&self) -> [f64; 3] {
        self.axis
    }

    /// The rotation angle at full weight, radians in `[-π, π]`.
    pub fn angle(&self) -> f64 {
        self.angle
    }

    /// The scale at full weight, along the frame's axes.
    pub fn scale(&self) -> [f64; 3] {
        self.scale
    }

    /// The rotation and scale pivot.
    pub fn pivot(&self) -> [f64; 3] {
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

    /// The full move as a column-major matrix (weight 1, primary side).
    pub fn matrix(&self) -> [[f64; 4]; 4] {
        affine(|p| self.weighted(p, 1.0))
    }

    /// The affine map that moves point `p` of weight `w`, as a column-major
    /// matrix: identity at weight 0, and on the mirrored side the move's
    /// image (reflect, move, reflect back). It carries what a point holds
    /// (normals, orientations) with the point; on the plane it is the
    /// move itself, which [`at`](Self::at) then puts back onto the plane.
    pub fn matrix_at(&self, p: [f64; 3], w: f64) -> [[f64; 4]; 4] {
        if w <= 0.0 {
            return affine(|q| q);
        }
        let mirrored = self.mirrored(p);
        affine(|q| self.image(q, w, mirrored))
    }

    /// Where point `p` of weight `w` goes: [`matrix_at`](Self::matrix_at)
    /// applied to it, and on a mirror's plane back onto the plane.
    pub fn at(&self, p: [f64; 3], w: f64) -> [f64; 3] {
        if w <= 0.0 {
            return p;
        }
        let mut q = self.image(p, w, self.mirrored(p));
        if let Some(mirror) = self.mirror.filter(|m| m.side(p) == Side::On) {
            q[mirror.axis.index()] = mirror.offset;
        }
        q
    }

    /// Whether every point moves rigidly with its side: no scale, or a
    /// uniform one.
    pub fn uniform_scale(&self) -> bool {
        let s = self.scale;
        (s[0] - s[1]).abs() <= s[0].abs() * 1e-12 && (s[0] - s[2]).abs() <= s[0].abs() * 1e-12
    }

    fn mirrored(&self, p: [f64; 3]) -> bool {
        self.mirror.is_some_and(|m| m.mirrored(p))
    }

    /// `q` moved at weight `w` by the move, or by its mirror image.
    fn image(&self, q: [f64; 3], w: f64, mirrored: bool) -> [f64; 3] {
        match self.mirror {
            Some(mirror) if mirrored => mirror.reflect(self.weighted(mirror.reflect(q), w)),
            _ => self.weighted(q, w),
        }
    }

    fn weighted(&self, p: [f64; 3], w: f64) -> [f64; 3] {
        let scale: [f64; 3] = std::array::from_fn(|i| 1.0 + (self.scale[i] - 1.0) * w);
        let local = rotate(conjugate(self.frame), sub(p, self.pivot));
        let scaled = rotate(self.frame, std::array::from_fn(|i| scale[i] * local[i]));
        let turned = rotate(axis_angle(self.axis, self.angle * w), scaled);
        std::array::from_fn(|i| self.pivot[i] + self.translate[i] * w + turned[i])
    }
}

/// The column-major matrix of the affine map `f`, read from the images of
/// the origin and the unit axes.
fn affine(f: impl Fn([f64; 3]) -> [f64; 3]) -> [[f64; 4]; 4] {
    let origin = f([0.0; 3]);
    let column = |e: [f64; 3]| {
        let q = f(e);
        [q[0] - origin[0], q[1] - origin[1], q[2] - origin[2], 0.0]
    };
    [
        column([1.0, 0.0, 0.0]),
        column([0.0, 1.0, 0.0]),
        column([0.0, 0.0, 1.0]),
        [origin[0], origin[1], origin[2], 1.0],
    ]
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|i| a[i] - b[i])
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// `q` of unit length; `None` for a zero quaternion.
fn normalized(q: [f64; 4]) -> Option<[f64; 4]> {
    let length = q.iter().map(|c| c * c).sum::<f64>().sqrt();
    (length > 0.0).then(|| q.map(|c| c / length))
}

fn conjugate(q: [f64; 4]) -> [f64; 4] {
    [-q[0], -q[1], -q[2], q[3]]
}

fn axis_angle(axis: [f64; 3], angle: f64) -> [f64; 4] {
    let (s, c) = (angle * 0.5).sin_cos();
    [axis[0] * s, axis[1] * s, axis[2] * s, c]
}

/// `v` turned by the unit quaternion `q`.
fn rotate(q: [f64; 4], v: [f64; 3]) -> [f64; 3] {
    let u = [q[0], q[1], q[2]];
    let t = cross(u, v).map(|c| 2.0 * c);
    let ut = cross(u, t);
    std::array::from_fn(|i| v[i] + q[3] * t[i] + ut[i])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: [f64; 3], b: [f64; 3]) -> bool {
        (0..3).all(|i| (a[i] - b[i]).abs() < 1e-12)
    }

    fn apply(m: [[f64; 4]; 4], p: [f64; 3]) -> [f64; 3] {
        std::array::from_fn(|r| m[0][r] * p[0] + m[1][r] * p[1] + m[2][r] * p[2] + m[3][r])
    }

    /// A quarter turn about Z.
    fn quarter() -> [f64; 4] {
        axis_angle([0.0, 0.0, 1.0], std::f64::consts::FRAC_PI_2)
    }

    #[test]
    fn full_weight_is_the_matrix_and_none_is_still() {
        let m = WeightedMove::new([1.0, 2.0, 3.0], quarter(), [2.0, 1.0, 1.0], [0.5, 0.0, 0.0])
            .unwrap();
        for p in [[0.0; 3], [1.0, 0.0, 0.0], [0.3, -2.0, 4.0]] {
            assert!(close(m.at(p, 1.0), apply(m.matrix(), p)), "{p:?}");
            assert_eq!(m.at(p, 0.0), p);
        }
        // A quarter turn about the pivot, then the translation.
        assert!(close(m.at([1.5, 0.0, 0.0], 1.0), [1.5, 2.0 + 2.0, 3.0]));
    }

    #[test]
    fn half_weight_goes_half_way() {
        let m = WeightedMove::new([2.0, 0.0, 0.0], quarter(), [1.0; 3], [0.0; 3]).unwrap();
        // Half the translation and an eighth turn.
        let eighth = std::f64::consts::FRAC_PI_4;
        let q = m.at([1.0, 0.0, 0.0], 0.5);
        assert!(close(q, [1.0 + eighth.cos(), eighth.sin(), 0.0]));
        assert!((m.angle() - std::f64::consts::FRAC_PI_2).abs() < 1e-12);
    }

    #[test]
    fn a_rotation_and_its_negation_move_alike() {
        let q = quarter();
        let a = WeightedMove::new([0.0; 3], q, [1.0; 3], [0.0; 3]).unwrap();
        let b = WeightedMove::new([0.0; 3], q.map(|c| -c), [1.0; 3], [0.0; 3]).unwrap();
        assert!(close(
            a.at([1.0, 2.0, 3.0], 0.3),
            b.at([1.0, 2.0, 3.0], 0.3)
        ));
        assert!(WeightedMove::new([f64::NAN, 0.0, 0.0], q, [1.0; 3], [0.0; 3]).is_none());
        assert!(WeightedMove::new([0.0; 3], [0.0; 4], [1.0; 3], [0.0; 3]).is_none());
    }

    #[test]
    fn mirrored_sides_move_as_images_and_the_plane_holds() {
        let mirror = Mirror::new(Axis::X, 0.0, 1e-6, Side::Positive).unwrap();
        let m = WeightedMove::new([1.0, 0.5, 0.0], [0.0, 0.0, 0.0, 1.0], [1.0; 3], [0.0; 3])
            .unwrap()
            .with_mirror(Some(mirror));
        // The primary side moves by the transform, the other by its image.
        assert_eq!(m.at([2.0, 0.0, 0.0], 1.0), [3.0, 0.5, 0.0]);
        assert_eq!(m.at([-2.0, 0.0, 0.0], 1.0), [-3.0, 0.5, 0.0]);
        // On the plane: moved, then back onto it.
        assert_eq!(m.at([0.0, 0.0, 0.0], 1.0), [0.0, 0.5, 0.0]);
        assert!(Mirror::new(Axis::Y, f64::INFINITY, 0.0, Side::On).is_none());
        assert!(Mirror::new(Axis::Y, 0.0, -1.0, Side::On).is_none());
    }

    #[test]
    fn scale_acts_along_the_frame() {
        // Stretch 2× along the frame's X, the frame turned a quarter about Z:
        // the value's Y.
        let m = WeightedMove::new([0.0; 3], [0.0, 0.0, 0.0, 1.0], [2.0, 1.0, 1.0], [0.0; 3])
            .unwrap()
            .with_frame(quarter())
            .unwrap();
        assert!(close(m.at([0.0, 1.0, 0.0], 1.0), [0.0, 2.0, 0.0]));
        assert!(close(m.at([1.0, 0.0, 0.0], 1.0), [1.0, 0.0, 0.0]));
        assert!(!m.uniform_scale());
    }

    #[test]
    fn the_matrix_at_a_point_moves_it_there() {
        let mirror = Mirror::new(Axis::X, 0.25, 1e-6, Side::Positive).unwrap();
        let m = WeightedMove::new(
            [1.0, 0.5, -0.2],
            quarter(),
            [2.0, 1.0, 0.5],
            [0.5, 0.1, 0.0],
        )
        .unwrap()
        .with_frame(axis_angle([1.0, 0.0, 0.0], 0.3))
        .unwrap()
        .with_mirror(Some(mirror));
        // Off the plane, both sides, at several weights.
        for p in [[2.0, 1.0, 0.5], [-1.5, 0.3, 2.0], [0.9, -2.0, 0.0]] {
            for w in [0.0, 0.25, 1.0] {
                assert!(close(m.at(p, w), apply(m.matrix_at(p, w), p)), "{p:?} {w}");
            }
        }
        // Weight 0 is the identity; the primary side at weight 1 is the move.
        assert_eq!(m.matrix_at([3.0, 0.0, 0.0], -1.0), affine(|q| q));
        assert_eq!(m.matrix_at([3.0, 0.0, 0.0], 1.0), m.matrix());
        // The mirrored side's matrix carries a normal as the image's.
        let n = [1.0, 0.0, 0.0];
        let image = m.matrix_at([-3.0, 0.0, 0.0], 1.0);
        let carried: [f64; 3] = std::array::from_fn(|r| (0..3).map(|c| image[c][r] * n[c]).sum());
        let primary = m.matrix();
        let direct: [f64; 3] = std::array::from_fn(|r| primary[0][r]);
        assert!(close(carried, [direct[0], -direct[1], -direct[2]]));
    }
}
