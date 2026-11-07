//! [`Input`]: the file a format reads, on disk or held in memory.
use super::files::{regular, whole};
use crate::Error;
use kinogaki_geometry::image::EmbeddedFile;
use std::borrow::Cow;
use std::fmt;
use std::io::Read as _;
use std::path::Path;
use std::sync::OnceLock;

/// How many first bytes [`Input::head`] holds at most.
const HEAD: usize = 64;

/// The file a format reads. A file embedded in a project, or packed
/// inside another file, has bytes and a name but no path, so a format
/// reads an `Input`: its [`name`](Input::name) (what the extension and a
/// project's file rules are read from), its [`head`](Input::head) (what
/// [`FileFormat::detects`](super::FileFormat::detects) sees), its
/// [`bytes`](Input::bytes), and its [`path`](Input::path) when it has one,
/// beside which a format finds the files it names (an OBJ's MTL). A format
/// keeping files packed in it (a usdz's textures, a GLB's images) takes its
/// bytes [`held`](Input::held), whose ranges it embeds without a copy.
///
/// ```
/// use kinogaki_studio_api::format::Input;
/// let held = b"P3\n1 1\n255\n";
/// let input = Input::memory("swatch.ppm", held);
/// assert_eq!((input.name(), input.path()), ("swatch.ppm", None));
/// assert_eq!(input.head(), held);
/// assert_eq!(&*input.bytes().unwrap(), held);
/// assert_eq!(input.to_string(), "swatch.ppm");
/// ```
#[derive(Debug)]
pub struct Input<'a> {
    name: Cow<'a, str>,
    source: Source<'a>,
    head: OnceLock<Vec<u8>>,
}

#[derive(Debug)]
enum Source<'a> {
    File(&'a Path),
    Memory(&'a [u8]),
    Embedded(&'a EmbeddedFile),
}

impl<'a> Input<'a> {
    /// The file at `path`, read as it is asked for.
    pub fn file(path: &'a Path) -> Self {
        let name = path
            .file_name()
            .map_or(Cow::Borrowed(""), |n| n.to_string_lossy());
        Self {
            name,
            source: Source::File(path),
            head: OnceLock::new(),
        }
    }

    /// `bytes`, the whole of a file named `name` (`wood.png`), held in
    /// memory: an embedded file, an entry of a package.
    pub fn memory(name: &'a str, bytes: &'a [u8]) -> Self {
        Self {
            name: Cow::Borrowed(name),
            source: Source::Memory(bytes),
            head: OnceLock::new(),
        }
    }

    /// `file`, a file kept in the project (or packed in another), its bytes
    /// shared.
    pub fn embedded(file: &'a EmbeddedFile) -> Self {
        Self {
            name: Cow::Borrowed(file.name()),
            source: Source::Embedded(file),
            head: OnceLock::new(),
        }
    }

    /// Its file name, extension included; empty for a path that ends in
    /// none.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Where it is on disk; `None` for a file held in memory, which names no
    /// files beside it.
    pub fn path(&self) -> Option<&'a Path> {
        match self.source {
            Source::File(path) => Some(path),
            Source::Memory(_) | Source::Embedded(_) => None,
        }
    }

    /// Its first bytes, up to 64: fewer for a shorter file, none for a
    /// file that can't be read (its [`bytes`](Self::bytes) say why).
    pub fn head(&self) -> &[u8] {
        self.head.get_or_init(|| match self.source {
            Source::Memory(bytes) => bytes[..bytes.len().min(HEAD)].to_vec(),
            Source::Embedded(file) => file.bytes()[..file.bytes().len().min(HEAD)].to_vec(),
            Source::File(path) => {
                let mut head = Vec::with_capacity(HEAD);
                let read = (regular(path).ok())
                    .and_then(|(file, _)| file.take(HEAD as u64).read_to_end(&mut head).ok());
                read.map_or_else(Vec::new, |_| head)
            }
        })
    }

    /// All of its bytes: read from disk each time they are asked for (a
    /// regular file alone, to the size it had when opened: a pipe or a
    /// device is never read, [`regular`]), or lent from memory.
    ///
    /// # Errors
    /// The file can't be read (it isn't there, or isn't a regular file):
    /// [`Invalid`](crate::ErrorKind::Invalid), the message naming it.
    pub fn bytes(&self) -> Result<Cow<'a, [u8]>, Error> {
        match self.source {
            Source::Memory(bytes) => Ok(Cow::Borrowed(bytes)),
            Source::Embedded(file) => Ok(Cow::Borrowed(file.bytes())),
            Source::File(path) => read(path).map(Cow::Owned),
        }
    }

    /// All of its bytes as one shared file named as it is, whose ranges a
    /// format keeps as the files packed in it
    /// ([`EmbeddedFile::slice`]): a file kept in the project shared as it
    /// is, one on disk read once, bytes lent from memory copied.
    ///
    /// # Errors
    /// As [`bytes`](Self::bytes).
    pub fn held(&self) -> Result<EmbeddedFile, Error> {
        match self.source {
            Source::Embedded(file) => Ok(file.clone()),
            Source::Memory(bytes) => Ok(EmbeddedFile::new(&self.name, bytes.to_vec())),
            Source::File(path) => Ok(EmbeddedFile::new(&self.name, read(path)?)),
        }
    }
}

/// The regular file at `path`, read whole.
fn read(path: &Path) -> Result<Vec<u8>, Error> {
    let (file, len) = regular(path)?;
    whole(file, len, path)
}

/// What a message names it by: its path, else its name.
impl fmt::Display for Input<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.source {
            Source::File(path) => path.display().fmt(f),
            Source::Memory(_) | Source::Embedded(_) => f.write_str(&self.name),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_is_read_as_it_is_asked_for_and_a_missing_one_says_so() {
        let dir = std::env::temp_dir().join(format!("kinogaki-input-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("long.txt");
        let text: Vec<u8> = (0..200u8).collect();
        std::fs::write(&path, &text).unwrap();
        let input = Input::file(&path);
        assert_eq!((input.name(), input.path()), ("long.txt", Some(&*path)));
        assert_eq!(input.head(), &text[..HEAD]);
        assert_eq!(&*input.bytes().unwrap(), &text[..]);
        assert_eq!(input.to_string(), path.display().to_string());
        let gone = dir.join("gone.png");
        let missing = Input::file(&gone);
        assert!(missing.head().is_empty());
        let why = missing.bytes().unwrap_err();
        assert_eq!(why.kind, crate::ErrorKind::Invalid, "nothing uninstalled");
        assert!(why.message().starts_with(&gone.display().to_string()));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// A device or a folder is never read, however it is asked for: only a
    /// regular file is (a pipe, which opening would wait on, alike).
    #[test]
    fn only_a_regular_file_is_read() {
        let dir = std::env::temp_dir().join(format!("kinogaki-regular-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for path in [Path::new("/dev/null"), &dir] {
            let input = Input::file(path);
            let why = input.bytes().unwrap_err();
            assert!(why.message().contains("not a regular file"), "{why:?}");
            assert!(input.held().is_err() && input.head().is_empty());
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn bytes_in_memory_are_lent_and_their_head_is_at_most_its_length() {
        let short = Input::memory("a.bin", b"ab");
        assert_eq!(short.head(), b"ab");
        let long = [7u8; 100];
        let input = Input::memory("b.bin", &long);
        assert_eq!(input.head().len(), HEAD);
        assert!(matches!(input.bytes().unwrap(), Cow::Borrowed(_)));
    }

    #[test]
    fn a_kept_file_is_lent_and_held_without_a_copy() {
        let package = EmbeddedFile::new("chair.usdz", (0..100u8).collect());
        let input = Input::embedded(&package);
        assert_eq!((input.name(), input.path()), ("chair.usdz", None));
        assert_eq!(
            (input.head().len(), input.to_string()),
            (HEAD, "chair.usdz".into())
        );
        let held = input.held().unwrap();
        assert!(std::ptr::eq(held.bytes(), package.bytes()));
        let entry = held.slice("wood.png", 10..20).unwrap();
        assert!(std::ptr::eq(entry.bytes(), &package.bytes()[10..20]));
        let lent = Input::memory("b.bin", b"abc").held().unwrap();
        assert_eq!((lent.name(), lent.bytes()), ("b.bin", &b"abc"[..]));
    }
}
