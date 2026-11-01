//! What a context adds over the open object in one view: the value its
//! [`ViewportLayer::edited`](super::ViewportLayer::edited) returns each
//! frame (overlays, guides and style; core draws the object's value
//! itself, drag previews included, and marks the templated pieces a live
//! tool picks from).
//!
//! Everything in it is shared: core turns it into its renderer's scene by
//! borrowing, nothing is copied, and the renderer uploads only values
//! whose `Arc` changed since the last frame. A context keeps the same
//! `Arc`s while nothing changed, so a steady frame costs nothing.
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
    /// A value per position for the Scalar shading (CAD's deviation).
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
}

impl fmt::Debug for Edited {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Edited")
            .field("look", &self.look)
            .field("lines", &self.lines.len())
            .field("pieces", &self.pieces)
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
    fn the_look_starts_plain() {
        let look = SurfaceLook::default();
        assert!(look.faces && look.wires && !look.points && !look.normals);
        assert_eq!((look.hidden_lines, look.hidden_points), (0, 0));
    }
}
