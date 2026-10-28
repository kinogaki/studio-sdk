//! The few geometric values the API exchanges: [`Vec3`], [`Affine3`],
//! [`Mat4`] and [`Bounds`].
//!
//! They are plain `Copy` data with public fields and no algorithm library:
//! a package does its maths with whatever library it likes (Studio's
//! kernels use their own) and converts at the boundary. Positions and
//! lengths are in scene units.

/// A point or direction in 3D, double precision.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec3 {
    /// X.
    pub x: f64,
    /// Y.
    pub y: f64,
    /// Z (up in Studio's scenes).
    pub z: f64,
}

impl Vec3 {
    /// The origin.
    pub const ZERO: Vec3 = Vec3::new(0.0, 0.0, 0.0);

    /// The vector (`x`, `y`, `z`).
    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    /// The vector of `[x, y, z]`.
    pub const fn from_array([x, y, z]: [f64; 3]) -> Self {
        Self { x, y, z }
    }

    /// `[x, y, z]`.
    pub const fn to_array(self) -> [f64; 3] {
        [self.x, self.y, self.z]
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
}

/// An affine placement `p ↦ M p + t`: a general 3×3 linear part (rotation,
/// scale, shear, reflection) and a translation, double precision. Items a
/// format reads are placed with it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Affine3 {
    /// The linear part as three columns: the images of X, Y and Z.
    pub linear: [Vec3; 3],
    /// The translation.
    pub translation: Vec3,
}

impl Affine3 {
    /// The placement that changes nothing.
    pub const IDENTITY: Affine3 = Affine3 {
        linear: [
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
        ],
        translation: Vec3::ZERO,
    };

    /// A pure translation by `t`.
    pub const fn translation(t: Vec3) -> Self {
        Self {
            translation: t,
            ..Self::IDENTITY
        }
    }
}

impl Default for Affine3 {
    /// [`Affine3::IDENTITY`].
    fn default() -> Self {
        Self::IDENTITY
    }
}

/// A 4×4 matrix of `f32`, column-major (`cols[3]` is the translation):
/// what render engines read. Render items place their copies with it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mat4 {
    /// The four columns.
    pub cols: [[f32; 4]; 4],
}

impl Mat4 {
    /// The identity matrix.
    pub const IDENTITY: Mat4 = Mat4 {
        cols: [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ],
    };

    /// A pure translation by (`x`, `y`, `z`).
    pub const fn translation(x: f32, y: f32, z: f32) -> Self {
        let mut m = Self::IDENTITY;
        m.cols[3] = [x, y, z, 1.0];
        m
    }
}

impl Default for Mat4 {
    /// [`Mat4::IDENTITY`].
    fn default() -> Self {
        Self::IDENTITY
    }
}

/// An axis-aligned box in a value's own space (before its object's
/// placement), for framing, culling and the info panel.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bounds {
    /// The smallest corner.
    pub min: Vec3,
    /// The largest corner.
    pub max: Vec3,
}

impl Bounds {
    /// The box holding only `p`.
    pub const fn point(p: Vec3) -> Self {
        Self { min: p, max: p }
    }

    /// The smallest box holding both `self` and `other`.
    pub fn union(self, other: Bounds) -> Bounds {
        Bounds {
            min: self.min.min(other.min),
            max: self.max.max(other.max),
        }
    }

    /// The box's center.
    pub fn center(self) -> Vec3 {
        Vec3::new(
            (self.min.x + self.max.x) * 0.5,
            (self.min.y + self.max.y) * 0.5,
            (self.min.z + self.max.z) * 0.5,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounds_join_and_center() {
        let a = Bounds {
            min: Vec3::new(0.0, 0.0, 0.0),
            max: Vec3::new(1.0, 1.0, 1.0),
        };
        let b = Bounds {
            min: Vec3::new(-1.0, 0.5, 0.0),
            max: Vec3::new(0.0, 3.0, 1.0),
        };
        let both = a.union(b);
        assert_eq!(both.min, Vec3::new(-1.0, 0.0, 0.0));
        assert_eq!(both.max, Vec3::new(1.0, 3.0, 1.0));
        assert_eq!(both.center(), Vec3::new(0.0, 1.5, 0.5));
        assert_eq!(
            Bounds::point(Vec3::ZERO)
                .union(Bounds::point(Vec3::ZERO))
                .min,
            Vec3::ZERO
        );
    }

    #[test]
    fn identities_and_translations() {
        assert_eq!(Affine3::default(), Affine3::IDENTITY);
        assert_eq!(
            Affine3::translation(Vec3::new(1.0, 2.0, 3.0)).linear,
            Affine3::IDENTITY.linear
        );
        assert_eq!(
            Mat4::translation(1.0, 2.0, 3.0).cols[3],
            [1.0, 2.0, 3.0, 1.0]
        );
        assert_eq!(Mat4::default(), Mat4::IDENTITY);
        assert_eq!(
            Vec3::from_array([1.0, 2.0, 3.0]).to_array(),
            [1.0, 2.0, 3.0]
        );
    }
}
