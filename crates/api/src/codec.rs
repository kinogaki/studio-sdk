//! How data and object bodies save into a project.
//!
//! Each data type and each object kind owns its part of the project file:
//! the environment creates an element for a value or a body (its kind and
//! place are the environment's) and hands the codec an
//! [`ElementWriter`] to fill it, or an [`ElementReader`] to read it back.
//! The environment stores nothing of its own inside that element. A
//! project holding values of a type no installed package registers still
//! opens: those values, and what holds them, are skipped, the Console
//! says what was skipped and why, and saving writes only what loaded.
//!
//! Codecs follow the project's rules: strict reading (no defaults for
//! missing values, no fallbacks) and no version fields. A body that needs
//! something no installed package provides (a light of a type no engine
//! declares) reads as [`Error::NotInstalled`]: the host skips that object
//! and says why, and the rest of the project opens.
use crate::components::Components;
use crate::data::Data;
use crate::element::{ElementReader, ElementWriter};
use crate::error::Error;
use crate::object::ObjectBody;
use kinogaki_render_api::Engines;
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

    /// Writes `components` (a selection of `data`'s components, in this
    /// type's representation) into `out`: what a kept result carries
    /// downstream beside its value. `Ok(false)` by default: the type
    /// doesn't save selections, and a kept result loads without one.
    fn write_components(
        &self,
        components: &Components,
        data: &Data,
        out: &mut dyn ElementWriter,
    ) -> Result<bool, Error> {
        let _ = (components, data, out);
        Ok(false)
    }

    /// Reads the components [`write_components`](Self::write_components)
    /// wrote into `element`, of `data`.
    fn read_components(
        &self,
        element: &dyn ElementReader,
        data: &Data,
    ) -> Result<Components, Error> {
        let _ = (element, data);
        Err(Error::invalid("this data type saves no selections"))
    }
}

/// Writes and reads the bodies of one object kind.
pub trait ObjectCodec: Send + Sync {
    /// Writes `body` into the element `out` fills (a Mesh object's
    /// network as children).
    fn write(&self, body: &dyn ObjectBody, out: &mut dyn ElementWriter) -> Result<(), Error>;

    /// Reads the body [`write`](Self::write) wrote into `element`, with
    /// `cx` saying what is installed: [`Error::NotInstalled`] when the body
    /// needs something that isn't.
    fn read(
        &self,
        element: &dyn ElementReader,
        cx: &ReadCx<'_>,
    ) -> Result<Arc<dyn ObjectBody>, Error>;
}

/// What is installed, as an object's codec may ask while it reads a body:
/// a read-only view of the host's packages.
#[derive(Clone, Copy, Debug)]
pub struct ReadCx<'a> {
    engines: &'a Engines,
}

impl<'a> ReadCx<'a> {
    /// What a host with render engines `engines` has installed.
    pub fn new(engines: &'a Engines) -> Self {
        Self { engines }
    }

    /// The installed render engines' descriptions (their declared prim
    /// types, settings and vocabularies); none installed: empty.
    pub fn engines(&self) -> &'a Engines {
        self.engines
    }
}
