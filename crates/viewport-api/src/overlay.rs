//! Retained, depth-tested overlay layers drawn over a surface packet:
//! diagnostic findings that reference the packet's points, and tool guides
//! with their own world-space positions.
use crate::{SurfaceSource, same_surface};
use std::sync::Arc;

/// A segment between two point ids. An ordinary edge has zero offset; a point
/// cross expands both ends by the offset.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "bytemuck", derive(bytemuck::Pod, bytemuck::Zeroable))]
pub struct OverlayLine {
    /// The two point ids.
    pub points: [u32; 2],
    /// How far a point cross reaches from its point; zero for an edge.
    pub offset: [f32; 3],
}
/// One colored layer: filled triangles (point ids) and lines. The ids
/// shared, layers move to a later packet of the same topology without a
/// copy, and a renderer keeps their upload while the allocations stay.
#[derive(Clone)]
pub struct OverlayLayer {
    /// sRGB.
    pub color: [u8; 3],
    /// Filled triangles, three point ids each.
    pub triangles: Arc<[u32]>,
    /// Line segments.
    pub lines: Arc<[OverlayLine]>,
}
impl OverlayLayer {
    /// Whether `other` holds the same color and the same allocations.
    pub fn same(&self, other: &Self) -> bool {
        self.color == other.color
            && Arc::ptr_eq(&self.triangles, &other.triangles)
            && Arc::ptr_eq(&self.lines, &other.lines)
    }
}
/// Layers over `base`'s points. Layer order is significant: overlapping
/// layers blend in order.
pub struct OverlayDraw {
    /// The packet whose points the layers index.
    pub base: Arc<dyn SurfaceSource>,
    /// The layers, drawn in order.
    pub layers: Vec<OverlayLayer>,
}
impl OverlayDraw {
    /// Whether the layers belong to `base`.
    pub fn matches(&self, base: &dyn SurfaceSource) -> bool {
        same_surface(&*self.base, base)
    }
    /// Logical upload size.
    pub fn upload_bytes(&self) -> usize {
        self.layers
            .iter()
            .map(|l| l.triangles.len() * 4 + l.lines.len() * std::mem::size_of::<OverlayLine>())
            .sum()
    }
    /// How many segments the layers hold.
    pub fn segment_count(&self) -> usize {
        self.layers.iter().map(|l| l.lines.len()).sum()
    }
}
/// Face normals shown with `base`: each face's center and unit normal in
/// the packet's object space, drawn as lines the display's normal length.
pub struct FaceNormals {
    /// The packet the normals belong to.
    pub base: Arc<dyn SurfaceSource>,
    /// Each face's center (`[0..3]`) and unit normal (`[3..6]`).
    pub faces: Vec<[f32; 6]>,
}
impl FaceNormals {
    /// Whether the normals belong to `base`.
    pub fn matches(&self, base: &dyn SurfaceSource) -> bool {
        same_surface(&*self.base, base)
    }
}
/// World-space guide segments shown with `base` (line point ids index
/// `positions`, not the surface's points).
pub struct GuideDraw {
    /// The packet the guides show with.
    pub base: Arc<dyn SurfaceSource>,
    /// The guides' own points, world space.
    pub positions: Vec<[f32; 3]>,
    /// The guide triangles and segments (ids index `positions`).
    pub layer: OverlayLayer,
}
impl GuideDraw {
    /// Whether the guides belong to `base`.
    pub fn matches(&self, base: &dyn SurfaceSource) -> bool {
        same_surface(&*self.base, base)
    }
    /// How many segments the guides hold.
    pub fn segment_count(&self) -> usize {
        self.layer.lines.len()
    }
    /// Logical upload size.
    pub fn upload_bytes(&self) -> usize {
        self.positions.len() * 12 + self.layer.lines.len() * std::mem::size_of::<OverlayLine>()
    }
}

/// World-space line layers drawn with the scene and tested against its
/// depth, independent of any surface: glyphs of objects that have no
/// surface (cameras, lights). Line point ids index `positions`.
pub struct SceneLines {
    /// The points, world space.
    pub positions: Vec<[f32; 3]>,
    /// The layers, drawn in order (ids index `positions`).
    pub layers: Vec<OverlayLayer>,
}
impl SceneLines {
    /// How many segments the layers hold.
    pub fn segment_count(&self) -> usize {
        self.layers.iter().map(|l| l.lines.len()).sum()
    }
    /// Logical upload size.
    pub fn upload_bytes(&self) -> usize {
        self.positions.len() * 12 + self.segment_count() * std::mem::size_of::<OverlayLine>()
    }
}
