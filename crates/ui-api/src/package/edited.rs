//! What a context adds over the open object in one view: the value its
//! [`ViewportLayer::edited`](super::ViewportLayer::edited) returns each
//! frame (overlays and guides; core draws the object's value
//! itself, and marks the templated pieces a live tool picks from), and
//! the points a drag is moving ([`Moving`]), which core previews over
//! that value.
//!
//! Everything in it is shared: core turns it into its renderer's scene by
//! borrowing, nothing is copied, and the renderer uploads only values
//! whose `Arc` changed since the last frame. A context keeps the same
//! `Arc`s while nothing changed, so a steady frame costs nothing.
use kinogaki_studio_api::geometry::WeightedMove;
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
    /// Diagnostic findings and edge marks over `value`'s points and
    /// polygons.
    pub overlays: Option<Arc<OverlayDraw>>,
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
    /// polygons until the edit it commits is shown; `None`: no drag.
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

/// A drag of some of the open polygons' points, which core previews over
/// the shown value: no cook, no cached value, and only the moved points'
/// polygons sent to the GPU each step.
///
/// The context resolves `points` (and `weights`) once, when the drag
/// begins, and hands the same `Arc`s every frame until it ends: core
/// gathers what the preview needs once per `points` allocation, so a
/// new allocation starts a new preview. `motion` is the current step,
/// built with the function the context's commit cooks with, so the
/// preview places each point where the edit will
/// ([`WeightedMove::at`]). Once the drag ends the context returns no
/// `Moving`; core keeps the last step shown while the edit it committed
/// cooks, so the committed value replaces it.
#[derive(Clone, Debug)]
pub struct Moving {
    /// The moved points, by index into the shown value's points, sorted
    /// and without repeats.
    pub points: Arc<[u32]>,
    /// Each moved point's weight (0 fixed, 1 moving fully), in `points`'
    /// order: a soft move. `None`: every point moves fully.
    pub weights: Option<Arc<[f64]>>,
    /// The step, with its mirror for a symmetric move.
    pub motion: WeightedMove,
}

impl fmt::Debug for Edited {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Edited")
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
