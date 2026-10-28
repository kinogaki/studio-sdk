//! Held edits: a node holding an authored delta over its input (Edit Mesh,
//! Edit CAD). Core owns the mechanism (capturing a delta from interactive
//! edits, re-applying it when the input recooks, undo); a package supplies
//! only how a delta applies to its data type.
//!
//! This is a sketch of the package side, enough to place the boundary: a
//! delta is itself [`Data`] of a type the package registers (so it saves,
//! stamps and counts memory through that type), and [`HeldEdit::apply`]
//! applies it. How a delta is authored (the interactive tools that record
//! moves, inserts and deletions) is the host UI interface's, still to be
//! designed (`studio-ui-api`).
use crate::cancel::CancelToken;
use crate::data::Data;
use crate::error::Error;
use crate::id::DataTypeId;

/// How authored deltas apply to values of one data type.
pub trait HeldEdit: Send + Sync {
    /// The data type the edits apply to (`kinogaki.mesh.geometry`).
    fn base(&self) -> DataTypeId;

    /// The data type of its deltas (`kinogaki.mesh.edits`), registered by
    /// the same package or an earlier one.
    fn delta(&self) -> DataTypeId;

    /// `base` with `delta` applied. A delta authored against an earlier
    /// version of the input applies as far as it still can (moves of
    /// points that still exist); what no longer applies is dropped and
    /// reported as [`Error::Invalid`] only when nothing applies.
    fn apply(&self, base: &Data, delta: &Data, cancel: &CancelToken) -> Result<Data, Error>;
}
