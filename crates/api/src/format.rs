//! [`FileFormat`]: readers and writers of outside files.
//!
//! A format reads a file into [`Imported`] items carrying [`Data`] of the
//! types it declares. The environment turns the items into objects (File
//! ▸ Import, [`Purpose::Objects`]), or a File node takes the one value it
//! reads ([`Purpose::Node`]); a File node offers every format whose output
//! its port accepts. With no format registered, nothing can be imported,
//! and nothing breaks.
//!
//! **Options.** A format's options are [`Parameters`] by name with
//! defaults ([`FileFormat::options`]), shown by its [`Schema`]
//! ([`FileFormat::schema`]) and saved by the format itself
//! ([`FileFormat::write_options`]), so a File node's options and the
//! application's import defaults keep the format's own spelling. A
//! format's writer has options of its own ([`FormatWriter::options`],
//! [`FormatWriter::schema`]): writing never gets the reading options.
pub mod options;

use crate::cancel::CancelToken;
use crate::data::Data;
use crate::element::{ElementReader, ElementWriter, Field, FieldRef};
use crate::error::Error;
use crate::id::{DataTypeId, FormatId};
use crate::math::{Affine3, Units};
use crate::params::{Parameters, Schema, Value};
use std::any::Any;
use std::path::Path;

/// A format's options by name (`"scale"`, `"merge_shells"`). A format's
/// [`FileFormat::options`] gives every one with its default; reading and
/// writing get all of them.
pub type FormatOptions = Parameters;

/// One item of a file: a value or a group, named, placed, and under a
/// parent.
#[derive(Clone, Debug)]
pub struct FileItem {
    /// Its name in the file (a USD prim's, an OBJ group's); empty for an
    /// unnamed one, which takes the file's name.
    pub name: String,
    /// Its parent's index in the same list (always earlier); `None`: at
    /// the top.
    pub parent: Option<usize>,
    /// Its placement relative to its parent.
    pub placement: Affine3,
    /// Whether it shows (a hidden prim's object starts hidden).
    pub visible: bool,
    /// Its value; `None` for a group, which only holds other items.
    pub data: Option<Data>,
}

impl FileItem {
    /// A visible, unplaced item `name` at the top holding `data`.
    pub fn top(name: impl Into<String>, data: Data) -> Self {
        Self {
            name: name.into(),
            parent: None,
            placement: Affine3::IDENTITY,
            visible: true,
            data: Some(data),
        }
    }
}

/// What reading a file produced.
#[derive(Clone, Debug, Default)]
pub struct Imported {
    /// The file's items, parents before their children.
    pub items: Vec<FileItem>,
    /// What was read with loss or skipped, for the user.
    pub warnings: Vec<String>,
}

/// What the host makes of a read.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Purpose {
    /// File ▸ Import: the file's items become objects, so options that
    /// shape them (keeping a hierarchy, a piece per object) apply.
    Objects,
    /// A File node: the file is one value of the node's port type, so the
    /// format reads it as one item and its schema leaves those options out.
    Node,
}

/// What a read is for and what it reads into, besides the format's
/// options.
#[derive(Clone)]
pub struct Reading<'a> {
    /// What the host makes of the items.
    pub purpose: Purpose,
    /// The project's units: lengths in the file are converted into them.
    pub units: Units,
    /// A number naming what the read is for (a File node's id), which a
    /// format may seed the names of what it reads with, so two nodes
    /// reading one file give values told apart.
    pub origin: u64,
    /// Values of the project the host lends formats, found by their Rust
    /// type ([`Reading::setting`]): a CAD kernel's tolerances.
    pub settings: &'a [&'a (dyn Any + Send + Sync)],
    /// Raised once the read is no longer wanted; long reads poll it
    /// ([`Reading::check`]) and stop with [`Error::Cancelled`].
    pub cancel: CancelToken,
    /// The data type the host wants the file read as, for
    /// [`Purpose::Node`]: the File node's output (a STEP file as B-rep or
    /// as polygons); `None` for Import, which takes any.
    pub wants: Option<DataTypeId>,
}

impl<'a> Reading<'a> {
    /// A read for `purpose` into `units`, of origin 0, lent nothing and
    /// never cancelled.
    pub fn new(purpose: Purpose, units: Units) -> Self {
        Self {
            purpose,
            units,
            origin: 0,
            settings: &[],
            cancel: CancelToken::default(),
            wants: None,
        }
    }

    /// The lent value of type `T`, when the host lends one.
    pub fn setting<T: Any>(&self) -> Option<&'a T> {
        self.settings
            .iter()
            .find_map(|s| (*s as &dyn Any).downcast_ref())
    }

    /// `Err(Error::Cancelled)` once the read is no longer wanted.
    pub fn check(&self) -> Result<(), Error> {
        self.cancel.check()
    }
}

/// A file format a package reads (and maybe writes).
pub trait FileFormat: Send + Sync {
    /// Its id (`kinogaki.format.obj`).
    fn id(&self) -> FormatId;

    /// What file dialogs call it ("Wavefront OBJ").
    fn label(&self) -> &str;

    /// Its file name extensions, lowercase and without the dot (`obj`).
    fn extensions(&self) -> &[&str];

    /// Whether a file starting with `head` (its first bytes, up to 64) is
    /// of this format, for a file whose extension names no format; no by
    /// default.
    fn detects(&self, head: &[u8]) -> bool {
        let _ = head;
        false
    }

    /// The data types its items hold; empty for a format that only writes
    /// (G-code), which nothing reads with.
    fn produces(&self) -> &[DataTypeId];

    /// Every option with its default value; none by default.
    fn options(&self) -> FormatOptions {
        FormatOptions::new()
    }

    /// The options dialog for `options` when read for `purpose`: a
    /// parameter per option that applies, named as the option and holding
    /// its current value; empty (no dialog) by default.
    fn schema(&self, options: &FormatOptions, purpose: Purpose) -> Schema {
        let _ = (options, purpose);
        Schema::default()
    }

    /// Writes `options` into the element `out` fills. By default each
    /// option is a field named as it: a number a real, a toggle a boolean,
    /// a choice its index, text as text, numbers and texts arrays (an
    /// option holds no data).
    fn write_options(
        &self,
        options: &FormatOptions,
        out: &mut dyn ElementWriter,
    ) -> Result<(), Error> {
        for (name, value) in options {
            let field = match value {
                Value::Number(v) => Field::Float(*v),
                Value::Numbers(v) => Field::F64s(v.clone()),
                Value::Bool(v) => Field::Bool(*v),
                Value::Index(v) => Field::Int(*v as i64),
                Value::Text(v) => Field::Text(v.clone()),
                Value::Texts(v) => Field::Texts(v.clone()),
                Value::Data(_) => {
                    return Err(Error::invalid(format_args!(
                        "option {name}: a format's options hold no data"
                    )));
                }
            };
            out.set(name, field)?;
        }
        Ok(())
    }

    /// Reads the options [`write_options`](Self::write_options) wrote into
    /// `element`, strictly: every option, each of its default's type.
    fn read_options(&self, element: &dyn ElementReader) -> Result<FormatOptions, Error> {
        let mut options = self.options();
        for (name, value) in &mut options {
            let wrong = || {
                Error::invalid(format_args!(
                    "{}: option {name} is not of its type",
                    element.location()
                ))
            };
            *value = match (&*value, element.required(name)?) {
                (Value::Number(_), FieldRef::Float(v)) => Value::Number(v),
                (Value::Numbers(_), FieldRef::F64s(v)) => Value::Numbers(v.to_vec()),
                (Value::Bool(_), FieldRef::Bool(v)) => Value::Bool(v),
                (Value::Index(_), FieldRef::Int(v)) => {
                    Value::Index(usize::try_from(v).map_err(|_| wrong())?)
                }
                (Value::Text(_), FieldRef::Text(v)) => Value::Text(v.to_owned()),
                (Value::Texts(_), FieldRef::Texts(v)) => Value::Texts(v.to_vec()),
                _ => return Err(wrong()),
            };
        }
        Ok(options)
    }

    /// Reads the file at `path` with `options`, as `reading` says.
    fn read(
        &self,
        path: &Path,
        options: &FormatOptions,
        reading: &Reading<'_>,
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

    /// Every option of its writing with its default (a STEP schema, AP214
    /// or AP242): its own, apart from its format's reading options; none by
    /// default.
    fn options(&self) -> FormatOptions {
        FormatOptions::new()
    }

    /// The Export dialog for `options`: a parameter per option, named as
    /// the option and holding its current value; empty (no dialog) by
    /// default.
    fn schema(&self, options: &FormatOptions) -> Schema {
        let _ = options;
        Schema::default()
    }

    /// Whether it writes a range of frames ([`Writing::frames`]); a writer
    /// that doesn't is handed one time's items and no frames. No by
    /// default.
    fn frames(&self) -> bool {
        false
    }

    /// Writes `items` (parents before children) to `path` as `writing`
    /// says, with `options`: every one of [`options`](Self::options), each
    /// set or at its default. Returns what was written with loss or
    /// skipped, for the user.
    fn write(
        &self,
        path: &Path,
        items: &[FileItem],
        writing: &Writing<'_>,
        options: &FormatOptions,
    ) -> Result<Vec<String>, Error>;
}

/// What a write is for, besides the writer's options.
#[derive(Clone)]
pub struct Writing<'a> {
    /// The project's units, which the items' lengths are in.
    pub units: Units,
    /// Raised once the write is no longer wanted; long writes poll it
    /// ([`Writing::check`]) and stop with [`Error::Cancelled`], and so does
    /// the work they ask of the host ([`meshed`](Self::meshed)).
    pub cancel: CancelToken,
    /// An item's value as polygons, for a writer that writes only those
    /// (a B-rep through the installed mesher); `None` when nothing meshes
    /// it.
    pub meshed: &'a (dyn Fn(&Data) -> Option<kinogaki_geometry::Polygons> + Sync),
    /// The range being written, for a writer that writes
    /// [`frames`](FormatWriter::frames); `None` writes `items` alone.
    pub frames: Option<&'a dyn Frames>,
}

impl<'a> Writing<'a> {
    /// A write in `units`, never cancelled, meshing nothing, of one time.
    pub fn new(units: Units) -> Self {
        Self {
            units,
            cancel: CancelToken::default(),
            meshed: &|_| None,
            frames: None,
        }
    }

    /// `Err(Error::Cancelled)` once the write is no longer wanted.
    pub fn check(&self) -> Result<(), Error> {
        self.cancel.check()
    }
}

/// A range of frames a writer writes ([`Writing::frames`]): the items at
/// each, keeping their structure (the same items, parents and names) from
/// frame to frame.
pub trait Frames: Sync {
    /// Frames a second.
    fn fps(&self) -> f64;
    /// How many frames.
    fn len(&self) -> usize;
    /// Whether it holds no frame.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// The items at frame `index` (from 0), cooked at its time.
    fn items_at(&self, index: usize) -> Result<Vec<FileItem>, Error>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::element::MemoryElement;
    use crate::math::UpAxis;

    struct Plain;
    impl FileFormat for Plain {
        fn id(&self) -> FormatId {
            FormatId::declared("acme.format.plain")
        }
        fn label(&self) -> &str {
            "Plain"
        }
        fn extensions(&self) -> &[&str] {
            &["plain"]
        }
        fn produces(&self) -> &[DataTypeId] {
            &[]
        }
        fn options(&self) -> FormatOptions {
            [
                ("scale".to_owned(), Value::Number(1.0)),
                ("normals".to_owned(), Value::Bool(true)),
                ("up".to_owned(), Value::Index(1)),
                ("prims".to_owned(), Value::Text(String::new())),
                ("color".to_owned(), Value::Numbers(vec![0.5; 3])),
            ]
            .into()
        }
        fn read(&self, _: &Path, _: &FormatOptions, r: &Reading<'_>) -> Result<Imported, Error> {
            r.check()?;
            Ok(Imported::default())
        }
    }

    #[test]
    fn options_save_and_read_back_by_default() {
        let mut options = Plain.options();
        options.insert("scale".into(), Value::Number(2.5));
        options.insert("prims".into(), Value::Text("/World".into()));
        let mut element = MemoryElement::new("plain");
        element.write(|w| Plain.write_options(&options, w)).unwrap();
        assert_eq!(element.read(|r| Plain.read_options(r)).unwrap(), options);
        // Strict: a missing option, or one of another type, fails.
        let mut short = MemoryElement::new("plain");
        short
            .write(|w| w.set("scale", Field::Text("2".into())))
            .unwrap();
        assert!(short.read(|r| Plain.read_options(r)).is_err());
    }

    #[test]
    fn a_writer_has_no_options_by_default_whatever_its_reading_has() {
        struct Writes;
        impl FormatWriter for Writes {
            fn accepts(&self) -> &[DataTypeId] {
                &[]
            }
            fn write(
                &self,
                _: &Path,
                _: &[FileItem],
                _: &Writing<'_>,
                _: &FormatOptions,
            ) -> Result<Vec<String>, Error> {
                Ok(Vec::new())
            }
        }
        assert!(!Plain.options().is_empty());
        assert!(Writes.options().is_empty());
        assert!(
            Writes
                .schema(&Plain.options())
                .parameters()
                .next()
                .is_none()
        );
    }

    #[test]
    fn readings_lend_settings_by_type_and_poll_cancellation() {
        let tolerance = 0.01_f64;
        let lent: [&(dyn Any + Send + Sync); 1] = [&tolerance];
        let stop = CancelToken::default();
        stop.cancel();
        let units = Units {
            up: UpAxis::Z,
            meters_per_unit: 0.001,
        };
        let reading = Reading {
            settings: &lent,
            cancel: stop,
            ..Reading::new(Purpose::Node, units)
        };
        assert_eq!(reading.setting::<f64>(), Some(&0.01));
        assert_eq!(reading.setting::<u32>(), None);
        let read = Plain.read(Path::new("a.plain"), &Plain.options(), &reading);
        assert_eq!(read.unwrap_err(), Error::Cancelled);
        let fine = Reading::new(Purpose::Objects, Units::default());
        assert!(
            Plain
                .read(Path::new("a.plain"), &Plain.options(), &fine)
                .is_ok()
        );
    }
}
