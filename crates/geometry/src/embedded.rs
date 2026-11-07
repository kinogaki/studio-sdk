//! [`EmbeddedFile`]: a file's original bytes kept in a project: an
//! image an Image node holds, a stage a format reads from memory. Any file,
//! so it lives at the crate's root.
use std::fmt;
use std::ops::Range;
use std::sync::{Arc, OnceLock};

/// A file's original bytes, kept in a project: its name, whose extension
/// picks the format that reads it (an image's color rules match it too),
/// its bytes, and their BLAKE3 hash, what a store keys its reads by and a save
/// writes it once by.
///
/// The bytes are a range of a shared buffer: every copy of a file shares
/// them. A file packed in another (a usdz's texture, a GLB's image) is a
/// [`slice`](Self::slice) of that file's bytes: shared when it is most of
/// them, else copied out once, so a small entry never keeps a large package
/// alive. The hash is made the first time it is asked for, a piece at a
/// time on the process's workers, and every copy shares it.
///
/// ```
/// use kinogaki_geometry::EmbeddedFile;
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

    /// File `name` holding `range` of its bytes (an entry of a package):
    /// sharing its buffer when the range is at least half of it, else a
    /// copy of the range, so the entry holds no more than twice its size.
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
        let range = start + range.start..start + range.end;
        if range.len() * 2 < self.held.buffer.len() {
            return Ok(EmbeddedFile::new(name, self.held.buffer[range].to_vec()));
        }
        Ok(EmbeddedFile {
            name: Arc::from(name),
            held: Arc::new(Held {
                buffer: self.held.buffer.clone(),
                range,
                hash: OnceLock::new(),
            }),
        })
    }

    /// How many bytes it keeps alive: its buffer's, which a shared slice
    /// holds whole.
    #[cfg(test)]
    fn held_bytes(&self) -> usize {
        self.held.buffer.len()
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
    /// A host asks where it reads the file (a worker), so a later ask on the
    /// window's thread finds it made.
    pub fn hash(&self) -> &[u8; 32] {
        self.held.hash.get_or_init(|| hash(self.bytes()))
    }

    /// Its hash, when it was made already.
    pub fn hashed(&self) -> Option<&[u8; 32]> {
        self.held.hash.get()
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
    let pieces: Vec<[u8; 32]> = crate::parallel::blocks(bytes.len().div_ceil(PIECE), 1, |range| {
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

/// Its name and length: never its hash, which would read every byte.
impl fmt::Debug for EmbeddedFile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        (f.debug_struct("EmbeddedFile"))
            .field("name", &self.name)
            .field("bytes", &self.held.range.len())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::image::{ImageColor, ImageFile, ImageSource};

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
        let part = whole.slice("part", PIECE..4 * PIECE + 9).unwrap();
        assert_eq!(part.bytes(), &bytes[PIECE..4 * PIECE + 9]);
        assert_eq!(part.hash(), blake3::hash(part.bytes()).as_bytes());
        let inner = part.slice("inner", 1..3).unwrap();
        assert_eq!(inner.bytes(), &bytes[PIECE + 1..PIECE + 3]);
        assert!(part.slice("past", 0..3 * PIECE + 10).is_err());
        assert!(std::ptr::eq(part.named("again").hash(), part.hash()));
    }

    /// A slice of most of its package shares it; a small one is copied
    /// out, keeping only its own bytes alive.
    #[test]
    fn a_small_slice_lets_its_package_go() {
        let package = EmbeddedFile::new("chair.usdz", vec![7; 1000]);
        let most = package.slice("mesh.usdc", 100..700).unwrap();
        assert_eq!(most.held_bytes(), 1000, "shared");
        let wood = package.slice("wood.png", 10..20).unwrap();
        assert_eq!((wood.held_bytes(), wood.bytes()), (10, &[7u8; 10][..]));
        assert_eq!(wood, EmbeddedFile::new("wood.png", vec![7; 10]));
    }

    /// Debug reads no bytes: a large file prints at once, its hash unmade.
    #[test]
    fn debug_prints_the_name_and_length_alone() {
        let file = EmbeddedFile::new("sky.exr", vec![1; 3 * PIECE]);
        let printed = format!("{file:?}");
        assert_eq!(
            printed,
            format!("EmbeddedFile {{ name: \"sky.exr\", bytes: {} }}", 3 * PIECE)
        );
        assert!(file.hashed().is_none());
    }
}
