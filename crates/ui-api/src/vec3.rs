//! Arithmetic on the API's plain [`Vec3`], for the viewport maths here.
//! Each is written as Studio's maths library computes it (same operations
//! in the same order), so results agree to the bit.
use kinogaki_studio_api::Vec3;

/// `a + b`.
pub(crate) fn add(a: Vec3, b: Vec3) -> Vec3 {
    Vec3::new(a.x + b.x, a.y + b.y, a.z + b.z)
}

/// `a - b`.
pub(crate) fn sub(a: Vec3, b: Vec3) -> Vec3 {
    Vec3::new(a.x - b.x, a.y - b.y, a.z - b.z)
}

/// `a * s`.
pub(crate) fn scale(a: Vec3, s: f64) -> Vec3 {
    Vec3::new(a.x * s, a.y * s, a.z * s)
}

/// The dot product.
pub(crate) fn dot(a: Vec3, b: Vec3) -> f64 {
    a.x * b.x + a.y * b.y + a.z * b.z
}

/// The length.
#[cfg(test)]
pub(crate) fn length(a: Vec3) -> f64 {
    dot(a, a).sqrt()
}

/// The cross product.
#[cfg(test)]
pub(crate) fn cross(a: Vec3, b: Vec3) -> Vec3 {
    Vec3::new(
        a.y * b.z - b.y * a.z,
        a.z * b.x - b.z * a.x,
        a.x * b.y - b.x * a.y,
    )
}

/// Whether every component is finite.
pub(crate) fn is_finite(a: Vec3) -> bool {
    a.x.is_finite() && a.y.is_finite() && a.z.is_finite()
}
