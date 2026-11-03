//! What a context adds over the open object in one view: the value its
//! [`Display::edited`](super::Display::edited) returns each
//! frame (overlays and guides; core draws the object's value
//! itself, and marks the templated pieces a live tool picks from), and
//! the points a drag is moving ([`Moving`]), which core previews over
//! that value.
//!
//! Everything in it is shared: core turns it into its renderer's scene by
//! borrowing, nothing is copied, and the renderer uploads only values
//! whose `Arc` changed since the last frame. A context keeps the same
//! `Arc`s while nothing changed, so a steady frame costs nothing.
use kinogaki_studio_api::edit::preview::Moving;
use kinogaki_studio_api::{NodeRef, ObjectRef};
use kinogaki_viewport_api::{GuideDraw, OverlayDraw};
use std::fmt;
use std::ops::Range;
use std::sync::Arc;

/// What a context adds over the open object in one view this frame.
///
/// Core draws the open object's shown value itself, through its type's
/// `Displayable`, and its drag previews; it draws this over whichever
/// packet it prepared. Overlays are bound to a topology and show only
/// with a packet of that topology, so ones made for another (a cook on its
/// way) never draw on the wrong one.
#[derive(Clone, Default)]
pub struct Edited {
    /// Layers of diagnostic findings and edge marks over the shown
    /// value's points and polygons, drawn in order; each is bound to the
    /// topology it was made for ([`OverlayDraw::on`]) and shows only over
    /// a packet of that topology, so a layer still made for the last cook
    /// leaves the others showing.
    pub overlays: Vec<Arc<OverlayDraw>>,
    /// A tool's guides; CAD's tessellation.
    pub guides: Option<Arc<GuideDraw>>,
    /// Every point marker of the surface shows, at any level (a context's
    /// own display state: its points level).
    pub points: bool,
    /// Which of the open object's own pieces draw in this view, by their
    /// index in its `Drawn` (a print's layers up to the Timeline's, or one
    /// alone); `None`: all of them. Choosing builds nothing: the pieces
    /// are the value's, prepared once.
    pub pieces: Option<Range<usize>>,
    /// The points a drag is moving, shown moved over the open object's
    /// polygons until the edit it commits is shown ([`Moving`]); `None`:
    /// no drag.
    pub moving: Option<Moving>,
    /// Nodes' cooked values core draws ghosted in this view, each placed
    /// by its object and in its tone (a tool's input beside what it makes,
    /// what a cut removes). Core cooks and prepares each as it does an
    /// object's shown value, kept while the value stays; one still cooking
    /// draws once its answer arrives.
    pub ghosts: Vec<Ghost>,
}

/// A node's cooked value drawn ghosted ([`Edited::ghosts`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ghost {
    /// The object whose network holds the node, which places it.
    pub object: ObjectRef,
    /// The node.
    pub node: NodeRef,
    /// What it shows as.
    pub tone: GhostTone,
}

/// What a ghost shows as: the host tints it with its theme's color for
/// that role, never a color of the context's.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum GhostTone {
    /// A value beside the edit (a tool's input): the plain ghost.
    #[default]
    Neutral,
    /// What the edit adds (a join's tool bodies): the theme's accent.
    Add,
    /// What the edit takes away (a cut's removed volume): the theme's
    /// error color.
    Remove,
}

impl fmt::Debug for Edited {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Edited")
            .field("overlays", &self.overlays.len())
            .field("points", &self.points)
            .field("pieces", &self.pieces)
            .field("moving", &self.moving.as_ref().map(|m| m.points.len()))
            .field("ghosts", &self.ghosts)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_ghost_starts_plain() {
        assert_eq!(
            GhostTone::default(),
            GhostTone::Neutral,
            "a ghost starts plain"
        );
    }
}
