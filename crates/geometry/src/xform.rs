//! [`Xform`]: the one placement, and [`Projective`]: a view's 4×4.
use crate::vec3::Vec3;
use bytemuck::{Pod, Zeroable};

/// How far `RᵀR` may stray from the identity for a placement to be
/// rigid: rotations composed from angles, or read from a file's rounded
/// decimals, stay within it; a scale or a shear does not. A structural
/// check, not a modeling tolerance.
const RIGID_SLACK: f64 = 1e-9;

/// An affine placement, double precision: every placement the SDK
/// exchanges (an object's, an item a file reads, a B-rep body's instance,
/// a render prim's, a piece's copies, a move).
///
/// **One convention.** It acts on column vectors: a point `p` goes to
/// `M·p`, `M` being three rows `[a b c | t]`, so the first three columns of
/// the rows are the images of X, Y and Z and the last is the translation.
///
/// It has the layout of `[[f64; 4]; 3]` (`Pod`), so a column of placements
/// is plain data. A maths library's or a GPU's own layout is made from it
/// only where it is used ([`to_cols`](Self::to_cols), the column-major
/// 4×4 a maths library converts from).
///
/// ```
/// use kinogaki_geometry::{Vec3, Xform};
///
/// // 90° about Z, raised 5 along it.
/// let placed = Xform::from_axes(Vec3::Y, -Vec3::X, Vec3::Z, Vec3::new(0.0, 0.0, 5.0));
/// assert_eq!(placed.apply_point(Vec3::X), Vec3::new(0.0, 1.0, 5.0));
/// assert_eq!(placed.inverse().unwrap().apply_point(Vec3::new(0.0, 1.0, 5.0)), Vec3::X);
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
#[repr(transparent)]
pub struct Xform([[f64; 4]; 3]);

impl Default for Xform {
    /// [`Xform::IDENTITY`].
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Xform {
    /// The placement that changes nothing.
    pub const IDENTITY: Xform = Xform([
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
    ]);

    /// The placement of rows `[a b c | t]`.
    pub const fn from_rows(rows: [[f64; 4]; 3]) -> Self {
        Self(rows)
    }

    /// The placement taking X, Y and Z to `x`, `y` and `z` and the origin
    /// to `origin`.
    pub const fn from_axes(x: Vec3, y: Vec3, z: Vec3, origin: Vec3) -> Self {
        Self([
            [x.x, y.x, z.x, origin.x],
            [x.y, y.y, z.y, origin.y],
            [x.z, y.z, z.z, origin.z],
        ])
    }

    /// A pure translation by `t`.
    pub const fn translation(t: Vec3) -> Self {
        Self::from_axes(Vec3::X, Vec3::Y, Vec3::Z, t)
    }

    /// The placement of a column-major 4×4 matrix (`cols[3]` the
    /// translation; its last row is taken as `[0 0 0 1]`): what a maths
    /// library's matrix converts from.
    pub fn from_cols(cols: [[f64; 4]; 4]) -> Self {
        Self(std::array::from_fn(|r| std::array::from_fn(|c| cols[c][r])))
    }

    /// The rows `[a b c | t]`.
    pub const fn rows(&self) -> &[[f64; 4]; 3] {
        &self.0
    }

    /// The column-major 4×4 matrix, last row `[0 0 0 1]`: what a maths
    /// library's matrix converts to.
    pub fn to_cols(&self) -> [[f64; 4]; 4] {
        let r = &self.0;
        std::array::from_fn(|c| [r[0][c], r[1][c], r[2][c], if c == 3 { 1.0 } else { 0.0 }])
    }

    /// The images of X, Y and Z (the linear part's columns).
    pub fn axes(&self) -> [Vec3; 3] {
        let r = &self.0;
        std::array::from_fn(|c| Vec3::new(r[0][c], r[1][c], r[2][c]))
    }

    /// Where the origin goes.
    pub fn origin(&self) -> Vec3 {
        let r = &self.0;
        Vec3::new(r[0][3], r[1][3], r[2][3])
    }

    /// Where point `p` goes: `M·p`.
    pub fn apply_point(&self, p: Vec3) -> Vec3 {
        let r = &self.0;
        let row = |i: usize| r[i][0] * p.x + r[i][1] * p.y + r[i][2] * p.z + r[i][3];
        Vec3::new(row(0), row(1), row(2))
    }

    /// Where direction `v` goes: the linear part alone.
    pub fn apply_vector(&self, v: Vec3) -> Vec3 {
        let r = &self.0;
        let row = |i: usize| r[i][0] * v.x + r[i][1] * v.y + r[i][2] * v.z;
        Vec3::new(row(0), row(1), row(2))
    }

    /// The placement undoing it; `None` when its linear part is singular
    /// (or the inverse isn't finite).
    pub fn inverse(&self) -> Option<Xform> {
        let [x, y, z] = self.axes();
        let cofactors = [y.cross(z), z.cross(x), x.cross(y)];
        let det = x.dot(cofactors[0]);
        if det == 0.0 || !det.is_finite() {
            return None;
        }
        // The inverse linear part's rows are the cofactors over det.
        let rows = cofactors.map(|c| c / det);
        let t = self.origin();
        let inverse = Xform(rows.map(|row| [row.x, row.y, row.z, -row.dot(t)]));
        inverse.is_finite().then_some(inverse)
    }

    /// Whether it is a rotation and a translation: `RᵀR` within 1e-9 of
    /// the identity in each entry and `det R > 0` (no scale, shear or
    /// mirror), every entry finite.
    pub(crate) fn is_rigid(&self) -> bool {
        let axes = self.axes();
        let orthonormal = (0..3).all(|i| {
            (0..3).all(|j| {
                let want = if i == j { 1.0 } else { 0.0 };
                (axes[i].dot(axes[j]) - want).abs() <= RIGID_SLACK
            })
        });
        self.is_finite() && orthonormal && axes[0].dot(axes[1].cross(axes[2])) > 0.0
    }

    /// Whether every entry is finite.
    pub(crate) fn is_finite(&self) -> bool {
        self.0.iter().flatten().all(|v| v.is_finite())
    }
}

/// A 4×4 projective matrix, double precision, column-major (`cols[3]` the
/// translation), acting on column vectors: a view's clip space from an
/// object's space, or back. A placement is an [`Xform`]; this is only for
/// what a projection does.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Projective {
    /// The four columns.
    pub cols: [[f64; 4]; 4],
}

impl Default for Projective {
    /// [`Projective::IDENTITY`].
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Projective {
    /// The identity.
    pub const IDENTITY: Projective = Projective {
        cols: [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ],
    };

    /// `v` through the matrix.
    pub fn apply(&self, v: [f64; 4]) -> [f64; 4] {
        let c = &self.cols;
        std::array::from_fn(|i| c[0][i] * v[0] + c[1][i] * v[1] + c[2][i] * v[2] + c[3][i] * v[3])
    }

    /// The point `p` (w = 1) through the matrix, divided by its w.
    pub fn project_point(&self, p: Vec3) -> Vec3 {
        let [x, y, z, w] = self.apply([p.x, p.y, p.z, 1.0]);
        Vec3::new(x / w, y / w, z / w)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sheared() -> Xform {
        Xform::from_rows([
            [2.0, 0.5, 0.0, 1.0],
            [0.0, 1.0, 0.0, -2.0],
            [0.0, 0.25, 3.0, 0.5],
        ])
    }

    #[test]
    fn the_inverse_undoes_and_singular_ones_have_none() {
        let a = sheared();
        let back = a.inverse().unwrap();
        for p in [Vec3::ZERO, Vec3::X, Vec3::new(0.5, -1.0, 2.0)] {
            assert!(
                (back.apply_point(a.apply_point(p)) - p).length() < 1e-12,
                "{p:?}"
            );
        }
        let flat = Xform::from_axes(Vec3::X, Vec3::Y, Vec3::ZERO, Vec3::ZERO);
        assert_eq!(flat.inverse(), None);
    }

    #[test]
    fn layouts_convert_by_name() {
        let a = sheared();
        assert_eq!(Xform::from_cols(a.to_cols()), a);
        assert_eq!(a.to_cols()[3], [1.0, -2.0, 0.5, 1.0]);
        assert_eq!(a.axes()[1], Vec3::new(0.5, 1.0, 0.25));
        assert_eq!(a.origin(), Vec3::new(1.0, -2.0, 0.5));
        assert_eq!(a.apply_vector(Vec3::X), Vec3::new(2.0, 0.0, 0.0));
    }

    #[test]
    fn only_rotations_and_translations_are_rigid() {
        let raised = Xform::translation(Vec3::new(0.0, 0.0, 5.0));
        assert!(raised.is_rigid());
        assert!(!sheared().is_rigid());
        let mirror = Xform::from_axes(-Vec3::X, Vec3::Y, Vec3::Z, Vec3::ZERO);
        assert!(!mirror.is_rigid());
        let nan = Xform::translation(Vec3::new(f64::NAN, 0.0, 0.0));
        assert!(!nan.is_rigid() && !nan.is_finite());
    }

    #[test]
    fn a_projection_divides_by_w() {
        let clip = Projective {
            cols: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 2.0, 0.0, 0.0],
                [0.0, 0.0, 0.5, 1.0],
                [0.0, 0.0, 0.25, 0.0],
            ],
        };
        let p = Vec3::new(0.3, 0.2, 1.5);
        assert_eq!(clip.apply([p.x, p.y, p.z, 1.0]), [0.3, 0.4, 1.0, 1.5]);
        let projected = clip.project_point(p);
        assert!((projected - Vec3::new(0.2, 0.4 / 1.5, 1.0 / 1.5)).length() < 1e-15);
        assert_eq!(Projective::default().project_point(p), p);
    }
}
