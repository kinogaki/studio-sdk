//! MaterialX documents' materials as Studio's, and back: what a format
//! reading or writing `.mtlx` documents, or files holding them, converts at
//! its boundary. `kinogaki-mtlx` reads and writes the document and flattens
//! its graphs ([`Document::materials`], [`Document::of_materials`]); this
//! module converts between those materials and the SDK's
//! ([`ImportedMaterial`]):
//!
//! - **Values** take the types Studio's MaterialX nodes hold them in, numbers
//!   at MaterialX's precision (32 bits); colors are converted from the space
//!   the document says they are in (else [`SPACE`], MaterialX's working
//!   space) into the project's working space, and written in [`SPACE`].
//! - **Files.** A `filename` value is an Image node wired into its input, one
//!   per distinct file and color space in a material: its path relative to
//!   the document's folder, found through the read's
//!   [`Named`](super::files::Named) (embedded with Embed Textures), its
//!   `colorspace` the Image node's Color ([`image_color`]). Includes are read
//!   through it too, each document once. Written, an Image node reading a
//!   file names its path relative to the document; one holding an embedded
//!   file writes it beside the document ([`Beside`](super::files::Beside));
//!   one reading a result is left out, said.
//! - **The shown node.** A material node holding only a surface shows that
//!   surface, as a Material shows one; any other material node shows itself.
//!   Written, a surface shown alone is read by a `surfacematerial` named as
//!   the Material.
//! - **Lengths** (an input of a `Length`: glTF PBR's thickness, a
//!   transmission depth) say their unit (`unittype="distance"
//!   unit="millimeter"`): the project's, or metres where MaterialX names no
//!   unit of its size. Read, a length the document says the unit of (on the
//!   input, its node, its graph or the document) moves into the project's,
//!   a texture's factor driving it too; one it says none of stays as read.
//! - **Double Sided** is Kinogaki's `kinogaki:doublesided` on the material
//!   node; the name a Material shows, its `uiname` where it isn't a valid
//!   MaterialX name.
//! - **An engine's material** is written as its standard form, an OpenPBR
//!   Surface; a material nothing standard approximates is left out, said.
//!
//! [`Document::materials`]: kinogaki_render_api::materialx::Document::materials
//! [`Document::of_materials`]: kinogaki_render_api::materialx::Document::of_materials
//! [`ImportedMaterial`]: crate::standard::ImportedMaterial
mod read;
mod write;

pub use read::read;
pub use write::write;

use kinogaki_geometry::image::ImageColor;

/// MaterialX's working space by MaterialX's name (`lin_rec709`, linear
/// Rec.709): what a document's colors are in where it says no other, and
/// what Studio writes them in.
pub const SPACE: &str = "lin_rec709";

/// How a texture whose `filename` says `colorspace` reads: as the file
/// says when nothing is said; data for `raw`, `none` or `data` (whatever
/// their case); else that space.
///
/// ```
/// use kinogaki_studio_api::format::materialx::image_color;
/// use kinogaki_studio_api::geometry::image::ImageColor;
/// assert_eq!(image_color(None), ImageColor::Auto);
/// assert_eq!(image_color(Some("none")), ImageColor::Data);
/// assert_eq!(image_color(Some("srgb_texture")), ImageColor::space("srgb_texture").unwrap());
/// ```
pub fn image_color(colorspace: Option<&str>) -> ImageColor {
    match colorspace {
        None => ImageColor::Auto,
        Some(raw)
            if ["raw", "none", "data"]
                .iter()
                .any(|r| raw.eq_ignore_ascii_case(r)) =>
        {
            ImageColor::Data
        }
        Some(space) => ImageColor::space(space).unwrap_or(ImageColor::Auto),
    }
}

/// The `colorspace` a texture reading as `color` says: none for one read
/// as its file says, `none` for data (MaterialX's own spelling of no
/// conversion), else its space.
pub fn colorspace(color: &ImageColor) -> Option<&str> {
    match color {
        ImageColor::Auto => None,
        ImageColor::Data => Some("none"),
        ImageColor::Space(space) => Some(space),
    }
}

#[cfg(test)]
mod tests;
