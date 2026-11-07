//! [`ImageSource`]: where an image's pixels come from, as the host's Image
//! node names them: a file on disk ([`ImageFile::Path`]) or the file's
//! original bytes kept in the project ([`EmbeddedFile`]), and how its colors
//! read ([`ImageColor`]). The host reads a source through the installed
//! formats into an [`Image`](super::Image); a material's texture holds one
//! until the host sends the engine the Texture prim it read.
use std::fmt;
use std::ops::Range;
use std::sync::{Arc, OnceLock};

/// An image file's original bytes, kept in a project: its name, whose
/// extension picks the format that reads it and which the color rules match,
/// its bytes, and their BLAKE3 hash, what a store keys its reads by and a save
/// writes it once by.
///
/// The bytes are a range of a shared buffer: every copy of a file shares
/// them, and a file packed in another (a usdz's texture, a GLB's image) is
/// held as a [`slice`](Self::slice) of that file's bytes, never copied. The
/// hash is made the first time it is asked for, a piece at a time on the
/// process's workers, and every copy shares it.
///
/// ```
/// use kinogaki_geometry::image::EmbeddedFile;
/// let package = EmbeddedFile::new("chair.usdz", b"PK..wood..".to_vec());
/// let wood = package.slice("wood.png", 4..8).unwrap();
/// assert_eq!((wood.name(), wood.bytes()), ("wood.png", &b"wood"[..]));
/// assert_eq!(wood, EmbeddedFile::new("wood.png", b"wood".to_vec()));
/// assert!(package.slice("past.png", 8..12).is_err());
/// ```
#[derive(Clone)]
pub struct EmbeddedFile {
    name: Arc<str>,
    held: Arc<Held>,
}

/// The bytes of an [`EmbeddedFile`] and their hash, once made.
struct Held {
    buffer: Arc<Vec<u8>>,
    range: Range<usize>,
    hash: OnceLock<[u8; 32]>,
}

impl EmbeddedFile {
    /// File `name` holding `bytes`, taken without a copy.
    pub fn new(name: &str, bytes: Vec<u8>) -> EmbeddedFile {
        let range = 0..bytes.len();
        EmbeddedFile {
            name: Arc::from(name),
            held: Arc::new(Held {
                buffer: Arc::new(bytes),
                range,
                hash: OnceLock::new(),
            }),
        }
    }

    /// The same bytes as file `name`, sharing them and their hash.
    pub fn named(&self, name: &str) -> EmbeddedFile {
        EmbeddedFile {
            name: Arc::from(name),
            held: self.held.clone(),
        }
    }

    /// File `name` holding `range` of its bytes (an entry of a package),
    /// sharing them.
    ///
    /// # Errors
    /// `range` doesn't lie within its bytes.
    pub fn slice(&self, name: &str, range: Range<usize>) -> Result<EmbeddedFile, crate::Error> {
        let inside = range.start <= range.end && range.end <= self.held.range.len();
        if !inside {
            return Err(crate::Error::invalid(format_args!(
                "{name}: bytes {range:?} lie outside {}'s {}",
                self.name,
                self.held.range.len()
            )));
        }
        let start = self.held.range.start;
        Ok(EmbeddedFile {
            name: Arc::from(name),
            held: Arc::new(Held {
                buffer: self.held.buffer.clone(),
                range: start + range.start..start + range.end,
                hash: OnceLock::new(),
            }),
        })
    }

    /// Its name: a file name, no folder.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Its bytes.
    pub fn bytes(&self) -> &[u8] {
        &self.held.buffer[self.held.range.clone()]
    }

    /// The BLAKE3 hash of its bytes, made the first time it is asked for.
    pub fn hash(&self) -> &[u8; 32] {
        self.held.hash.get_or_init(|| hash(self.bytes()))
    }
}

/// How many bytes a piece of a parallel hash holds: a power of two of
/// BLAKE3's chunks, so each piece is a subtree of its tree.
const PIECE: usize = 1 << 20;

/// `bytes`' BLAKE3 hash, as `blake3::hash` gives it: past one piece, each
/// piece's subtree on the workers, then the subtrees merged as BLAKE3's
/// tree merges them.
fn hash(bytes: &[u8]) -> [u8; 32] {
    use blake3::hazmat::{HasherExt, Mode, left_subtree_len, merge_subtrees_root};
    if bytes.len() <= PIECE {
        return *blake3::hash(bytes).as_bytes();
    }
    let pieces: Vec<[u8; 32]> = crate::parallel::map(bytes.len().div_ceil(PIECE), 1, |range| {
        (range.map(|k| {
            let at = k * PIECE;
            let mut hasher = blake3::Hasher::new();
            hasher.set_input_offset(at as u64);
            hasher.update(&bytes[at..(at + PIECE).min(bytes.len())]);
            hasher.finalize_non_root()
        }))
        .collect::<Vec<_>>()
    })
    .concat();
    let len = bytes.len() as u64;
    let left = left_subtree_len(len);
    let (l, r) = pieces.split_at((left / PIECE as u64) as usize);
    let root = merge_subtrees_root(&subtree(l, left), &subtree(r, len - left), Mode::Hash);
    *root.as_bytes()
}

/// The chaining value of the subtree of `len` bytes whose pieces' values
/// are `pieces`.
fn subtree(pieces: &[[u8; 32]], len: u64) -> [u8; 32] {
    use blake3::hazmat::{Mode, left_subtree_len, merge_subtrees_non_root};
    if let [one] = pieces {
        return *one;
    }
    let left = left_subtree_len(len);
    let (l, r) = pieces.split_at((left / PIECE as u64) as usize);
    merge_subtrees_non_root(&subtree(l, left), &subtree(r, len - left), Mode::Hash)
}

/// Files are equal by name and bytes.
impl PartialEq for EmbeddedFile {
    fn eq(&self, other: &EmbeddedFile) -> bool {
        self.name == other.name
            && (Arc::ptr_eq(&self.held, &other.held)
                || self.held.range.len() == other.held.range.len() && self.hash() == other.hash())
    }
}

impl Eq for EmbeddedFile {}

impl fmt::Debug for EmbeddedFile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let hash: String = self.hash()[..4]
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        (f.debug_struct("EmbeddedFile"))
            .field("name", &self.name)
            .field("bytes", &self.held.range.len())
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

    #[test]
    fn a_hash_made_a_piece_at_a_time_is_blake3s() {
        let bytes: Vec<u8> = (0..5 * PIECE + 77).map(|i| (i * 31 % 251) as u8).collect();
        for len in [
            0,
            1,
            1024,
            PIECE,
            PIECE + 1,
            2 * PIECE,
            3 * PIECE + 5,
            bytes.len(),
        ] {
            let file = EmbeddedFile::new("a", bytes[..len].to_vec());
            assert_eq!(file.hash(), blake3::hash(&bytes[..len]).as_bytes(), "{len}");
        }
        // A slice hashes its own bytes; copies share the hash.
        let whole = EmbeddedFile::new("whole", bytes.clone());
        let part = whole.slice("part", PIECE..3 * PIECE + 9).unwrap();
        assert_eq!(part.bytes(), &bytes[PIECE..3 * PIECE + 9]);
        assert_eq!(part.hash(), blake3::hash(part.bytes()).as_bytes());
        let inner = part.slice("inner", 1..3).unwrap();
        assert_eq!(inner.bytes(), &bytes[PIECE + 1..PIECE + 3]);
        assert!(part.slice("past", 0..2 * PIECE + 10).is_err());
        assert!(std::ptr::eq(part.named("again").hash(), part.hash()));
    }
}
