//! Vertex shading a renderer derives from point positions.
//!
//! A packet whose points all move differently (a whole-mesh deformer drag)
//! would otherwise rebuild and upload every vertex record each step. With
//! a [`ShadingPlan`] the packet uploads positions alone and the renderer
//! recomputes each face's normal and each vertex's shaded normal on the
//! GPU. The face normal is the normalized sum of `(p[i] - p[0]) × (p[i+1] -
//! p[0])` over its vertices. The shaded normal follows the plan's
//! [`Smoothing`]; sums weight each face's normal by the face's angle at the
//! point and fall back to the vertex's own face normal. Curvature is not
//! derived: it keeps the packet's values.
use std::sync::Arc;

/// Connectivity and smoothing rule the derivation reads. Its parts are
/// shared between plans: a renderer uploads each part once and keeps it
/// while the next plan names the same allocation.
#[derive(Clone, Debug)]
pub struct ShadingPlan {
    /// Every face's vertices.
    pub faces: Arc<FaceVertices>,
    /// How shaded normals are made.
    pub smoothing: Smoothing,
}

/// The vertices of every face, built once per topology.
#[derive(Debug, Default)]
pub struct FaceVertices {
    /// Face `f`'s vertices are `face_offsets[f]..face_offsets[f + 1]`.
    pub face_offsets: Vec<u32>,
    /// The point of every vertex.
    pub vertex_points: Vec<u32>,
    /// The face of every vertex.
    pub vertex_faces: Vec<u32>,
}

/// How a vertex's shaded normal is made.
#[derive(Clone, Debug)]
pub enum Smoothing {
    /// The faces around the point whose normal is within 60° of the
    /// vertex's own face (`dot >= 0.5`), summed in `point_vertices` order.
    Angle(Arc<PointFans>),
    /// The faces reached from the vertex's own face across the links of
    /// its fan (see [`PointFans`]).
    Edges(Arc<PointFans>),
    /// Authored normals, normalized; a zero value takes the face normal.
    Authored(Arc<AuthoredNormals>),
}

/// The vertices around each point: those at point `p` are
/// `point_vertices[point_offsets[p]..point_offsets[p + 1]]`.
///
/// For [`Smoothing::Edges`] each entry also carries, above [`PointFans::ID`],
/// whether it starts a fan ([`PointFans::FIRST`]) and how its face joins the
/// next entry of the same fan, the last joining the first
/// ([`PointFans::LINK_SHIFT`]: a [`Link`]). A fan is the faces at the point
/// in order across the point's edges; an open fan's last entry is
/// [`Link::Apart`].
#[derive(Debug, Default)]
pub struct PointFans {
    /// Point `p`'s entries start at `point_offsets[p]`; one more than points.
    pub point_offsets: Vec<u32>,
    /// The entries: vertex ids, with fan bits for [`Smoothing::Edges`].
    pub point_vertices: Vec<u32>,
}
impl PointFans {
    /// The vertex id bits of an entry.
    pub const ID: u32 = (1 << 29) - 1;
    /// The entry starts a fan.
    pub const FIRST: u32 = 1 << 29;
    /// The entry's [`Link`] to the next entry of its fan.
    pub const LINK_SHIFT: u32 = 30;
    /// An entry of an edge fan (`vertex` at most [`Self::ID`]).
    pub fn entry(vertex: u32, first: bool, link: Link) -> u32 {
        debug_assert!(vertex <= Self::ID);
        vertex | if first { Self::FIRST } else { 0 } | (link as u32) << Self::LINK_SHIFT
    }
}

/// How two neighboring faces of a fan share their edge's shading.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Link {
    /// A hard edge, a border or an edge of more than two faces.
    Apart = 0,
    /// A soft edge: joined whatever the angle.
    Smooth = 1,
    /// An unmarked edge: joined when the two faces are within 60°.
    Angle = 2,
}

/// Normals authored on one class of elements, normalized (zero stays zero).
#[derive(Debug, Default)]
pub struct AuthoredNormals {
    /// Which elements the normals are on.
    pub class: NormalClass,
    /// One normal an element of `class`.
    pub values: Vec<[f32; 3]>,
}

/// Which element's authored normal a vertex reads: its own, its point's or
/// its face's.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u32)]
pub enum NormalClass {
    #[default]
    /// One normal a polygon vertex.
    Vertex = 0,
    /// One normal a point.
    Point = 1,
    /// One normal a face.
    Face = 2,
}

impl ShadingPlan {
    /// How many faces.
    pub fn face_count(&self) -> usize {
        self.faces.face_offsets.len().saturating_sub(1)
    }
    /// How many polygon vertices.
    pub fn vertex_count(&self) -> usize {
        self.faces.vertex_points.len()
    }
    /// Logical upload size.
    pub fn upload_bytes(&self) -> usize {
        let faces = &self.faces;
        4 * (faces.face_offsets.len() + faces.vertex_points.len() + faces.vertex_faces.len())
            + match &self.smoothing {
                Smoothing::Angle(fans) | Smoothing::Edges(fans) => {
                    4 * (fans.point_offsets.len() + fans.point_vertices.len())
                }
                Smoothing::Authored(normals) => 12 * normals.values.len(),
            }
    }
}
