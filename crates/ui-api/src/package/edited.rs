//! What the open location's edited surface shows in one view: the value a
//! context's [`ViewportLayer::edited`](super::ViewportLayer::edited)
//! returns each frame, and how its templated pieces are marked.
//!
//! Everything in it is shared: core turns it into its renderer's scene by
//! borrowing, nothing is copied, and the renderer uploads only values
//! whose `Arc` changed since the last frame. A context keeps the same
//! `Arc`s while nothing changed, so a steady frame costs nothing.
use kinogaki_studio_api::viewport::{ElementKind, ElementSet, SurfaceSource};
use kinogaki_viewport_api::{
    FaceNormals, GuideDraw, LineHighlight, OverlayDraw, PointWeights, PreviewUpdate, SceneLines,
};
use std::fmt;
use std::sync::Arc;

/// What the open location's edited surface shows in one view this frame.
///
/// Every layer that names a base packet (`overlays`, `guides`,
/// `face_normals`) and `preview` must belong to `surface`; values made
/// for another packet (a cook on its way) are left out by the context.
#[derive(Clone, Default)]
pub struct Edited {
    /// The surface being edited.
    pub surface: Option<Arc<dyn SurfaceSource>>,
    /// What is selected on it: faces (or whole objects), lines and markers,
    /// each set in its kind's slot ([`Edited::select`]).
    pub selection: [Option<ElementSet>; 3],
    /// A live move over `surface`.
    pub preview: Option<Arc<PreviewUpdate>>,
    /// Diagnostic findings and edge marks over `surface`'s points.
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
    /// What is under the pointer.
    pub hover: Hover,
    /// Which of the surface's parts draw.
    pub look: SurfaceLook,
    /// This view's own world-space lines (CAD's combs, silhouettes).
    pub lines: Vec<Arc<SceneLines>>,
    /// The plane the grid lies in instead of the ground (a sketch's):
    /// origin, first axis, second axis, the axes unit length.
    pub grid_plane: Option<[[f64; 3]; 3]>,
    /// A section plane `[n.x, n.y, n.z, d]`, `n` unit length: what lies
    /// where `n · p + d > 0` is cut away (a sketch's Slice).
    pub section: Option<[f32; 4]>,
}

impl Edited {
    /// The selection slot sets of `kind` go in.
    pub fn slot(kind: ElementKind) -> usize {
        match kind {
            ElementKind::Face | ElementKind::Object => 0,
            ElementKind::Edge => 1,
            ElementKind::Point | ElementKind::Vertex => 2,
        }
    }

    /// `set` in its kind's selection slot (replacing what was there).
    pub fn select(&mut self, set: ElementSet) {
        let slot = Self::slot(set.kind);
        self.selection[slot] = Some(set);
    }
}

impl fmt::Debug for Edited {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Edited")
            .field("surface", &self.surface.is_some())
            .field("selection", &self.selection)
            .field("preview", &self.preview.is_some())
            .field("hover", &self.hover)
            .field("look", &self.look)
            .field("lines", &self.lines.len())
            .finish_non_exhaustive()
    }
}

/// What of the edited surface is under the pointer, by the ids its packet
/// gives.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Hover {
    /// A face, shown blended over the surface.
    pub face: Option<u32>,
    /// The hovered face's mirror under symmetry, tinted like it.
    pub mirror: Option<u32>,
    /// A face identifying the hovered object (a CAD body, a mesh piece).
    pub object: Option<u32>,
    /// A point marker, in its class's hover look.
    pub point: Option<u32>,
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

/// How the open object's templated piece is marked.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TemplateMark {
    /// A live tool chose it as an operand.
    pub operand: bool,
    /// It is the branch a live tool would take under the pointer.
    pub hovered: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sets_take_their_kinds_slots_and_the_look_starts_plain() {
        let mut edited = Edited::default();
        edited.select(ElementSet::new(ElementKind::Vertex, false, [4]));
        edited.select(ElementSet::new(ElementKind::Object, false, [1]));
        edited.select(ElementSet::new(ElementKind::Edge, false, [2]));
        edited.select(ElementSet::new(ElementKind::Point, false, [3]));
        let kinds = edited
            .selection
            .each_ref()
            .map(|s| s.as_ref().map(|s| s.kind));
        assert_eq!(
            kinds,
            [
                Some(ElementKind::Object),
                Some(ElementKind::Edge),
                Some(ElementKind::Point)
            ]
        );
        let look = SurfaceLook::default();
        assert!(look.faces && look.wires && !look.points && !look.normals);
        assert_eq!((look.hidden_lines, look.hidden_points), (0, 0));
    }
}
