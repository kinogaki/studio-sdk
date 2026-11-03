//! [`placements`]: where copies or instances go on a value's points, from
//! the standard instance attributes, as Houdini's Copy to Points and
//! instancing place them.
//!
//! Each point's placement is `T(P) · R · S`:
//! - `T(P)`: to the point.
//! - `R`: [`ORIENT`](names::ORIENT) when the point has it (a quaternion,
//!   normalized; zero turns nothing); else the frame aiming +Z along
//!   [`N`](names::N) (or [`V`](names::V) without it) with +Y toward
//!   [`UP`](names::UP) (world +Y without it, +X when the aim is along it);
//!   else no turn.
//! - `S`: [`SCALE`](names::SCALE) per axis times [`PSCALE`](names::PSCALE).
//!
//! A point with [`TRANSFORM`](names::TRANSFORM) is placed at `T(P) · M`
//! by that matrix alone, the other attributes aside.
use crate::attributes::{Domain, names};
use crate::column::Column;
use crate::geometry::Geometry;
use crate::vec3::Vec3;
use crate::xform::Xform;

/// The placement of a copy on each of `points`' points (see the module).
///
/// ```
/// use kinogaki_geometry::{Attribute, Attributes, Geometry, Meaning, Primitives, Vec3, names, placements};
///
/// let points = Primitives::of_polygons(2, vec![0].into(), Vec::new().into()).unwrap();
/// let sizes = Attribute::tuples(vec![[2.0f32], [0.5]], Meaning::None).unwrap();
/// let attributes = Attributes::new().with(kinogaki_geometry::Domain::Point, names::PSCALE, sizes).unwrap();
/// let value = Geometry::new(vec![[0.0; 3], [1.0, 0.0, 0.0]].into(), points, attributes).unwrap();
/// let placed = placements(&value);
/// assert_eq!(placed[0].apply_point(Vec3::X), Vec3::new(2.0, 0.0, 0.0));
/// assert_eq!(placed[1].apply_point(Vec3::X), Vec3::new(1.5, 0.0, 0.0));
/// ```
pub fn placements(points: &Geometry) -> Column<Xform> {
    let a = points.attributes();
    let tuples = |name| a.tuples::<[f32; 3]>(Domain::Point, name);
    let (orient, aim, up) = (
        a.tuples::<[f32; 4]>(Domain::Point, names::ORIENT),
        tuples(names::N).or_else(|| tuples(names::V)),
        tuples(names::UP),
    );
    let (scale, pscale) = (
        tuples(names::SCALE),
        a.tuples::<[f32; 1]>(Domain::Point, names::PSCALE),
    );
    let transform = a.tuples::<[f64; 16]>(Domain::Point, names::TRANSFORM);
    let vec = |v: [f32; 3]| Vec3::new(v[0].into(), v[1].into(), v[2].into());
    (points.positions().iter().enumerate())
        .map(|(p, &at)| {
            let at = Vec3::from(at);
            if let Some(m) = transform.map(|t| t[p]) {
                let row = |r: usize| [m[4 * r], m[4 * r + 1], m[4 * r + 2], m[4 * r + 3]];
                let mut rows = [row(0), row(1), row(2)];
                for (r, t) in rows.iter_mut().zip([at.x, at.y, at.z]) {
                    r[3] += t;
                }
                return Xform::from_rows(rows);
            }
            let [x, y, z] = match (orient, aim) {
                (Some(q), _) => turned(q[p]),
                (None, Some(n)) => aimed(vec(n[p]), up.map_or(Vec3::Y, |u| vec(u[p]))),
                (None, None) => [Vec3::X, Vec3::Y, Vec3::Z],
            };
            let s = scale.map_or(Vec3::new(1.0, 1.0, 1.0), |s| vec(s[p]));
            let k = pscale.map_or(1.0, |s| f64::from(s[p][0]));
            Xform::from_axes(x * (s.x * k), y * (s.y * k), z * (s.z * k), at)
        })
        .collect()
}

/// The axes unit quaternion `q` (x y z w, normalized here) turns X, Y and
/// Z to; zero turns nothing.
fn turned(q: [f32; 4]) -> [Vec3; 3] {
    let [x, y, z, w] = q.map(f64::from);
    let length = (x * x + y * y + z * z + w * w).sqrt();
    if length <= 0.0 || !length.is_finite() {
        return [Vec3::X, Vec3::Y, Vec3::Z];
    }
    let [x, y, z, w] = [x, y, z, w].map(|c| c / length);
    [
        Vec3::new(
            1.0 - 2.0 * (y * y + z * z),
            2.0 * (x * y + w * z),
            2.0 * (x * z - w * y),
        ),
        Vec3::new(
            2.0 * (x * y - w * z),
            1.0 - 2.0 * (x * x + z * z),
            2.0 * (y * z + w * x),
        ),
        Vec3::new(
            2.0 * (x * z + w * y),
            2.0 * (y * z - w * x),
            1.0 - 2.0 * (x * x + y * y),
        ),
    ]
}

/// The frame aiming +Z along `aim` with +Y toward `up` (world +Y, then
/// +X, when `up` lies along the aim); no turn when `aim` is zero.
fn aimed(aim: Vec3, up: Vec3) -> [Vec3; 3] {
    let unit = |v: Vec3| {
        let l = v.length();
        (l > 0.0 && l.is_finite()).then(|| v / l)
    };
    let Some(z) = unit(aim) else {
        return [Vec3::X, Vec3::Y, Vec3::Z];
    };
    let x = ([up, Vec3::Y, Vec3::X].into_iter())
        .find_map(|u| unit(u.cross(z)))
        .unwrap_or(Vec3::X);
    [x, z.cross(x), z]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Attribute, Attributes, Meaning, Primitives};

    fn points(at: Vec<[f64; 3]>, attributes: Attributes) -> Geometry {
        let table = Primitives::of_polygons(at.len(), vec![0].into(), Vec::new().into()).unwrap();
        Geometry::new(at.into(), table, attributes).unwrap()
    }

    fn close(a: Vec3, b: Vec3) -> bool {
        (a - b).length() < 1e-6
    }

    #[test]
    fn placements_follow_the_standard_attributes() {
        let at = vec![[1.0, 2.0, 3.0]; 4];
        // No attributes: at the point, unturned.
        let bare = placements(&points(at.clone(), Attributes::new()));
        assert_eq!(bare[0], Xform::translation(Vec3::new(1.0, 2.0, 3.0)));
        // orient: a quarter turn about Z (not normalized), scaled.
        let half = std::f32::consts::FRAC_1_SQRT_2;
        let q = Attribute::tuples(
            vec![[0.0, 0.0, 2.0 * half, 2.0 * half]; 4],
            Meaning::Rotation,
        );
        let s = Attribute::tuples(vec![[1.0f32, 2.0, 3.0]; 4], Meaning::None);
        let k = Attribute::tuples(vec![[2.0f32]; 4], Meaning::None);
        let a = (Attributes::new().with(Domain::Point, names::ORIENT, q.unwrap()))
            .and_then(|a| a.with(Domain::Point, names::SCALE, s.unwrap()))
            .and_then(|a| a.with(Domain::Point, names::PSCALE, k.unwrap()))
            .unwrap();
        let m = placements(&points(at.clone(), a))[0];
        assert!(close(m.apply_vector(Vec3::X), Vec3::new(0.0, 2.0, 0.0)));
        assert!(close(m.apply_vector(Vec3::Y), Vec3::new(-4.0, 0.0, 0.0)));
        assert!(close(m.origin(), Vec3::new(1.0, 2.0, 3.0)));
        // N aims +Z, up turns +Y; up along N still makes a frame.
        let n = Attribute::tuples(vec![[1.0f32, 0.0, 0.0]; 4], Meaning::Normal).unwrap();
        let a = Attributes::new().with(Domain::Point, names::N, n).unwrap();
        let m = placements(&points(at.clone(), a))[0];
        assert!(close(m.apply_vector(Vec3::Z), Vec3::X) && close(m.apply_vector(Vec3::Y), Vec3::Y));
        let n = Attribute::tuples(vec![[0.0f32, 1.0, 0.0]; 4], Meaning::Normal).unwrap();
        let a = Attributes::new().with(Domain::Point, names::N, n).unwrap();
        let m = placements(&points(at.clone(), a))[0];
        let [x, y, z] = m.axes();
        assert!(close(z, Vec3::Y) && x.dot(z).abs() < 1e-9 && y.dot(z).abs() < 1e-9);
        assert!(close(x.cross(y), z), "a right-handed frame");
        // transform: the matrix alone, at the point.
        let mut t = [0.0; 16];
        (t[0], t[5], t[10], t[15], t[3]) = (2.0, 1.0, 1.0, 1.0, 10.0);
        let t = Attribute::tuples(vec![t; 4], Meaning::Transform).unwrap();
        let a = Attributes::new()
            .with(Domain::Point, names::TRANSFORM, t)
            .unwrap();
        let m = placements(&points(at, a))[0];
        assert_eq!(m.apply_point(Vec3::X), Vec3::new(13.0, 2.0, 3.0));
    }
}
