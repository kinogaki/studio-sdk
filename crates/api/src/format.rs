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
//! application's import defaults keep the format's own spelling.
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
#[derive(Clone, Copy)]
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
    /// Whether the read is no longer wanted; long reads poll it and stop
    /// with [`Error::Cancelled`].
    pub cancelled: &'a (dyn Fn() -> bool + Sync),
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
            cancelled: &|| false,
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
        if (self.cancelled)() {
            Err(Error::Cancelled)
        } else {
            Ok(())
        }
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

    /// The data types its items hold.
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
    /// a choice its index, text as text, numbers and texts arrays.
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

    /// Writes `items` (parents before children), whose lengths are in
    /// `units`, to `path` with `options`.
    fn write(
        &self,
        path: &Path,
        items: &[FileItem],
        units: Units,
        options: &FormatOptions,
    ) -> Result<(), Error>;
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
    fn readings_lend_settings_by_type_and_poll_cancellation() {
        let tolerance = 0.01_f64;
        let lent: [&(dyn Any + Send + Sync); 1] = [&tolerance];
        let stop = || true;
        let units = Units {
            up: UpAxis::Z,
            meters_per_unit: 0.001,
        };
        let reading = Reading {
            settings: &lent,
            cancelled: &stop,
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
