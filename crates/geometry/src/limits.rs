//! The product limits: the largest values Studio and every kernel accept.
//! Builders refuse anything larger ([`ErrorKind::Limit`]), so a count of
//! any element fits a `u32` with room to spare, and memory stays bounded.
//!
//! [`ErrorKind::Limit`]: crate::ErrorKind::Limit
use crate::error::{ErrorKind, GeometryError};

/// The most points a value holds (2²³).
pub const MAX_POINTS: usize = 8_388_608;
/// The most primitives (polygons and polylines, or curves) a value holds
/// (2²³).
pub const MAX_PRIMITIVES: usize = 8_388_608;
/// The most vertices (corners of every primitive together) a value holds
/// (2²⁵).
pub const MAX_VERTICES: usize = 33_554_432;
/// The longest attribute or group name, in bytes.
pub const MAX_NAME: usize = 256;
/// The highest curve degree.
pub const MAX_DEGREE: u8 = 15;
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
