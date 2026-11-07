//! The standard names of an image's metadata, its Detail attributes, beside
//! whatever a file carries.

/// Its pixels are meant to be seen as pixels (pixel art): a host magnifying
/// it samples the nearest texel rather than blending. An `i32` 1.
pub const PIXELATED: &str = "pixelated";
