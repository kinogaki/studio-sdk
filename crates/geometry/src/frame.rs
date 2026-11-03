//! [`Frame`]: an origin and right-handed orthonormal axes, checked, and
//! the unit and finite checks it shares with the B-rep carriers.
use crate::Error;
use crate::limits::MAX_COORDINATE;
use crate::vec3::Vec3;
use crate::xform::Xform;

/// How far a stored unit vector's squared length, or a frame's dot
/// products, may stray from exact: the rounding of normalizing a vector
/// once (16 ulps), never a modeling tolerance.
pub(crate) const UNIT_SLACK: f64 = 16.0 * f64::EPSILON;

/// Whether every coordinate of `v` is within the coordinate limit.
pub(crate) fn finite<const N: usize>(v: [f64; N]) -> bool {
    v.iter().all(|c| c.abs() <= MAX_COORDINATE)
}

pub(crate) fn length_squared<const N: usize>(v: [f64; N]) -> f64 {
    v.iter().map(|c| c * c).sum()
}

/// Whether `v` is finite and of unit length, within [`UNIT_SLACK`].
pub(crate) fn is_unit<const N: usize>(v: [f64; N]) -> bool {
    finite(v) && (length_squared(v) - 1.0).abs() <= UNIT_SLACK
}

/// A right-handed orthonormal frame: an origin and unit axes with
/// `z = x × y`, each within the rounding of one normalization (16 ulps).
/// Analytic curves and surfaces are placed by frames, and a view looks
/// squarely at one. As a placement it is the rigid [`Xform`] taking the
/// world axes to its own ([`xform`](Self::xform)).
///
/// ```
/// use kinogaki_geometry::{Frame, Vec3};
///
/// let raised = Frame::new(Vec3::new(0.0, 0.0, 5.0), Vec3::Y, -Vec3::X, Vec3::Z).unwrap();
/// assert_eq!(raised.xform().apply_point(Vec3::X), Vec3::new(0.0, 1.0, 5.0));
/// let left = Frame::new(Vec3::ZERO, Vec3::X, Vec3::Y, -Vec3::Z);
/// assert!(left.is_err());
/// ```
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    origin: Vec3,
    x: Vec3,
    y: Vec3,
    z: Vec3,
}

impl Frame {
    /// The world frame: at the origin, along the world axes.
    pub const WORLD: Frame = Frame {
        origin: Vec3::ZERO,
        x: Vec3::X,
        y: Vec3::Y,
        z: Vec3::Z,
    };

    /// The frame at `origin` with axes `x`, `y`, `z`, checked.
    ///
    /// # Errors
    /// A point not finite, or axes not orthonormal and right-handed.
    pub fn new(origin: Vec3, x: Vec3, y: Vec3, z: Vec3) -> Result<Self, Error> {
        let off = x.cross(y) - z;
        let handed = [off.x, off.y, off.z].iter().all(|c| c.abs() <= UNIT_SLACK);
        let [o, x_, y_, z_] = [origin, x, y, z].map(Vec3::to_array);
        if !(finite(o) && is_unit(x_) && is_unit(y_) && is_unit(z_))
            || x.dot(y).abs() > UNIT_SLACK
            || !handed
        {
            return Err(Error::invalid(
                "a frame's axes are not orthonormal and right-handed",
            ));
        }
        Ok(Self { origin, x, y, z })
    }

    /// The origin.
    pub fn origin(&self) -> Vec3 {
        self.origin
    }

    /// The x axis.
    pub fn x(&self) -> Vec3 {
        self.x
    }

    /// The y axis.
    pub fn y(&self) -> Vec3 {
        self.y
    }

    /// The z axis (`x × y`).
    pub fn z(&self) -> Vec3 {
        self.z
    }

    /// The rigid placement taking the world frame to this one.
    pub fn xform(&self) -> Xform {
        Xform::from_axes(self.x, self.y, self.z, self.origin)
    }
}
