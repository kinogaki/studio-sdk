//! The standard names of an image's metadata, its Detail attributes, beside
//! whatever a file carries.

/// The primaries of its color layers' space: red, green, blue and white, x
/// then y each, one `[f32; 8]` (OpenEXR's `chromaticities`). A host tags an
/// image with them for a writer, which copies them into the file.
pub const CHROMATICITIES: &str = "chromaticities";

/// Its pixels are meant to be seen as pixels (pixel art): a host magnifying
/// it samples the nearest texel rather than blending. An `i32` 1.
pub const PIXELATED: &str = "pixelated";
