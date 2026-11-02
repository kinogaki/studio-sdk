//! What a context adds over the open object in one view: the value its
//! [`ViewportLayer::edited`](super::ViewportLayer::edited) returns each
//! frame (overlays, guides and style; core draws the object's value
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
use kinogaki_viewport_api::{
    FaceNormals, GuideDraw, LineHighlight, OverlayDraw, PointWeights, SceneLines,
};
use std::fmt;
use std::ops::Range;
use std::sync::Arc;

/// What a context adds over the open object in one view this frame.
///
/// Core draws the open object's shown value itself, through its type's
/// `Displayable`, and its drag previews; it draws this over whichever
/// packet it prepared. Layers bound to a topology (`overlays`,
/// `face_normals`) show only with a packet of that topology, so ones made
/// for another (a cook on its way) never draw on the wrong one.
#[derive(Clone, Default)]
pub struct Edited {
    /// Diagnostic findings and edge marks over `value`'s points and
    /// polygons.
    pub overlays: Option<Arc<OverlayDraw>>,
    /// A tool's guides; CAD's tessellation.
    pub guides: Option<Arc<GuideDraw>>,
    /// Face normals, drawn when the view shows them.
    pub face_normals: Option<Arc<FaceNormals>>,
    /// The soft-selection tint.
    pub weights: Option<Arc<PointWeights>>,
    /// The hovered line and the lines a second click would add.
    pub highlight: Option<Arc<LineHighlight>>,
    /// Linear RGB9E5 words by face or by face corner (0: the clay).
    pub colors: Option<Arc<[u32]>>,
    /// A value per point of the open object's drawn polygons, in their
    /// point order, for the Scalar shading (CAD's deviation): the points of
    /// [`HostDoc::shown_polygons`](crate::host::HostDoc::shown_polygons)
    /// (a B-rep's joined mesh, whose `face` names each polygon's face), on
    /// which the drawn surface's points are numbered. Values of another
    /// count than the drawn surface's points (a cook on its way) don't draw.
    pub scalars: Option<Arc<[f32]>>,
    /// The values the Scalar shading spans in this view, low to high;
    /// `None`: the view's own range.
    pub scalar_range: Option<[f32; 2]>,
    /// Which of the surface's parts draw (the context's own display state:
    /// points at a points level, hidden line classes). The selection and
    /// the hover are core's: it draws them from the context's
    /// `Levels::components` and the hit it handed `Levels::hovered`.
    pub look: SurfaceLook,
    /// This view's own world-space lines (CAD's combs, silhouettes).
    pub lines: Vec<Arc<SceneLines>>,
    /// The plane the grid lies in instead of the ground (a sketch's):
    /// origin, first axis, second axis, the axes unit length.
    pub grid_plane: Option<[[f64; 3]; 3]>,
    /// A section plane `[n.x, n.y, n.z, d]`, `n` unit length: what lies
    /// where `n · p + d > 0` is cut away (a sketch's Slice).
    pub section: Option<[f32; 4]>,
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
            .field("look", &self.look)
            .field("lines", &self.lines.len())
            .field("pieces", &self.pieces)
            .field("moving", &self.moving.as_ref().map(|m| m.points.len()))
            .field("ghosts", &self.ghosts)
            .finish_non_exhaustive()
    }
}

/// Which of the edited surface's parts draw in a view.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SurfaceLook {
    /// Its shaded faces (a subdivision cage beside its smooth surface
    /// draws none).
    pub faces: bool,
    /// Its ordinary edges.
    pub wires: bool,
    /// Every point marker, at any level.
    pub points: bool,
    /// Each corner's shading normal.
    pub normals: bool,
    /// Line classes not drawn (bit `LineClassId.0` set: hulls, isoparms).
    pub hidden_lines: u32,
    /// Point classes not drawn (bit `PointClassId.0` set).
    pub hidden_points: u32,
}

impl Default for SurfaceLook {
    /// Faces and edges, no points or normals, every class.
    fn default() -> Self {
        Self {
            faces: true,
            wires: true,
            points: false,
            normals: false,
            hidden_lines: 0,
            hidden_points: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_look_and_a_ghost_start_plain() {
        let look = SurfaceLook::default();
        assert!(look.faces && look.wires && !look.points && !look.normals);
        assert_eq!((look.hidden_lines, look.hidden_points), (0, 0));
        assert_eq!(
            GhostTone::default(),
            GhostTone::Neutral,
            "a ghost starts plain"
        );
    }
}
