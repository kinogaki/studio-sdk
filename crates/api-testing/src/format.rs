//! A format's test support: a folder of a test's own ([`Scratch`]), and a
//! write read back as Import reads it ([`round_trip`]).
use crate::Unconverted;
use kinogaki_studio_api::geometry::Units;
use kinogaki_studio_api::{
    Diagnostic, Error, FileFormat, FileItem, Imported, Input, Purpose, Reading, Writing,
};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

/// A new, empty folder of a test's own in the system's temporary folder,
/// by its canonical path (the temporary folder may be behind a link, which
/// a relative path a written file names can't climb back through);
/// removed, with what was written in it, when dropped. No two scratches of
/// a run share a folder, nor do two runs. It is the folder's [`Path`].
///
/// ```
/// use kinogaki_studio_api_testing::format::Scratch;
/// let scratch = Scratch::new("doc");
/// std::fs::write(scratch.join("a.txt"), "a").unwrap();
/// let folder = scratch.to_owned();
/// drop(scratch);
/// assert!(!folder.exists());
/// ```
#[derive(Debug)]
pub struct Scratch(PathBuf);

impl Scratch {
    /// A folder whose name holds `name`, the test's.
    pub fn new(name: &str) -> Scratch {
        static MADE: AtomicU32 = AtomicU32::new(0);
        let made = MADE.fetch_add(1, Ordering::Relaxed);
        let id = std::process::id();
        let dir = std::env::temp_dir().join(format!("kinogaki-{name}-{id}-{made}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("a scratch folder");
        Scratch(dir.canonicalize().expect("a scratch folder"))
    }
}

impl std::ops::Deref for Scratch {
    type Target = Path;
    fn deref(&self) -> &Path {
        &self.0
    }
}

impl AsRef<Path> for Scratch {
    fn as_ref(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// `items` written to `path` by `format`'s writer with its default
/// options, and the file read back as Import reads it with the format's
/// default options, in a project of `units` whose colors convert to
/// nothing ([`Unconverted`]): what was read (its warnings the read's), and
/// the writer's warnings.
///
/// # Errors
/// A format that writes nothing, a failed write, or a failed read.
pub fn round_trip(
    format: &dyn FileFormat,
    items: &[FileItem],
    path: &Path,
    units: Units,
) -> Result<(Imported, Vec<Diagnostic>), Error> {
    let label = format.label();
    let writer =
        (format.writer()).ok_or_else(|| Error::invalid(format!("{label} writes nothing")))?;
    let writing = Writing::new(units, &Unconverted);
    let said = writer.write(path, items, &writing, &writer.options())?;
    let reading = Reading::new(Purpose::Objects, units, &Unconverted);
    let read = format.read(&Input::file(path), &format.options(), &reading)?;
    Ok((read, said))
}
