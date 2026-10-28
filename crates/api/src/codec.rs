//! How data and object bodies save into a project.
//!
//! Each data type and each object kind owns its part of the project file:
//! the environment creates an element for a value or a body (its kind and
//! place are the environment's) and hands the codec an
//! [`ElementWriter`] to fill it, or an [`ElementReader`] to read it back.
//! The environment stores nothing of its own inside that element, so a
//! type that is not installed leaves its elements untouched: the file
//! still opens and saves them back as they were.
//!
//! Codecs follow the project's rules: strict reading (no defaults for
//! missing values, no fallbacks) and no version fields.
use crate::data::Data;
use crate::element::{ElementReader, ElementWriter};
use crate::error::Error;
use crate::object::ObjectBody;
use std::sync::Arc;

/// Writes and reads values of one data type.
pub trait DataCodec: Send + Sync {
    /// Writes `data` into the element `out` fills (its fields and
    /// children).
    ///
    /// `base`, when given, is a value of the same type already written
    /// elsewhere in the project that `data` was derived from (a held base
    /// of an edit): a codec may store only what differs, as a mesh that
    /// only moved points stores no topology.
    fn write(
        &self,
        data: &Data,
        base: Option<&Data>,
        out: &mut dyn ElementWriter,
    ) -> Result<(), Error>;

    /// Reads the value [`write`](Self::write) wrote into `element`; `base`
    /// is the value read for the base it was written against.
    fn read(&self, element: &dyn ElementReader, base: Option<&Data>) -> Result<Data, Error>;
}

/// Writes and reads the bodies of one object kind.
pub trait ObjectCodec: Send + Sync {
    /// Writes `body` into the element `out` fills (a Mesh object's
    /// network as children).
    fn write(&self, body: &dyn ObjectBody, out: &mut dyn ElementWriter) -> Result<(), Error>;

    /// Reads the body [`write`](Self::write) wrote into `element`.
    fn read(&self, element: &dyn ElementReader) -> Result<Arc<dyn ObjectBody>, Error>;
}
