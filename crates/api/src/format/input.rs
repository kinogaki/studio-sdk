//! [`Input`]: the file a format reads, on disk or held in memory.
use crate::Error;
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
/// beside which a format finds the files it names (an OBJ's MTL).
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
            Source::Memory(_) => None,
        }
    }

    /// Its first bytes, up to 64: fewer for a shorter file, none for a
    /// file that can't be read (its [`bytes`](Self::bytes) say why).
    pub fn head(&self) -> &[u8] {
        self.head.get_or_init(|| match self.source {
            Source::Memory(bytes) => bytes[..bytes.len().min(HEAD)].to_vec(),
            Source::File(path) => {
                let mut head = Vec::with_capacity(HEAD);
                let read = std::fs::File::open(path)
                    .and_then(|file| file.take(HEAD as u64).read_to_end(&mut head));
                read.map_or_else(|_| Vec::new(), |_| head)
            }
        })
    }

    /// All of its bytes: read from disk each time they are asked for, or
    /// lent from memory.
    ///
    /// # Errors
    /// The file can't be read: [`Missing`](crate::ErrorKind::Missing) when
    /// there is no such file, else [`Invalid`](crate::ErrorKind::Invalid);
    /// the message names it.
    pub fn bytes(&self) -> Result<Cow<'a, [u8]>, Error> {
        match self.source {
            Source::Memory(bytes) => Ok(Cow::Borrowed(bytes)),
            Source::File(path) => std::fs::read(path).map(Cow::Owned).map_err(|e| {
                let why = format_args!("{}: {e}", path.display());
                match e.kind() {
                    std::io::ErrorKind::NotFound => Error::missing(why),
                    _ => Error::invalid(why),
                }
            }),
        }
    }
}

/// What a message names it by: its path, else its name.
impl fmt::Display for Input<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.source {
            Source::File(path) => path.display().fmt(f),
            Source::Memory(_) => f.write_str(&self.name),
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
        assert_eq!(why.kind, crate::ErrorKind::Missing);
        assert!(why.message().starts_with(&gone.display().to_string()));
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
}
