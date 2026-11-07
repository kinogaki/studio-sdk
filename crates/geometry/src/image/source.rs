//! [`ImageSource`]: where an image's pixels come from, as the host's Image
//! node names them: a file on disk ([`ImageFile::Path`]) or the file's
//! original bytes kept in the project ([`EmbeddedFile`]), and how its colors
//! read ([`ImageColor`]). The host reads a source through the installed
//! formats into an [`Image`](super::Image); a material's texture holds one
//! until the host sends the engine the Texture prim it read.
use std::fmt;
use std::sync::Arc;

/// An image file's original bytes, kept in a project: its name, whose
/// extension picks the format that reads it and which the color rules match,
/// its bytes (shared by every copy) and their BLAKE3 hash, what a store keys
/// its reads by and a save writes it once by.
#[derive(Clone)]
pub struct EmbeddedFile {
    name: Arc<str>,
    bytes: Arc<Vec<u8>>,
    hash: [u8; 32],
}

impl EmbeddedFile {
    /// File `name` holding `bytes`, hashed.
    pub fn new(name: &str, bytes: Vec<u8>) -> EmbeddedFile {
        let hash = *blake3::hash(&bytes).as_bytes();
        EmbeddedFile {
            name: Arc::from(name),
            bytes: Arc::new(bytes),
            hash,
        }
    }

    /// The same bytes as file `name`, sharing them and their hash.
    pub fn named(&self, name: &str) -> EmbeddedFile {
        EmbeddedFile {
            name: Arc::from(name),
            ..self.clone()
        }
    }

    /// Its name: a file name, no folder.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Its bytes.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// The BLAKE3 hash of its bytes.
    pub fn hash(&self) -> &[u8; 32] {
        &self.hash
    }
}

/// Files are equal by name and bytes.
impl PartialEq for EmbeddedFile {
    fn eq(&self, other: &EmbeddedFile) -> bool {
        self.hash == other.hash && self.name == other.name
    }
}

impl Eq for EmbeddedFile {}

impl fmt::Debug for EmbeddedFile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let hash: String = self.hash[..4].iter().map(|b| format!("{b:02x}")).collect();
        (f.debug_struct("EmbeddedFile"))
            .field("name", &self.name)
            .field("bytes", &self.bytes.len())
            .field("hash", &hash)
            .finish()
    }
}

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
    /// says.
    Space(Arc<str>),
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
    fn embedded_files_are_equal_by_name_and_bytes() {
        let a = EmbeddedFile::new("wood.png", vec![1, 2, 3]);
        let b = EmbeddedFile::new("wood.png", vec![1, 2, 3]);
        assert_eq!(a, b);
        assert_eq!(a.hash(), b.hash());
        assert_ne!(a, EmbeddedFile::new("oak.png", vec![1, 2, 3]));
        assert_ne!(a, EmbeddedFile::new("wood.png", vec![1, 2, 4]));
        assert_eq!((a.name(), a.bytes()), ("wood.png", &[1u8, 2, 3][..]));
        let source = ImageSource::new(ImageFile::Embedded(a), ImageColor::Data);
        assert_eq!(source.name(), "wood.png");
        assert_eq!(ImageSource::path("t/a.png").name(), "t/a.png");
        assert_eq!(ImageSource::path("a.png").color(), &ImageColor::Auto);
    }
}
