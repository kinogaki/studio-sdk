//! [`FileFormat`]: readers and writers of outside files.
//!
//! A format reads a file, on disk or held in memory ([`Input`]), into
//! [`Imported`] items carrying [`Data`] of the types it declares. The
//! environment turns the items into objects (File ▸ Import,
//! [`Purpose::Objects`]), makes a document of them (File ▸ Open,
//! [`Purpose::Document`]: lengths in the file's own units), or takes the
//! one value a file is ([`Purpose::Value`]: a File node's output, an image
//! the host's image store reads); a File node offers every format whose
//! output its port accepts. With no format registered, nothing can be
//! imported, and nothing breaks.
//!
//! **Options.** A format's options are [`Parameters`] by name with
//! defaults ([`FileFormat::options`]), shown by its [`Schema`]
//! ([`FileFormat::schema`]). A format's writer has options of its own
//! ([`FormatWriter::options`], [`FormatWriter::schema`]): writing never
//! gets the reading options. A node holding a reader's or a writer's
//! options holds each as a row named by [`options::row`]; a host keeping
//! them elsewhere (the application's settings) saves each as a field named
//! as it ([`options::write`], [`options::read`]).
//!
//! **Images.** A format of pictures reads a file as one item holding one
//! standard [`Image`](kinogaki_geometry::image::Image)
//! ([`standard::image`](crate::standard::image)), as the file holds it,
//! and converts nothing:
//! - its samples as stored, each channel group a layer (several images
//!   in one file, an OpenEXR's parts, are the layers of one Image);
//! - its color layers tagged as the file says
//!   ([`ColorTag`](kinogaki_geometry::image::ColorTag): a space's name, an
//!   ICC profile, primaries, else unknown, linear for float samples), with
//!   its alpha straight or premultiplied as the file has it; its data
//!   layers raw;
//! - every attribute the file carries as the Image's metadata, beside the
//!   standard names ([`metadata`](kinogaki_geometry::image::metadata)).
//!
//! The host resolves the tag to a space of its configuration and converts
//! (a format holds no color maths). A writer of pictures takes one item
//! holding an Image the host made for the file (as a display shows it when
//! the writer is [`display_referred`](FormatWriter::display_referred), else
//! linear light) and tagged for it, copies the tag (a space's primaries, a
//! file that holds them takes from the host's
//! [`primaries`](crate::color::ColorService::primaries)), writes each layer
//! as the file can hold it, and says, as a warning, what it left out.
//!
//! **Untrusted files.** A file may be hostile: a reader never trusts what
//! it says. Every size, count or depth it claims is checked against the
//! read's [`ReadBudget`] ([`Reading::budget`]) before anything is
//! allocated or followed, and a decoder checks its output against its input
//! ([`ReadBudget::expand`]). Recursion is bounded by the budget's depth, so
//! it holds on the 2 MiB stack a read runs on; references are followed
//! once each, by the identity of what they name. The files a file names
//! are found and read through [`files::Named`] alone: wherever they are,
//! regular files only, within the budget. What a file asks
//! beyond its budget is an error naming the file; what a reader left out, a
//! warning. A read never panics, overflows its stack or aborts on a failed
//! allocation.
mod budget;
mod conversion;
pub mod files;
mod input;
pub mod materialx;
pub mod options;
mod unique;

pub use budget::{ReadBudget, ReadLimits};
pub use conversion::Conversion;
pub use input::Input;
pub use unique::Unique;

use crate::cancel::CancelToken;
use crate::color::ColorService;
use crate::data::Data;
use crate::id::{DataTypeId, FormatId};
use crate::params::{Parameters, Schema};
use crate::{Diagnostic, Error};
use kinogaki_geometry::{Units, Xform};
use std::path::Path;

/// A format's options by name (`"scale"`, `"merge_shells"`). A format's
/// [`FileFormat::options`] gives every one with its default; reading and
/// writing get all of them.
pub type FormatOptions = Parameters;

/// One item of a file: a value or a group, named, placed, and under a
/// parent. A value naming another item of the same list (a portal's dome,
/// a geometry light's mesh: an [`ObjectRef`](crate::ObjectRef) in a light
/// value) names it by its index in the list, both ways: the host names the
/// objects made of them on Import, and the items written of the objects
/// named on Export.
#[derive(Clone, Debug)]
pub struct FileItem {
    /// Its name in the file (a USD prim's, an OBJ group's); empty for an
    /// unnamed one, which takes the file's name.
    pub name: String,
    /// Its parent's index in the same list (always earlier); `None`: at
    /// the top.
    pub parent: Option<usize>,
    /// Its placement relative to its parent.
    pub placement: Xform,
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
            placement: Xform::IDENTITY,
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
    /// What was read with loss or skipped, for the user: warnings.
    pub warnings: Vec<Diagnostic>,
    /// For a [`Purpose::Document`] read, the units its items' lengths are
    /// in: the file's own ([`Reading::document_units`]), or `None` for a
    /// file stating none, whose lengths are in [`Reading::units`]. `None`
    /// for the other purposes, whose lengths are in [`Reading::units`].
    pub units: Option<Units>,
    /// What the file says of itself that no item holds (a USD layer's
    /// `defaultPrim`, `doc` and time codes; the unit, up axis and MTL name an
    /// OBJ was read with), by name, in the format's own spelling (a value
    /// only text can hold as [`Value::Text`](crate::Value::Text)): what this
    /// format's writer takes back ([`Writing::metadata`]) when it writes the
    /// document read from the file again. Empty when there is none.
    pub metadata: FormatOptions,
    /// Whether the items flatten what other files gave this one (a USD
    /// layer's references, payloads and sublayers to files outside it): a
    /// write over the file would put their content in it, so Studio saves
    /// a document read from it elsewhere.
    pub flattened: bool,
}

/// What the host makes of a read.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Purpose {
    /// File ▸ Import: the file's items become objects, so options that
    /// shape them (keeping a hierarchy, a piece per object) apply.
    Objects,
    /// File ▸ Open: the file's items become a document of their own, as
    /// [`Objects`](Self::Objects) do, with their lengths in the units the
    /// file states, or those its options say it is in, and its axis not
    /// turned ([`Reading::document_units`]); [`Imported::units`] says
    /// which. Its schema lists the options that apply, leaving out those
    /// that change the file's content (a scale, embedding the files it
    /// names): the host reads with those at their defaults.
    Document,
    /// One value of the type the host wants ([`Reading::wants`]): a File
    /// node's output, an image of the host's image store. The format reads
    /// the file as one item, and its schema leaves the options shaping
    /// objects out.
    Value,
}

/// What a read is for and what it reads into, besides the format's
/// options.
#[derive(Clone)]
pub struct Reading<'a> {
    /// What the host makes of the items.
    pub purpose: Purpose,
    /// The project's units: lengths in the file are converted into them,
    /// except for a [`Purpose::Document`] read of a file stating its own
    /// ([`Reading::document_units`]).
    pub units: Units,
    /// The host's color management: colors in the file are converted
    /// from the file's space into the project's working space.
    pub colors: &'a dyn ColorService,
    /// What the read may spend, and its cancellation: what the file claims
    /// is checked against it, and long reads poll it ([`Reading::check`])
    /// and stop with [`Error::cancelled()`].
    pub budget: ReadBudget,
    /// The data type the host wants the file read as, for
    /// [`Purpose::Value`]: a File node's output (a STEP file as B-rep or
    /// as polygons); `None` for Import, which takes any.
    pub wants: Option<DataTypeId>,
}

impl<'a> Reading<'a> {
    /// A read for `purpose` into `units` and `colors`' working space, of
    /// the default budget and never cancelled, wanting any type.
    pub fn new(purpose: Purpose, units: Units, colors: &'a dyn ColorService) -> Self {
        Self {
            purpose,
            units,
            colors,
            budget: ReadBudget::default(),
            wants: None,
        }
    }

    /// `Err(Error::cancelled())` once the read is no longer wanted.
    pub fn check(&self) -> Result<(), Error> {
        self.budget.check()
    }

    /// The units of a file stating `file` (its own, or those its options
    /// say it is in) when this read keeps them: `Some(file)` for a
    /// [`Purpose::Document`] read, which reports them as
    /// [`Imported::units`]; `None` for the others, whose lengths go into
    /// [`Reading::units`]. A format converts into
    /// `document_units(file).unwrap_or(reading.units)`.
    ///
    /// ```
    /// use kinogaki_studio_api::geometry::{Units, UpAxis};
    /// use kinogaki_studio_api::{Purpose, Reading};
    /// # let colors = kinogaki_studio_api_testing::Unconverted;
    ///
    /// let centimetres = Units::new(0.01, UpAxis::Z).unwrap();
    /// let open = Reading::new(Purpose::Document, Units::MILLIMETERS, &colors);
    /// assert_eq!(open.document_units(centimetres), Some(centimetres));
    /// let import = Reading::new(Purpose::Objects, Units::MILLIMETERS, &colors);
    /// assert_eq!(import.document_units(centimetres), None);
    /// ```
    pub fn document_units(&self, file: Units) -> Option<Units> {
        (self.purpose == Purpose::Document).then_some(file)
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

    /// Whether a file starting with `head` (its first bytes, up to 64:
    /// [`Input::head`]) is of this format, for a file whose extension names
    /// no format; no by default.
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

    /// The rows for `options` when read for `purpose` (Studio shows
    /// [`Purpose::Objects`]' on Settings ▸ File Formats, which File ▸
    /// Import and File ▸ Open both read with, and a File node shows
    /// [`Purpose::Value`]'): a parameter per option that applies, named as
    /// the option and holding its current value; empty (no rows) by
    /// default. An option [`Purpose::Document`]'s rows leave out is read
    /// at its default on File ▸ Open.
    fn schema(&self, options: &FormatOptions, purpose: Purpose) -> Schema {
        let _ = (options, purpose);
        Schema::default()
    }

    /// Reads `input` with `options`, as `reading` says. Called on a
    /// worker; a message about the file names it as `input` displays.
    fn read(
        &self,
        input: &Input<'_>,
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

    /// The rows for `options` (Studio shows them on Settings ▸ Export): a
    /// parameter per option, named as the option and holding its current
    /// value; empty (no rows) by default.
    fn schema(&self, options: &FormatOptions) -> Schema {
        let _ = options;
        Schema::default()
    }

    /// Whether its files hold display-referred code values (a PNG's, a
    /// JPEG's) rather than scene-linear light (an OpenEXR's): a host hands
    /// such a writer an image as a display shows it, through the view a
    /// render is seen in, tagged with that display. No by default.
    fn display_referred(&self) -> bool {
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
    ) -> Result<Vec<Diagnostic>, Error>;
}

/// What a write is for, besides the writer's options.
#[derive(Clone)]
pub struct Writing<'a> {
    /// The project's units, which the items' lengths are in.
    pub units: Units,
    /// The host's color management: the items' colors are in its working
    /// space, converted into the file's.
    pub colors: &'a dyn ColorService,
    /// Raised once the write is no longer wanted; long writes poll it
    /// ([`Writing::check`]) and stop with [`Error::cancelled()`], and so does
    /// the work they ask of the host ([`meshed`](Self::meshed)).
    pub cancel: CancelToken,
    /// An item's value as polygons, for a writer that writes only those
    /// (a B-rep through the installed mesher); `None` when nothing meshes
    /// it.
    pub meshed: &'a (dyn Fn(&Data) -> Option<kinogaki_geometry::Geometry> + Sync),
    /// The metadata this format read from the file the document came from
    /// ([`Imported::metadata`]), when this format writes the document
    /// again; `None` otherwise. A writer uses what still applies (a
    /// `defaultPrim` naming no top item doesn't) and says what it left out.
    pub metadata: Option<&'a FormatOptions>,
}

impl<'a> Writing<'a> {
    /// A write in `units` and `colors`' working space, never cancelled,
    /// meshing nothing, of no metadata.
    pub fn new(units: Units, colors: &'a dyn ColorService) -> Self {
        Self {
            units,
            colors,
            cancel: CancelToken::default(),
            meshed: &|_| None,
            metadata: None,
        }
    }

    /// `Err(Error::cancelled())` once the write is no longer wanted.
    pub fn check(&self) -> Result<(), Error> {
        self.cancel.check()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::ColorService;
    use crate::params::Value;
    use kinogaki_geometry::UpAxis;

    /// A color service converting nothing (the readings here read no color).
    #[derive(Debug)]
    struct Unconverted;
    impl ColorService for Unconverted {
        fn working(&self) -> &str {
            kinogaki_render_api::RENDER_SPACE
        }
        fn convert(&self, _: &str, _: &str, _: &mut [[f64; 3]]) -> Result<(), Error> {
            Ok(())
        }
        fn revision(&self) -> u64 {
            0
        }
    }

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
                ("up".to_owned(), Value::Choice("y".into())),
                ("prims".to_owned(), Value::Text(String::new())),
                ("color".to_owned(), Value::Color([0.5; 3])),
                ("count".to_owned(), Value::Integer(3)),
            ]
            .into()
        }
        fn read(
            &self,
            _: &Input<'_>,
            _: &FormatOptions,
            r: &Reading<'_>,
        ) -> Result<Imported, Error> {
            r.check()?;
            Ok(Imported::default())
        }
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
            ) -> Result<Vec<Diagnostic>, Error> {
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
    fn readings_poll_cancellation() {
        let stop = CancelToken::default();
        stop.cancel();
        let units = Units::new(0.001, UpAxis::Z).unwrap();
        let reading = Reading {
            budget: ReadBudget::default().cancelled_by(stop),
            ..Reading::new(Purpose::Value, units, &Unconverted)
        };
        let input = Input::memory("a.plain", b"");
        let read = Plain.read(&input, &Plain.options(), &reading);
        assert_eq!(read.unwrap_err(), Error::cancelled());
        let fine = Reading::new(Purpose::Objects, Units::default(), &Unconverted);
        assert!(Plain.read(&input, &Plain.options(), &fine).is_ok());
    }
}
