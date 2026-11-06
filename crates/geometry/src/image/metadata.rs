//! The standard names of an image's metadata, its Detail attributes, beside
//! whatever a file carries.

/// The primaries of its color layers' space: red, green, blue and white, x
/// then y each, one `[f32; 8]` (OpenEXR's `chromaticities`). A host tags an
/// image with them for a writer, which copies them into the file.
pub const CHROMATICITIES: &str = "chromaticities";
