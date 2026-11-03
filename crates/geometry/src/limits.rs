//! The product limits: the largest values Studio and every kernel accept.
//! Builders refuse anything larger ([`ErrorKind::Limit`]), so every
//! element id fits a `u32` and `u32::MAX` stays free as "no element" (an
//! indexed text's missing value, an open primitive's last vertex). Memory
//! is the budget's to bound, not the limits'.
//!
//! [`ErrorKind::Limit`]: crate::ErrorKind::Limit
use crate::error::{ErrorKind, GeometryError};

/// The most points a value holds (2³² − 2).
pub const MAX_POINTS: usize = u32::MAX as usize - 1;
/// The most primitives (polygons, polylines and curves) a value holds
/// (2³² − 2).
pub const MAX_PRIMITIVES: usize = u32::MAX as usize - 1;
/// The most vertices (corners of every primitive together) a value holds
/// (2³² − 2).
pub const MAX_VERTICES: usize = u32::MAX as usize - 1;
/// The longest attribute or group name, in bytes.
pub const MAX_NAME: usize = 256;
/// The highest curve degree, of [`Geometry`](crate::Geometry) curves and of B-rep
/// NURBS ([`crate::brep`]) alike.
pub const MAX_DEGREE: u8 = 25;
/// The largest position component's magnitude.
pub const MAX_COORDINATE: f64 = 1e100;

/// `Err` naming `what` when `count` is over `limit`.
pub(crate) fn check(count: usize, limit: usize, what: &str) -> Result<(), GeometryError> {
    if count > limit {
        return Err(GeometryError::whole(
            ErrorKind::Limit,
            format!("{count} {what} is over the limit of {limit}"),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_count_past_its_limit_is_refused() {
        for limit in [MAX_POINTS, MAX_PRIMITIVES, MAX_VERTICES] {
            assert!(check(limit, limit, "elements").is_ok());
            let e = check(limit + 1, limit, "elements").unwrap_err();
            assert_eq!(e.kind(), ErrorKind::Limit);
        }
    }
}
