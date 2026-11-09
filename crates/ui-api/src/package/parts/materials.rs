//! What a Material dropped on a context's objects does ([`Materials`]).
use super::part;
use crate::host::{Cx, ObjectRef};
use kinogaki_studio_api::Error;

part! {
    /// What a Material dragged from the Materials pane does when it is let
    /// go over one of the context's objects: core finds the object (in a
    /// view, the open object's surface or an object at an object level; an
    /// Outliner row), the context makes the edit. A context with this part
    /// takes drops on its objects; without it, its objects refuse them.
    pub trait Materials<S> {
        /// Assigns the Material at `path` (absolute) to `object`, as Houdini's
        /// Material node does: a new node of the context's after the
        /// displayed one, on the object's selection while it is open, else on
        /// all of it. Core holds the call in one undo step ("Assign
        /// Material"); the selection stays as it is.
        ///
        /// # Errors
        /// What the context refuses (a selection that names nothing the
        /// Material can go on), in words for the status line.
        fn assign(&self, s: &mut S, cx: &mut dyn Cx, object: ObjectRef, path: &str)
        -> Result<(), Error>;
    }
}
