//! How a placement acts on attribute values, by their [`Meaning`]: what a
//! [`gather`](super::gather) does to each input and what
//! [`place_points`](super::Editor::place_points) and
//! [`place_primitives`](super::Editor::place_primitives) ask for. Points move,
//! vectors and matrices follow, normals take the inverse transpose, rotations
//! compose; a reflection mirrors a rotation (the same turn seen in a mirror).
use kinogaki_geometry::{Attribute, AttributeType, Column, Error, Meaning, Values, Vec3, Xform};
use std::ops::Range;

/// Whether a placement changes values of `ty`.
pub(super) fn turns(ty: AttributeType) -> bool {
    !ty.array
        && matches!(
            ty.meaning,
            Meaning::Point
                | Meaning::Vector
                | Meaning::Normal
                | Meaning::Rotation
                | Meaning::Transform
        )
}

/// `attribute` with the items in `items` placed by `at`, the rest shared.
pub(super) fn placed(
    attribute: &Attribute,
    at: Xform,
    items: impl Iterator<Item = Range<u32>>,
) -> Result<Attribute, Error> {
    let ty = attribute.ty();
    if !turns(ty) {
        return Ok(attribute.clone());
    }
    let placing = Placing::new(ty.meaning, at);
    let width = usize::from(ty.width);
    fn write<T: Copy>(
        column: &Column<T>,
        width: usize,
        placing: &Placing,
        items: impl Iterator<Item = Range<u32>>,
        to: impl Fn(T) -> f64,
        from: impl Fn(f64) -> T,
    ) -> Column<T> {
        let mut out = column.edit();
        let mut item = vec![0.0; width];
        for k in items.flatten() {
            let cells = k as usize * width;
            (0..width).for_each(|i| item[i] = to(out[cells + i]));
            placing.apply(&mut item);
            (0..width).for_each(|i| out[cells + i] = from(item[i]));
        }
        out.finish()
    }
    let values = match attribute.values() {
        Values::F32(c) => Values::F32(write(c, width, &placing, items, f64::from, |x| x as f32)),
        Values::F64(c) => Values::F64(write(c, width, &placing, items, |x| x, |x| x)),
        _ => return Ok(attribute.clone()),
    };
    attribute.with_values(values)
}

/// How one placement acts on the values of one meaning.
pub(super) enum Placing {
    Point(Xform),
    Vector([[f64; 3]; 3]),
    Normal([[f64; 3]; 3]),
    /// Composes with the turn: `turn` after the rotation.
    Rotation([f64; 4]),
    /// A reflection: the rotation seen in the mirror, `turn` on both sides.
    Mirrored([f64; 4]),
    Transform(Xform),
}

impl Placing {
    pub(super) fn new(meaning: Meaning, at: Xform) -> Self {
        let linear = |x: &Xform| {
            let r = x.rows();
            [
                [r[0][0], r[0][1], r[0][2]],
                [r[1][0], r[1][1], r[1][2]],
                [r[2][0], r[2][1], r[2][2]],
            ]
        };
        match meaning {
            Meaning::Point => Self::Point(at),
            Meaning::Normal => {
                // The inverse's transpose: rows of the inverse read as columns.
                let inverse = at.inverse().map_or_else(|| linear(&at), |i| linear(&i));
                Self::Normal(std::array::from_fn(|i| {
                    std::array::from_fn(|j| inverse[j][i])
                }))
            }
            Meaning::Rotation => Self::rotation(&at),
            Meaning::Transform => Self::Transform(at),
            _ => Self::Vector(linear(&at)),
        }
    }

    /// How `at` acts on rotations. A mirror is the half turn about its normal
    /// with every direction reversed, which a rotation doesn't see; what is
    /// left is that half turn, on both sides: a mirror across X keeps a turn
    /// about X and reverses those about Y and Z.
    fn rotation(at: &Xform) -> Self {
        let [x, y, z] = at.axes();
        if x.cross(y).dot(z) >= 0.0 {
            return Self::Rotation(rotation_of([x, y, z]));
        }
        Self::Mirrored(rotation_of([x * -1.0, y * -1.0, z * -1.0]))
    }

    pub(super) fn apply(&self, item: &mut [f64]) {
        let turned = |m: &[[f64; 3]; 3], v: &[f64]| -> [f64; 3] {
            std::array::from_fn(|i| m[i][0] * v[0] + m[i][1] * v[1] + m[i][2] * v[2])
        };
        match self {
            Self::Point(at) => {
                let p = at
                    .apply_point([item[0], item[1], item[2]].into())
                    .to_array();
                item.copy_from_slice(&p);
            }
            Self::Vector(m) => item.copy_from_slice(&turned(m, item)),
            Self::Normal(m) => {
                let n = turned(m, item);
                let length = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
                let scale = if length > 0.0 { 1.0 / length } else { 1.0 };
                item.copy_from_slice(&n.map(|c| c * scale));
            }
            Self::Rotation(q) => {
                let q2 = [item[0], item[1], item[2], item[3]];
                item.copy_from_slice(&multiplied(*q, q2));
            }
            Self::Mirrored(q) => {
                let [x, y, z, w] = *q;
                let turned = multiplied(
                    multiplied(*q, [item[0], item[1], item[2], item[3]]),
                    [-x, -y, -z, w],
                );
                item.copy_from_slice(&turned);
            }
            Self::Transform(at) => composed(at, item),
        }
    }
}

/// `m`, row-major of side 3 or 4, after `at`.
fn composed(at: &Xform, m: &mut [f64]) {
    let side = if m.len() == 16 { 4 } else { 3 };
    let rows = at.rows();
    let before = m.to_vec();
    for r in 0..3 {
        for c in 0..side {
            let by = |k: usize| rows[r][k] * before[k * side + c];
            m[r * side + c] =
                by(0) + by(1) + by(2) + if side == 4 && c == 3 { rows[r][3] } else { 0.0 };
        }
    }
}

/// The unit quaternion (x y z w) of the rotation with these axes, their
/// scales taken out.
fn rotation_of(axes: [Vec3; 3]) -> [f64; 4] {
    let [x, y, z] = axes.map(|a| {
        let l = a.length();
        if l > 0.0 { a / l } else { a }
    });
    // Shepperd's method over the matrix whose columns are the axes.
    let (m00, m11, m22) = (x.x, y.y, z.z);
    let trace = m00 + m11 + m22;
    if trace > 0.0 {
        let s = (trace + 1.0).sqrt() * 2.0;
        [(y.z - z.y) / s, (z.x - x.z) / s, (x.y - y.x) / s, 0.25 * s]
    } else if m00 > m11 && m00 > m22 {
        let s = (1.0 + m00 - m11 - m22).sqrt() * 2.0;
        [0.25 * s, (y.x + x.y) / s, (z.x + x.z) / s, (y.z - z.y) / s]
    } else if m11 > m22 {
        let s = (1.0 + m11 - m00 - m22).sqrt() * 2.0;
        [(y.x + x.y) / s, 0.25 * s, (z.y + y.z) / s, (z.x - x.z) / s]
    } else {
        let s = (1.0 + m22 - m00 - m11).sqrt() * 2.0;
        [(z.x + x.z) / s, (z.y + y.z) / s, 0.25 * s, (x.y - y.x) / s]
    }
}

/// `a` after `b`, both x y z w.
fn multiplied(a: [f64; 4], b: [f64; 4]) -> [f64; 4] {
    let [ax, ay, az, aw] = a;
    let [bx, by, bz, bw] = b;
    [
        aw * bx + ax * bw + ay * bz - az * by,
        aw * by - ax * bz + ay * bw + az * bx,
        aw * bz + ax * by - ay * bx + az * bw,
        aw * bw - ax * bx - ay * by - az * bz,
    ]
}
