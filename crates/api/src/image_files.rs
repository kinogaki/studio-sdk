//! Image files as the host reads them: a package asks by path and never
//! waits. What a reader gets are standard [`Image`]s in linear ACEScg half
//! floats, premultiplied, RGBA: the host holds the decoders and the color
//! maths (its file rules, its working space).
use kinogaki_geometry::image::Image;

/// An image file as far as it is read.
#[derive(Clone, Debug, PartialEq)]
pub enum FileImage {
    /// It is on its way: the window wakes when it arrives.
    Reading,
    /// Here it is.
    Read(Image),
    /// Why it can't be read.
    Failed(String),
}

/// The host's image files.
pub trait ImageFiles: Send + Sync {
    /// The file at `path` (relative to the project's folder unless absolute):
    /// asking starts reading it, and never waits.
    fn image(&self, path: &str) -> FileImage;

    /// Moves whenever a file's answer does.
    fn revision(&self) -> u64;
}
