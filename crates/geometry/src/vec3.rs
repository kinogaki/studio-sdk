//! [`Vec3`]: the one 3D vector, and [`Ray`].
use bytemuck::{Pod, Zeroable};
use std::ops::{Add, AddAssign, Div, Mul, Neg, Sub, SubAssign};

/// A point or a direction in 3D, double precision: every vector the SDK
/// exchanges. It has the layout of `[f64; 3]` (`Pod`), so a column of
/// positions views as vectors at no cost.
///
/// Each operation is written as Studio's maths library computes it (the
/// same operations in the same order), so results agree to the bit.
///
/// ```
/// use kinogaki_geometry::Vec3;
///
/// let x = Vec3::new(1.0, 0.0, 0.0);
/// let y = Vec3::new(0.0, 1.0, 0.0);
/// assert_eq!(x.cross(y), Vec3::new(0.0, 0.0, 1.0));
/// assert_eq!((x + y) * 2.0, Vec3::new(2.0, 2.0, 0.0));
/// assert_eq!(Vec3::from([1.0, 2.0, 3.0]).to_array(), [1.0, 2.0, 3.0]);
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
#[repr(C)]
pub struct Vec3 {
    /// X.
    pub x: f64,
    /// Y.
    pub y: f64,
    /// Z.
    pub z: f64,
}

impl Vec3 {
    /// The origin, or no direction.
    pub const ZERO: Vec3 = Vec3::new(0.0, 0.0, 0.0);
    /// The unit X axis.
    pub const X: Vec3 = Vec3::new(1.0, 0.0, 0.0);
    /// The unit Y axis.
    pub const Y: Vec3 = Vec3::new(0.0, 1.0, 0.0);
    /// The unit Z axis.
    pub const Z: Vec3 = Vec3::new(0.0, 0.0, 1.0);

    /// The vector (`x`, `y`, `z`).
    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    /// `[x, y, z]`.
    pub const fn to_array(self) -> [f64; 3] {
        [self.x, self.y, self.z]
    }

    /// `[x, y, z]` at single precision, for an upload.
    pub fn to_f32(self) -> [f32; 3] {
        [self.x as f32, self.y as f32, self.z as f32]
    }

    /// The dot product.
    pub fn dot(self, other: Vec3) -> f64 {
        self.x * other.x + self.y * other.y + self.z * other.z
    }

    /// The cross product (right-handed).
    pub fn cross(self, other: Vec3) -> Vec3 {
        Vec3::new(
            self.y * other.z - other.y * self.z,
            self.z * other.x - other.z * self.x,
            self.x * other.y - other.x * self.y,
        )
    }

    /// The length.
    pub fn length(self) -> f64 {
        self.dot(self).sqrt()
    }

    /// The vector at unit length; `None` when it has no direction or isn't
    /// finite.
    pub fn normalized(self) -> Option<Vec3> {
        let length = self.length();
        (length > 0.0 && length.is_finite()).then(|| self * (1.0 / length))
    }

    /// The point `t` of the way from `self` to `other` (0 is `self`, 1
    /// `other`).
    pub fn lerp(self, other: Vec3, t: f64) -> Vec3 {
        self + (other - self) * t
    }

    /// The smaller of each component.
    pub fn min(self, other: Vec3) -> Vec3 {
        Vec3::new(
            self.x.min(other.x),
            self.y.min(other.y),
            self.z.min(other.z),
        )
    }

    /// The larger of each component.
    pub fn max(self, other: Vec3) -> Vec3 {
        Vec3::new(
            self.x.max(other.x),
            self.y.max(other.y),
            self.z.max(other.z),
        )
    }

    /// Whether every component is finite.
    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite() && self.z.is_finite()
    }
}

impl From<[f64; 3]> for Vec3 {
    fn from([x, y, z]: [f64; 3]) -> Self {
        Self { x, y, z }
    }
}

impl From<Vec3> for [f64; 3] {
    fn from(v: Vec3) -> Self {
        v.to_array()
    }
}

impl Add for Vec3 {
    type Output = Vec3;
    fn add(self, b: Vec3) -> Vec3 {
        Vec3::new(self.x + b.x, self.y + b.y, self.z + b.z)
    }
}

impl Sub for Vec3 {
    type Output = Vec3;
    fn sub(self, b: Vec3) -> Vec3 {
        Vec3::new(self.x - b.x, self.y - b.y, self.z - b.z)
    }
}

impl Mul<f64> for Vec3 {
    type Output = Vec3;
    fn mul(self, s: f64) -> Vec3 {
        Vec3::new(self.x * s, self.y * s, self.z * s)
    }
}

impl Div<f64> for Vec3 {
    type Output = Vec3;
    fn div(self, s: f64) -> Vec3 {
        Vec3::new(self.x / s, self.y / s, self.z / s)
    }
}

impl Neg for Vec3 {
    type Output = Vec3;
    fn neg(self) -> Vec3 {
        Vec3::new(-self.x, -self.y, -self.z)
    }
}

impl AddAssign for Vec3 {
    fn add_assign(&mut self, b: Vec3) {
        *self = *self + b;
    }
}

impl SubAssign for Vec3 {
    fn sub_assign(&mut self, b: Vec3) {
        *self = *self - b;
    }
}

/// A ray: from `origin` along `direction` (not necessarily of unit
/// length; a view's pick ray is).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ray {
    /// Where it starts.
    pub origin: Vec3,
    /// Where it goes.
    pub direction: Vec3,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arithmetic_and_products() {
        let a = Vec3::new(1.0, 2.0, 3.0);
        let b = Vec3::new(-2.0, 0.5, 4.0);
        assert_eq!(a + b - b, a);
        assert_eq!(-a * 2.0 / 2.0, Vec3::new(-1.0, -2.0, -3.0));
        assert_eq!(a.dot(b), 11.0);
        assert_eq!(Vec3::Y.cross(Vec3::Z), Vec3::X);
        assert_eq!(a.cross(b).dot(a), 0.0);
        let unit = Vec3::new(3.0, 0.0, 4.0).normalized().unwrap();
        assert!((unit - Vec3::new(0.6, 0.0, 0.8)).length() < 1e-15);
        assert_eq!(Vec3::ZERO.normalized(), None);
        assert_eq!(a.min(b), Vec3::new(-2.0, 0.5, 3.0));
        assert_eq!(a.max(b), Vec3::new(1.0, 2.0, 4.0));
        assert_eq!(Vec3::ZERO.lerp(b, 0.5), Vec3::new(-1.0, 0.25, 2.0));
        assert!(!Vec3::new(f64::NAN, 0.0, 0.0).is_finite());
    }

    #[test]
    fn a_column_of_arrays_views_as_vectors() {
        let points = [[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]];
        let vectors: &[Vec3] = bytemuck::cast_slice(&points);
        assert_eq!(vectors[1], Vec3::new(4.0, 5.0, 6.0));
    }
}
