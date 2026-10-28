//! Saving into a project: the element tree codecs write and read.
//!
//! A project file is a tree of **elements**. Each element has a kind, typed
//! fields (scalars and flat numeric or text arrays), text metadata and
//! named child elements. A codec ([`DataCodec`](crate::DataCodec),
//! [`ObjectCodec`](crate::ObjectCodec)) fills the element the environment
//! gives it through an [`ElementWriter`], and reads it back through an
//! [`ElementReader`]. Packages never see the host's file format: Studio
//! implements these traits over its project documents, and
//! [`MemoryElement`] implements them in memory for package tests.
//!
//! **Names.** Field names, metadata keys and child names are
//! [`is_name`]s: ASCII letters, digits, `_` and `-`, not starting with `-`.
//! A writer refuses any other name.
//!
//! **Shared values.** Two places may hold one value (two frozen nodes
//! keeping the same mesh). [`ElementWriter::share`] writes it once per save
//! and makes later places refer to it, and
//! [`read_shared`](trait.ElementReader.html#method.read_shared) reads it
//! once per load and hands every place the same `Arc`.
//!
//! **Strictness.** Readers fail on anything missing or of the wrong type,
//! naming the field and the element ([`ElementReader::location`]); there
//! are no defaults for missing values and no version fields.
mod memory;

pub use memory::MemoryElement;

use crate::error::Error;
use std::any::Any;
use std::sync::Arc;

/// A shared value as [`ElementReader::shared_any`] keeps it.
pub type SharedValue = Arc<dyn Any + Send + Sync>;

/// What builds a shared value from the element it was written to.
pub type ReadShared<'f> = dyn FnMut(&dyn ElementReader) -> Result<SharedValue, Error> + 'f;

/// Whether `text` may name a field, a metadata key or a child element:
/// non-empty ASCII letters, digits, `_` and `-`, not starting with `-`.
///
/// ```
/// use kinogaki_studio_api::element::is_name;
/// assert!(is_name("points") && is_name("output_1") && is_name("P"));
/// assert!(!is_name("") && !is_name("-x") && !is_name("a.b") && !is_name("a b"));
/// ```
pub fn is_name(text: &str) -> bool {
    !text.is_empty()
        && !text.starts_with('-')
        && text
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

/// An [`Error::Invalid`] unless `name` [`is_name`].
pub fn check_name(name: &str) -> Result<(), Error> {
    if is_name(name) {
        Ok(())
    } else {
        Err(Error::invalid(format_args!(
            "{name:?} is not a valid element, field or metadata name"
        )))
    }
}

/// A field's value as written: a scalar or a flat array.
#[derive(Clone, Debug, PartialEq)]
pub enum Field {
    /// A boolean.
    Bool(bool),
    /// A whole number.
    Int(i64),
    /// A real number.
    Float(f64),
    /// Text.
    Text(String),
    /// Bytes (flags, small codes).
    U8s(Vec<u8>),
    /// Unsigned 32-bit numbers (indices, counts).
    U32s(Vec<u32>),
    /// Signed 32-bit numbers.
    I32s(Vec<i32>),
    /// Signed 64-bit numbers.
    I64s(Vec<i64>),
    /// Single-precision reals.
    F32s(Vec<f32>),
    /// Double-precision reals (positions).
    F64s(Vec<f64>),
    /// Texts (names).
    Texts(Vec<String>),
}

impl Field {
    /// How many values it holds: 1 for a scalar.
    pub fn len(&self) -> usize {
        self.as_ref().len()
    }

    /// Whether it is an empty array.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The field borrowed.
    pub fn as_ref(&self) -> FieldRef<'_> {
        match self {
            Self::Bool(v) => FieldRef::Bool(*v),
            Self::Int(v) => FieldRef::Int(*v),
            Self::Float(v) => FieldRef::Float(*v),
            Self::Text(v) => FieldRef::Text(v),
            Self::U8s(v) => FieldRef::U8s(v),
            Self::U32s(v) => FieldRef::U32s(v),
            Self::I32s(v) => FieldRef::I32s(v),
            Self::I64s(v) => FieldRef::I64s(v),
            Self::F32s(v) => FieldRef::F32s(v),
            Self::F64s(v) => FieldRef::F64s(v),
            Self::Texts(v) => FieldRef::Texts(v),
        }
    }
}

/// A field's value as read, borrowed from the reader: the same variants as
/// [`Field`].
#[derive(Clone, Copy, Debug, PartialEq)]
#[allow(missing_docs)]
pub enum FieldRef<'a> {
    Bool(bool),
    Int(i64),
    Float(f64),
    Text(&'a str),
    U8s(&'a [u8]),
    U32s(&'a [u32]),
    I32s(&'a [i32]),
    I64s(&'a [i64]),
    F32s(&'a [f32]),
    F64s(&'a [f64]),
    Texts(&'a [String]),
}

impl FieldRef<'_> {
    /// How many values it holds: 1 for a scalar.
    pub fn len(&self) -> usize {
        match self {
            Self::Bool(_) | Self::Int(_) | Self::Float(_) | Self::Text(_) => 1,
            Self::U8s(v) => v.len(),
            Self::U32s(v) => v.len(),
            Self::I32s(v) => v.len(),
            Self::I64s(v) => v.len(),
            Self::F32s(v) => v.len(),
            Self::F64s(v) => v.len(),
            Self::Texts(v) => v.len(),
        }
    }

    /// Whether it is an empty array.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The owned field.
    pub fn to_owned(&self) -> Field {
        match *self {
            Self::Bool(v) => Field::Bool(v),
            Self::Int(v) => Field::Int(v),
            Self::Float(v) => Field::Float(v),
            Self::Text(v) => Field::Text(v.to_owned()),
            Self::U8s(v) => Field::U8s(v.to_vec()),
            Self::U32s(v) => Field::U32s(v.to_vec()),
            Self::I32s(v) => Field::I32s(v.to_vec()),
            Self::I64s(v) => Field::I64s(v.to_vec()),
            Self::F32s(v) => Field::F32s(v.to_vec()),
            Self::F64s(v) => Field::F64s(v.to_vec()),
            Self::Texts(v) => Field::Texts(v.to_vec()),
        }
    }
}

/// Fills one element: what a codec writes through.
///
/// The environment creates the element (its kind and place) before handing
/// the writer to a codec; the codec sets fields and metadata and adds
/// children. Setting a field or metadata key twice keeps the last value.
pub trait ElementWriter {
    /// Sets field `name` to `field`.
    fn set(&mut self, name: &str, field: Field) -> Result<(), Error>;

    /// Sets field `name` to the array `field` laid out as rows of `width`
    /// values (points as rows of 3), so the file shows its shape. The
    /// array's length must be a multiple of `width`; reading gives the flat
    /// array back.
    fn set_rows(&mut self, name: &str, width: u32, field: Field) -> Result<(), Error>;

    /// Sets metadata `key` to `value`.
    fn set_metadata(&mut self, key: &str, value: &str) -> Result<(), Error>;

    /// Adds child element `name` of `kind` (an [`is_name`]) and returns its
    /// writer. Adding a child that exists is an error.
    fn child(&mut self, name: &str, kind: &str) -> Result<Box<dyn ElementWriter + '_>, Error>;

    /// Writes a value that other places may share as child `name` of
    /// `kind`. `key` identifies the value for this save (an `Arc`'s
    /// address, a content hash): the first `share` with a key calls `write`
    /// with the new child's writer; a later one with the same key writes
    /// only a reference to that child, and `write` is not called.
    fn share(
        &mut self,
        name: &str,
        kind: &str,
        key: u64,
        write: &mut dyn FnMut(&mut dyn ElementWriter) -> Result<(), Error>,
    ) -> Result<(), Error>;
}

/// Reads one element: what a codec reads through.
pub trait ElementReader {
    /// The element's kind.
    fn kind(&self) -> &str;

    /// Field `name`, when it is set.
    fn field(&self, name: &str) -> Option<FieldRef<'_>>;

    /// Metadata `key`, when it is set.
    fn metadata(&self, key: &str) -> Option<&str>;

    /// The child elements' names, in the order they were added.
    fn children(&self) -> Vec<String>;

    /// Child `name`'s reader, when there is one. A child written with
    /// [`ElementWriter::share`] is read with
    /// [`read_shared`](trait.ElementReader.html#method.read_shared)
    /// instead.
    fn child(&self, name: &str) -> Option<Box<dyn ElementReader + '_>>;

    /// The value shared as child `name`: `read` builds it from the child
    /// it resolves to, once per load and element; every later call for the
    /// same element returns the same `Arc`. Codecs call the typed
    /// [`read_shared`](trait.ElementReader.html#method.read_shared).
    fn shared_any(&self, name: &str, read: &mut ReadShared<'_>) -> Result<SharedValue, Error>;

    /// Where the element is, for messages (`/objects/box/result`).
    fn location(&self) -> String;
}

impl<'r> dyn ElementReader + 'r {
    /// The `T` shared as child `name` (see [`ElementWriter::share`]), read
    /// by `read` the first time this load meets its element.
    pub fn read_shared<T: Any + Send + Sync>(
        &self,
        name: &str,
        mut read: impl FnMut(&dyn ElementReader) -> Result<T, Error>,
    ) -> Result<Arc<T>, Error> {
        let any = self.shared_any(name, &mut |r| Ok(Arc::new(read(r)?) as SharedValue))?;
        any.downcast().map_err(|_| {
            Error::invalid(format_args!(
                "{}: shared {name} was read as another type",
                self.location()
            ))
        })
    }

    /// Child `name`, or an error naming it.
    pub fn required_child(&self, name: &str) -> Result<Box<dyn ElementReader + '_>, Error> {
        self.child(name)
            .ok_or_else(|| Error::invalid(format_args!("{}: no {name}", self.location())))
    }

    /// Field `name`, or an error naming it.
    pub fn required(&self, name: &str) -> Result<FieldRef<'_>, Error> {
        self.field(name)
            .ok_or_else(|| Error::invalid(format_args!("{}: no field {name}", self.location())))
    }

    fn wrong(&self, name: &str, what: &str) -> Error {
        Error::invalid(format_args!("{}: {name} is not {what}", self.location()))
    }

    /// Boolean field `name`.
    pub fn bool(&self, name: &str) -> Result<bool, Error> {
        match self.required(name)? {
            FieldRef::Bool(v) => Ok(v),
            _ => Err(self.wrong(name, "a boolean")),
        }
    }

    /// Whole-number field `name`.
    pub fn int(&self, name: &str) -> Result<i64, Error> {
        match self.required(name)? {
            FieldRef::Int(v) => Ok(v),
            _ => Err(self.wrong(name, "a whole number")),
        }
    }

    /// Real-number field `name`.
    pub fn float(&self, name: &str) -> Result<f64, Error> {
        match self.required(name)? {
            FieldRef::Float(v) => Ok(v),
            _ => Err(self.wrong(name, "a number")),
        }
    }

    /// Text field `name`.
    pub fn text(&self, name: &str) -> Result<&str, Error> {
        match self.required(name)? {
            FieldRef::Text(v) => Ok(v),
            _ => Err(self.wrong(name, "text")),
        }
    }

    /// `u32` array field `name`.
    pub fn u32s(&self, name: &str) -> Result<&[u32], Error> {
        match self.required(name)? {
            FieldRef::U32s(v) => Ok(v),
            _ => Err(self.wrong(name, "an array of u32")),
        }
    }

    /// `f32` array field `name`.
    pub fn f32s(&self, name: &str) -> Result<&[f32], Error> {
        match self.required(name)? {
            FieldRef::F32s(v) => Ok(v),
            _ => Err(self.wrong(name, "an array of f32")),
        }
    }

    /// `f64` array field `name`.
    pub fn f64s(&self, name: &str) -> Result<&[f64], Error> {
        match self.required(name)? {
            FieldRef::F64s(v) => Ok(v),
            _ => Err(self.wrong(name, "an array of f64")),
        }
    }

    /// Text array field `name`.
    pub fn texts(&self, name: &str) -> Result<&[String], Error> {
        match self.required(name)? {
            FieldRef::Texts(v) => Ok(v),
            _ => Err(self.wrong(name, "an array of text")),
        }
    }
}
