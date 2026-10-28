//! [`FileFormat`]: readers and writers of outside files.
//!
//! A format reads a file into [`Imported`] items carrying [`Data`] of the
//! types it declares; the environment then gives each value to the first
//! object kind that [adopts](crate::ObjectKindSpec::adopts) it, and a File
//! node offers every format whose output its context accepts. With no
//! format registered, nothing can be imported, and nothing breaks.
use crate::cancel::CancelToken;
use crate::data::Data;
use crate::error::Error;
use crate::id::{DataTypeId, FormatId};
use crate::math::Affine3;
use crate::params::{Parameters, Schema};
use std::path::Path;

/// A format's options by name (`"scale"`, `"merge_shells"`). A format's
/// [`FileFormat::options`] gives every one with its default; reading and
/// writing get all of them.
pub type FormatOptions = Parameters;

/// One item of a file: a value or a group, named, placed, and under a
/// parent.
#[derive(Clone, Debug)]
pub struct FileItem {
    /// Its name in the file (a USD prim's, an OBJ group's).
    pub name: String,
    /// Its parent's index in the same list (always earlier); `None`: at
    /// the top.
    pub parent: Option<usize>,
    /// Its placement relative to its parent.
    pub placement: Affine3,
    /// Its value; `None` for a group, which only holds other items.
    pub data: Option<Data>,
}

/// What reading a file produced.
#[derive(Clone, Debug, Default)]
pub struct Imported {
    /// The file's items, parents before their children.
    pub items: Vec<FileItem>,
    /// What was read with loss or skipped, for the user.
    pub warnings: Vec<String>,
}

/// A file format a package reads (and maybe writes).
pub trait FileFormat: Send + Sync {
    /// Its id (`kinogaki.format.obj`).
    fn id(&self) -> FormatId;

    /// What file dialogs call it ("Wavefront OBJ").
    fn label(&self) -> &str;

    /// Its file name extensions, lowercase and without the dot (`obj`).
    fn extensions(&self) -> &[&str];

    /// The data types its items hold.
    fn produces(&self) -> &[DataTypeId];

    /// Every option with its default value; none by default.
    fn options(&self) -> FormatOptions {
        FormatOptions::new()
    }

    /// The import dialog for `options`: a parameter per option, named as
    /// the option and holding its current value; empty (no dialog) by
    /// default.
    fn schema(&self, options: &FormatOptions) -> Schema {
        let _ = options;
        Schema::default()
    }

    /// Reads the file at `path` with `options`. Long reads poll `cancel`.
    fn read(
        &self,
        path: &Path,
        options: &FormatOptions,
        cancel: &CancelToken,
    ) -> Result<Imported, Error>;

    /// Its writer, when it writes files too.
    fn writer(&self) -> Option<&dyn FormatWriter> {
        None
    }
}

/// Writes files of a format.
pub trait FormatWriter: Send + Sync {
    /// The data types it can write; other items' values are skipped (their
    /// groups are kept).
    fn accepts(&self) -> &[DataTypeId];

    /// Writes `items` (parents before children) to `path` with `options`.
    fn write(
        &self,
        path: &Path,
        items: &[FileItem],
        options: &FormatOptions,
        cancel: &CancelToken,
    ) -> Result<(), Error>;
}
