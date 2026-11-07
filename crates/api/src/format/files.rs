//! The files a file names: those a file being read names, found and read
//! by one rule ([`Named`]), and the image files a written document names
//! ([`Beside`]): what a format writing materials (an MTL library, a
//! MaterialX document) writes for the Image nodes feeding its textures. A
//! file on disk is named by its path relative to the document; an embedded
//! file is written beside the document, once for equal bytes, and named so.
//!
//! ```
//! use kinogaki_studio_api::format::files::Beside;
//! use kinogaki_studio_api::geometry::image::{EmbeddedFile, ImageFile};
//!
//! let folder = std::env::temp_dir().join(format!("beside-doc-{}", std::process::id()));
//! let mut beside = Beside::new(&folder.join("scene.mtl"));
//! let on_disk = ImageFile::Path(folder.join("maps/wood.png").display().to_string().into());
//! assert_eq!(beside.name(&on_disk).unwrap(), "maps/wood.png");
//! let held = ImageFile::Embedded(EmbeddedFile::new("wood.png", vec![1, 2, 3]));
//! assert_eq!(beside.name(&held).unwrap(), "scene_textures/wood.png");
//! assert_eq!(std::fs::read(folder.join("scene_textures/wood.png")).unwrap(), [1, 2, 3]);
//! let beside_it = ImageFile::Path(folder.join("../maps/a.png").display().to_string().into());
//! assert_eq!(beside.name(&beside_it).unwrap(), "../maps/a.png");
//! # std::fs::remove_dir_all(&folder).unwrap();
//! ```
mod named;

pub use named::Named;

use crate::Error;
use kinogaki_geometry::image::{EmbeddedFile, ImageFile};
use std::collections::HashMap;
use std::io::Read as _;
use std::path::{Component, Path, PathBuf};

/// The regular file at `path`, open, and its size then: looked at before
/// it is opened (opening a pipe waits for a writer) and again once open. A
/// device, a pipe or a folder is never read, whoever names it (a file being
/// read, a project): one could block, or never end.
///
/// # Errors
/// It isn't there ([`Missing`](crate::ErrorKind::Missing)), isn't a regular
/// file, or can't be opened; the message names it.
pub fn regular(path: &Path) -> Result<(std::fs::File, u64), Error> {
    let failed = |e: std::io::Error| {
        let why = format_args!("{}: {e}", path.display());
        match e.kind() {
            std::io::ErrorKind::NotFound => Error::missing(why),
            _ => Error::invalid(why),
        }
    };
    let regular = |meta: std::fs::Metadata| match meta.is_file() {
        true => Ok(meta.len()),
        false => Err(Error::invalid(format_args!(
            "{}: not a regular file",
            path.display()
        ))),
    };
    regular(std::fs::metadata(path).map_err(failed)?)?;
    let file = std::fs::File::open(path).map_err(failed)?;
    let len = regular(file.metadata().map_err(failed)?)?;
    Ok((file, len))
}

/// The `len` bytes of `file`, opened at `path` by [`regular`]: no more than
/// it held when it was opened.
pub(crate) fn whole(file: std::fs::File, len: u64, path: &Path) -> Result<Vec<u8>, Error> {
    let mut bytes = Vec::with_capacity(usize::try_from(len).unwrap_or(0));
    (file.take(len).read_to_end(&mut bytes))
        .map_err(|e| Error::invalid(format_args!("{}: {e}", path.display())))?;
    Ok(bytes)
}

/// Where the image files a document names go, as one write names them.
#[derive(Debug)]
pub struct Beside {
    /// The document's folder, which the names are relative to.
    folder: PathBuf,
    /// Where embedded files are written: `<stem>_textures` in that folder.
    textures: PathBuf,
    /// The name each embedded file was written under, by its bytes' hash.
    written: HashMap<[u8; 32], String>,
}

impl Beside {
    /// The files of a document written at `document`.
    pub fn new(document: &Path) -> Beside {
        let folder = document
            .parent()
            .map_or_else(PathBuf::new, |f| real(f).into_owned());
        let stem = document
            .file_stem()
            .map_or_else(String::new, |s| s.to_string_lossy().into_owned());
        Beside {
            textures: folder.join(format!("{stem}_textures")),
            folder,
            written: HashMap::new(),
        }
    }

    /// What the document names `file` by, `/` between its parts: a file on
    /// disk by its path relative to the document's folder, both with their
    /// links followed where they exist (`..` climbing out of the folder;
    /// absolute where none reaches it, on another volume); an
    /// embedded file written into the `<stem>_textures` folder beside the
    /// document under its own name, once for equal bytes, numbered past a
    /// file of that name holding other bytes (`wood 2.png`), and named by
    /// where it went.
    ///
    /// # Errors
    /// Why an embedded file can't be written.
    pub fn name(&mut self, file: &ImageFile) -> Result<String, Error> {
        let held = match file {
            ImageFile::Path(path) => return Ok(relative(&real(Path::new(&**path)), &self.folder)),
            ImageFile::Embedded(held) => held,
        };
        if let Some(name) = self.written.get(held.hash()) {
            return Ok(name.clone());
        }
        let path = write(held, &self.textures)?;
        let name = relative(&path, &self.folder);
        self.written.insert(*held.hash(), name.clone());
        Ok(name)
    }
}

/// `path` with the links of its longest existing part followed (a file's
/// canonical path, as the files a read names are given, with what isn't
/// there yet after it), so two spellings of one place (`/var` and
/// `/private/var` on macOS) meet.
fn real(path: &Path) -> std::borrow::Cow<'_, Path> {
    let mut rest = Vec::new();
    let mut at = path;
    loop {
        if let Ok(real) = at.canonicalize() {
            return std::borrow::Cow::Owned(rest.iter().rev().fold(real, |p, part| p.join(part)));
        }
        match (at.parent(), at.file_name()) {
            (Some(parent), Some(name)) => {
                rest.push(name);
                at = parent;
            }
            _ => return std::borrow::Cow::Borrowed(path),
        }
    }
}

/// `path` as a document in `folder` names it: relative to the folder (`..`
/// climbing out of it), `/` between its parts; a relative path as it is; an
/// absolute one no relative path reaches (another volume, or a folder named
/// relative) absolute.
fn relative(path: &Path, folder: &Path) -> String {
    let words = |p: &Path| -> Vec<String> {
        p.components()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect()
    };
    let rooted = |p: &Path| {
        matches!(
            p.components().next(),
            Some(Component::RootDir | Component::Prefix(_))
        )
    };
    if !rooted(path) {
        return words(path).join("/");
    }
    let (to, from) = (words(path), words(folder));
    let shared = to.iter().zip(&from).take_while(|(a, b)| a == b).count();
    if shared == 0 || !rooted(folder) {
        return path.display().to_string();
    }
    let up = std::iter::repeat_n("..".to_owned(), from.len() - shared);
    up.chain(to[shared..].iter().cloned())
        .collect::<Vec<_>>()
        .join("/")
}

/// `file` written into `folder` under its own name, `<stem> 2.<ext>`,
/// `<stem> 3.<ext>`… past a file there of other bytes; a file there of the
/// same bytes is kept. Where it is.
fn write(file: &EmbeddedFile, folder: &Path) -> Result<PathBuf, Error> {
    let failed =
        |path: &Path, e: std::io::Error| Error::invalid(format!("{}: {e}", path.display()));
    let named = Path::new(file.name());
    let stem = named
        .file_stem()
        .map_or_else(|| "image".to_owned(), |s| s.to_string_lossy().into_owned());
    let extension = named.extension().map(|e| e.to_string_lossy().into_owned());
    let at = |n: u32| {
        let stem = match n {
            1 => stem.clone(),
            n => format!("{stem} {n}"),
        };
        folder.join(match &extension {
            Some(e) => format!("{stem}.{e}"),
            None => stem,
        })
    };
    for n in 1.. {
        let path = at(n);
        match std::fs::read(&path) {
            Ok(there) if there == file.bytes() => return Ok(path),
            Ok(_) => continue,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                std::fs::create_dir_all(folder).map_err(|e| failed(folder, e))?;
                std::fs::write(&path, file.bytes()).map_err(|e| failed(&path, e))?;
                return Ok(path);
            }
            Err(e) => return Err(failed(&path, e)),
        }
    }
    unreachable!("a free name")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_are_relative_to_the_document_where_one_reaches() {
        let folder = Path::new("/work/out");
        assert_eq!(
            relative(Path::new("/work/out/wood.png"), folder),
            "wood.png"
        );
        assert_eq!(
            relative(Path::new("/work/maps/a b.png"), folder),
            "../maps/a b.png"
        );
        assert_eq!(
            relative(Path::new("maps/wood.png"), folder),
            "maps/wood.png"
        );
        assert_eq!(relative(Path::new("/x.png"), Path::new("/")), "x.png");
        // A document named by a relative path: an absolute file stays as
        // it is, never `//work/…`.
        assert_eq!(
            relative(Path::new("/work/maps/a.png"), Path::new("")),
            "/work/maps/a.png"
        );
    }

    #[test]
    fn a_document_named_relative_names_an_absolute_file_as_it_reads_back() {
        // A document in the working folder: its folder is "", whatever the
        // working folder is.
        let mut beside = Beside::new(Path::new("scene.usda"));
        let file = "/kinogaki-nowhere/maps/wood.png";
        let named = beside.name(&ImageFile::Path(file.into())).unwrap();
        assert_eq!(named, file);
    }

    #[test]
    fn a_file_reached_through_a_link_is_named_relative_to_the_document() {
        let dir = std::env::temp_dir().join(format!("kinogaki-beside-link-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("real/maps")).unwrap();
        std::fs::write(dir.join("real/maps/a.png"), [1]).unwrap();
        std::os::unix::fs::symlink(dir.join("real"), dir.join("link")).unwrap();
        let mut beside = Beside::new(&dir.join("link/look.mtl"));
        for path in ["real/maps/a.png", "link/maps/a.png", "link/maps/gone.png"] {
            let file = ImageFile::Path(dir.join(path).display().to_string().into());
            let name = Path::new(path).file_name().unwrap().to_string_lossy();
            assert_eq!(
                beside.name(&file).unwrap(),
                format!("maps/{name}"),
                "{path}"
            );
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn embedded_files_are_written_once_and_numbered_past_a_clash() {
        let dir = std::env::temp_dir().join(format!("kinogaki-beside-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut beside = Beside::new(&dir.join("look.mtlx"));
        let wood = ImageFile::Embedded(EmbeddedFile::new("wood.png", vec![1]));
        let again = ImageFile::Embedded(EmbeddedFile::new("copy.png", vec![1]));
        let other = ImageFile::Embedded(EmbeddedFile::new("wood.png", vec![2]));
        assert_eq!(beside.name(&wood).unwrap(), "look_textures/wood.png");
        assert_eq!(
            beside.name(&again).unwrap(),
            "look_textures/wood.png",
            "equal bytes"
        );
        assert_eq!(beside.name(&other).unwrap(), "look_textures/wood 2.png");
        // A second write finds its files there and keeps them.
        let mut next = Beside::new(&dir.join("look.mtlx"));
        assert_eq!(next.name(&other).unwrap(), "look_textures/wood 2.png");
        assert_eq!(
            std::fs::read(dir.join("look_textures/wood 2.png")).unwrap(),
            [2]
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
