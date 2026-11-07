//! [`ImageSource`]: where an image's pixels come from, as the host's Image
//! node names them: a file on disk ([`ImageFile::Path`]) or the file's
//! original bytes kept in the project ([`EmbeddedFile`]), and how its colors
//! read ([`ImageColor`]). The host reads a source through the installed
//! formats into an [`Image`](super::Image); a material's texture holds one
//! until the host sends the engine the Texture prim it read.
use crate::{EmbeddedFile, Error};
use std::sync::Arc;

/// The file a source reads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ImageFile {
    /// A path as typed: relative to the project's folder unless absolute.
    Path(Arc<str>),
    /// The file's bytes, kept in the project.
    Embedded(EmbeddedFile),
}

/// How a source's colors read.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum ImageColor {
    /// As the file says: its tag, else the host's file rules on its name,
    /// else sRGB for integers and the working space for floats.
    #[default]
    Auto,
    /// Its values as stored, never converted: data (a normal map, a
    /// roughness).
    Data,
    /// In this space of the project's color configuration, whatever the file
    /// says. Made by [`space`](ImageColor::space), which refuses a name a
    /// save would read back as another color.
    Space(Arc<str>),
}

impl ImageColor {
    /// Colors in space `name` of the project's color configuration.
    ///
    /// ```
    /// use kinogaki_geometry::image::ImageColor;
    /// assert!(ImageColor::space("srgb_rec709_scene").is_ok());
    /// assert!(ImageColor::space("data").is_err(), "Data's key");
    /// ```
    ///
    /// # Errors
    /// A blank name, or one that is [`Auto`](Self::Auto)'s or
    /// [`Data`](Self::Data)'s key ([`key`](Self::key)): saved, it would read
    /// back as that color.
    pub fn space(name: &str) -> Result<ImageColor, Error> {
        match ImageColor::of_key(name) {
            ImageColor::Space(space) if !space.trim().is_empty() => Ok(ImageColor::Space(space)),
            _ => Err(Error::invalid(format_args!(
                "{name:?} names no color space"
            ))),
        }
    }

    /// How an Image node's Color and a saved source spell it: `auto`,
    /// `data`, or the space's name.
    pub fn key(&self) -> &str {
        match self {
            ImageColor::Auto => "auto",
            ImageColor::Data => "data",
            ImageColor::Space(space) => space,
        }
    }

    /// The color `key` spells ([`key`](Self::key)); a blank one is
    /// [`Auto`](Self::Auto).
    pub fn of_key(key: &str) -> ImageColor {
        match key {
            "" | "auto" => ImageColor::Auto,
            "data" => ImageColor::Data,
            space => ImageColor::Space(Arc::from(space)),
        }
    }
}

/// Where an image's pixels come from, and how its colors read (see the
/// module docs).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImageSource {
    file: ImageFile,
    color: ImageColor,
}

impl ImageSource {
    /// `file`, its colors read as `color` says.
    pub fn new(file: ImageFile, color: ImageColor) -> ImageSource {
        ImageSource { file, color }
    }

    /// The file at `path` (as typed), its colors as it says.
    pub fn path(path: &str) -> ImageSource {
        ImageSource::new(ImageFile::Path(Arc::from(path)), ImageColor::Auto)
    }

    /// The file it reads.
    pub fn file(&self) -> &ImageFile {
        &self.file
    }

    /// How its colors read.
    pub fn color(&self) -> &ImageColor {
        &self.color
    }

    /// The file's name: the path as typed, or the embedded file's name.
    pub fn name(&self) -> &str {
        match &self.file {
            ImageFile::Path(path) => path,
            ImageFile::Embedded(file) => file.name(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_space_is_never_a_key_of_another_color() {
        for color in [
            ImageColor::Auto,
            ImageColor::Data,
            ImageColor::space("ACEScg").unwrap(),
        ] {
            assert_eq!(ImageColor::of_key(color.key()), color);
        }
        assert_eq!(ImageColor::of_key(""), ImageColor::Auto);
        for refused in ["", "  ", "auto", "data"] {
            assert!(ImageColor::space(refused).is_err(), "{refused:?}");
        }
        // Another case is another name: the configuration's to resolve.
        assert_eq!(
            ImageColor::space("Data").unwrap(),
            ImageColor::Space(Arc::from("Data"))
        );
    }
}
