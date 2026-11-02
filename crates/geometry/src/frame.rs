//! [`Frame`]: an origin and right-handed orthonormal axes, checked, and
//! the unit and finite checks it shares with the B-rep carriers.
use crate::error::{ErrorKind, GeometryError};
use crate::limits::MAX_COORDINATE;
use crate::vector::cross;

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
/// squarely at one.
///
/// ```
/// use kinogaki_geometry::Frame;
///
/// let raised = Frame::new([0.0, 0.0, 5.0], [0.0, 1.0, 0.0], [-1.0, 0.0, 0.0], [0.0, 0.0, 1.0]);
/// assert!(raised.is_ok());
/// let left = Frame::new([0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, -1.0]);
/// assert!(left.is_err());
/// ```
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    origin: [f64; 3],
    x: [f64; 3],
    y: [f64; 3],
    z: [f64; 3],
}

impl Frame {
    /// The world frame: at the origin, along the world axes.
    pub const WORLD: Frame = Frame {
        origin: [0.0; 3],
        x: [1.0, 0.0, 0.0],
        y: [0.0, 1.0, 0.0],
        z: [0.0, 0.0, 1.0],
    };

    /// The frame at `origin` with axes `x`, `y`, `z`, checked.
    ///
    /// # Errors
    /// A point not finite, or axes not orthonormal and right-handed.
    pub fn new(
        origin: [f64; 3],
        x: [f64; 3],
        y: [f64; 3],
        z: [f64; 3],
    ) -> Result<Self, GeometryError> {
        let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
        let cross = cross(x, y);
        let handed = (0..3).all(|i| (cross[i] - z[i]).abs() <= UNIT_SLACK);
        if !(finite(origin) && is_unit(x) && is_unit(y) && is_unit(z))
            || dot(x, y).abs() > UNIT_SLACK
            || !handed
        {
            return Err(GeometryError::whole(
                ErrorKind::Frame,
                "a frame's axes are not orthonormal and right-handed",
            ));
        }
        Ok(Self { origin, x, y, z })
    }

    /// The origin.
    pub fn origin(&self) -> [f64; 3] {
        self.origin
    }

    /// The x axis.
    pub fn x(&self) -> [f64; 3] {
        self.x
    }

    /// The y axis.
    pub fn y(&self) -> [f64; 3] {
        self.y
    }

    /// The z axis (`x × y`).
    pub fn z(&self) -> [f64; 3] {
        self.z
    }
}
