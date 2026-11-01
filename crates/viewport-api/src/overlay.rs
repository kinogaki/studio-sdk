//! Retained, depth-tested overlay layers drawn over a surface: diagnostic
//! findings that name a value's points and polygons (bound to its topology,
//! never to a packet), and tool guides with their own world-space
//! positions.
use crate::SurfaceSource;
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
/// One colored layer over a value's surface: filled polygons and lines.
/// Polygons are filled with the packet's own triangles, so a layer stays
/// right on every packet of its topology (a drag clips concave polygons
/// again); the ids shared, a renderer keeps their upload while the
/// allocations stay.
#[derive(Clone)]
pub struct OverlayLayer {
    /// sRGB.
    pub color: [u8; 3],
    /// Filled polygons, by primitive id.
    pub polygons: Arc<[u32]>,
    /// Line segments between point ids.
    pub lines: Arc<[OverlayLine]>,
}
impl OverlayLayer {
    /// Whether `other` holds the same color and the same allocations.
    pub fn same(&self, other: &Self) -> bool {
        self.color == other.color
            && Arc::ptr_eq(&self.polygons, &other.polygons)
            && Arc::ptr_eq(&self.lines, &other.lines)
    }
}
/// One colored layer of lines alone, between ids of whatever positions it
/// is drawn with (a guide's, a scene glyph's).
#[derive(Clone)]
pub struct LineLayer {
    /// sRGB.
    pub color: [u8; 3],
    /// Line segments.
    pub lines: Arc<[OverlayLine]>,
}
impl LineLayer {
    /// Whether `other` holds the same color and the same allocation.
    pub fn same(&self, other: &Self) -> bool {
        self.color == other.color && Arc::ptr_eq(&self.lines, &other.lines)
    }
}
/// Layers over a value's points and polygons, made from the value alone
/// (packet point i is value point i). Layer order is significant:
/// overlapping layers blend in order.
pub struct OverlayDraw {
    /// The topology the layers name ([`SurfaceSource::topology_key`](crate::SurfaceSource::topology_key)).
    pub on: [u8; 32],
    /// The layers, drawn in order.
    pub layers: Vec<OverlayLayer>,
}
impl OverlayDraw {
    /// Whether the layers belong to `surface`'s topology.
    pub fn matches(&self, surface: &dyn SurfaceSource) -> bool {
        surface.topology_key() == self.on
    }
    /// Logical upload size.
    pub fn upload_bytes(&self) -> usize {
        self.layers
            .iter()
            .map(|l| l.polygons.len() * 4 + l.lines.len() * std::mem::size_of::<OverlayLine>())
            .sum()
    }
    /// How many segments the layers hold.
    pub fn segment_count(&self) -> usize {
        self.layers.iter().map(|l| l.lines.len()).sum()
    }
}
/// Face normals shown with a value's surface: each face's center and unit
/// normal in the value's own space, drawn as lines the display's normal
/// length.
pub struct FaceNormals {
    /// The topology the faces are of ([`SurfaceSource::topology_key`](crate::SurfaceSource::topology_key)).
    pub on: [u8; 32],
    /// Each face's center (`[0..3]`) and unit normal (`[3..6]`).
    pub faces: Vec<[f32; 6]>,
}
impl FaceNormals {
    /// Whether the normals belong to `surface`'s topology.
    pub fn matches(&self, surface: &dyn SurfaceSource) -> bool {
        surface.topology_key() == self.on
    }
}
/// A tool's guide segments, shown with the edited surface in the value's
/// own space (line point ids index `positions`).
pub struct GuideDraw {
    /// The point `positions` are offsets from, in the value's space (as a
    /// packet's are from [`SurfaceSource::origin`], so guides far from the
    /// origin stay precise); typically the value's bounds center.
    pub origin: [f64; 3],
    /// The guides' own points, as f32 offsets from `origin`.
    pub positions: Vec<[f32; 3]>,
    /// The segments (ids index `positions`).
    pub layer: LineLayer,
}
impl GuideDraw {
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
    pub layers: Vec<LineLayer>,
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
