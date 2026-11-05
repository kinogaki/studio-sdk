//! Image files as the host reads them: a package asks by path and never
//! waits. What a reader gets are standard [`Image`]s of half floats,
//! premultiplied, RGBA: the host holds the decoders and the color maths (a
//! file's own tag, its file rules, its working space). [`image`](ImageFiles::image)
//! is linear ACEScg, for a scene-referred network; [`picture`](ImageFiles::picture)
//! is what the file holds, for a page.
use kinogaki_geometry::Error;
use kinogaki_geometry::image::Image;

/// An image file as far as it is read.
#[derive(Clone, Debug, PartialEq)]
pub enum FileImage {
    /// It is on its way: the window wakes when it arrives.
    Reading,
    /// Here it is.
    Read(Image),
    /// Why it can't be read.
    Failed(Error),
}

/// The host's image files. A UI's or a runner's never wait; a cook's
/// ([`Project::files`](crate::cook::Project::files)) waits on its worker and
/// answers [`Read`](FileImage::Read) or [`Failed`](FileImage::Failed). Asked
/// again, a file that hasn't changed answers with the same tiles, so what shows
/// it uploads nothing.
pub trait ImageFiles: Send + Sync {
    /// The file at `path` (relative to the project's folder unless absolute),
    /// its colors in linear ACEScg.
    fn image(&self, path: &str) -> FileImage;

    /// The file at `path` to place on a page: a display-referred file (a PNG, a
    /// JPEG) keeps its code values, in the space of the display its tag says; a
    /// scene-referred one (a linear OpenEXR) is linear ACEScg, as
    /// [`image`](Self::image) has it.
    fn picture(&self, path: &str) -> FileImage;

    /// Moves whenever a file's answer does.
    fn revision(&self) -> u64;
}

/// No image files: where nothing can read them (a project read outside the
/// editor).
#[derive(Clone, Copy, Debug, Default)]
pub struct NoFiles;

impl ImageFiles for NoFiles {
    fn image(&self, _: &str) -> FileImage {
        FileImage::Failed(Error::missing("nothing reads image files here"))
    }

    fn picture(&self, path: &str) -> FileImage {
        self.image(path)
    }

    fn revision(&self) -> u64 {
        0
    }
}
