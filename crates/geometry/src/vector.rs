//! The few vector operations triangulation and shading need, on plain
//! arrays.

pub(crate) type V3 = [f64; 3];
pub(crate) type V2 = [f64; 2];

pub(crate) fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

pub(crate) fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

pub(crate) fn scale(a: V3, s: f64) -> V3 {
    [a[0] * s, a[1] * s, a[2] * s]
}

pub(crate) fn dot(a: V3, b: V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

pub(crate) fn cross(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

pub(crate) fn length(a: V3) -> f64 {
    dot(a, a).sqrt()
}

/// `a` at unit length, or zero when it has none (or isn't finite).
pub(crate) fn normalize_or_zero(a: V3) -> V3 {
    let l = length(a);
    if l > 0.0 && l.is_finite() {
        scale(a, 1.0 / l)
    } else {
        [0.0; 3]
    }
}

pub(crate) fn to_f32(a: V3) -> [f32; 3] {
    a.map(|c| c as f32)
}

pub(crate) fn sub2(a: V2, b: V2) -> V2 {
    [a[0] - b[0], a[1] - b[1]]
}

pub(crate) fn perp_dot(a: V2, b: V2) -> f64 {
    a[0] * b[1] - a[1] * b[0]
}

pub(crate) fn dot2(a: V2, b: V2) -> f64 {
    a[0] * b[0] + a[1] * b[1]
}

pub(crate) fn length_squared2(a: V2) -> f64 {
    dot2(a, a)
}

/// Twice the signed area of the triangle `a b c` (positive when it turns
/// left).
pub(crate) fn turn(a: V2, b: V2, c: V2) -> f64 {
    perp_dot(sub2(b, a), sub2(c, a))
}
