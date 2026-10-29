//! Held edits: a node holding an authored delta over its input (Edit Mesh,
//! Edit CAD). Core owns the mechanism: it captures the node's input once
//! as the delta's base (later cooks never read upstream again until the
//! node is reloaded), keeps one result per held state that every copy of
//! the state shares (undo snapshots show the same value again), applies
//! the delta anew when it changes, and undoes. A package supplies only how
//! a delta applies to its data type.
//!
//! A delta is itself [`Data`] of a type the package registers, so it
//! saves, stamps and counts memory through that type. How a delta is
//! authored (the interactive tools that record moves, inserts and
//! deletions) is the package's UI, through the host UI interface.
use crate::cook::{CookCx, CookError, Outputs};
use crate::data::Data;
use crate::id::DataTypeId;

/// How authored deltas apply to values of one data type.
pub trait HeldEdit: Send + Sync {
    /// The data type the edits apply to (`kinogaki.mesh.geometry`).
    fn base(&self) -> DataTypeId;

    /// The data type of its deltas (`kinogaki.mesh.action`), registered by
    /// the same package or an earlier one.
    fn delta(&self) -> DataTypeId;

    /// Whether applying `delta` reads the node's input `port` (1 and up)
    /// besides the captured base: what it projects onto, say. Port 0 is
    /// the base, which core captures once and never reads again. None by
    /// default.
    fn reads_input(&self, delta: &Data, port: usize) -> bool {
        let _ = (delta, port);
        false
    }

    /// `base` with `delta` applied: the held node's outputs. `inputs[i]`
    /// is the value wired into port `i + 1` when
    /// [`reads_input`](Self::reads_input) asked for it, else `None`. A
    /// delta authored against an earlier version of the input applies as
    /// far as it still can (moves of points that still exist); it fails
    /// only when nothing applies. The scratch slot of `cx` keeps what
    /// speeds the next application (adjusting the latest edit), as for any
    /// cook.
    fn apply(
        &self,
        cx: &mut CookCx<'_>,
        base: &Data,
        delta: &Data,
        inputs: &[Option<Data>],
    ) -> Result<Outputs, CookError>;
}
