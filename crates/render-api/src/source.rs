//! What a host renders an object from: triangles and the columns a render
//! reads from them ([`TriangleSource`]), whatever made them (a polygon
//! mesh's display triangulation, a CAD tessellation, a toolpath preview).
//!
//! Hosts keep engine scenes in step with sources cheaply, so a drag sends
//! only the points that moved:
//!
//! - an unchanged source (the same [`identity`](TriangleSource::identity))
//!   is skipped;
//! - a source with the same [`topology`](TriangleSource::topology) is a
//!   points update, never a rebuild, narrowed by
//!   [`moved_since`](TriangleSource::moved_since);
//! - looks, texture coordinates and attributes are resent only when their
//!   [`columns`](TriangleSource::columns) or
//!   [`attribute_identity`](TriangleSource::attribute_identity) change.
use crate::{MeshAttribute, Ranges};
use std::sync::Arc;

/// What a triangle renders as.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Look {
    /// The material's path (a Material's node path, a USD material path,
    /// an OBJ material name); also the name Cryptomatte gives the
    /// material.
    pub material: Arc<str>,
}

impl Look {
    /// The name Cryptomatte gives the material.
    pub fn path(&self) -> Arc<str> {
        self.material.clone()
    }
}

/// Triangles and the columns a render reads from them: an immutable
/// snapshot whose arrays are shared `Arc`s, never copied by the host.
///
/// **Invariants** (the conformance suite checks them):
/// - the same identity: everything is identical;
/// - the same topology: identical [`triangles`](Self::triangles);
/// - the same topology and the same [`columns`](Self::columns): identical
///   looks and texture coordinates.
///
/// Every method is called from any thread and must be quick except the
/// array getters, which may build their array on first use.
pub trait TriangleSource: Send + Sync {
    /// This snapshot's identity: a new snapshot gets a new one. A source
    /// whose identity is the last one sent is skipped.
    fn identity(&self) -> &Arc<()>;

    /// Equal while the triangles stand: an edit with the same topology is
    /// a points update, not a rebuild.
    fn topology(&self) -> u64;

    /// How many triangles; 0: nothing renders.
    fn triangle_count(&self) -> usize;

    /// Three point indices a triangle.
    fn triangles(&self) -> Arc<[[u32; 3]]>;

    /// The point positions, in the value's own space.
    fn points(&self) -> Arc<[[f32; 3]]>;

    /// Shading normals, three a triangle; `None`: flat. The host skips
    /// them during a drag and asks once it ends, so a source builds them
    /// lazily.
    fn corner_normals(&self) -> Option<Arc<[[f32; 3]]>>;

    /// Texture coordinates, three a triangle; `None`: zero.
    fn corner_uvs(&self) -> Option<Arc<[[f32; 2]]>>;

    /// Each triangle's index into the distinct looks (`u16::MAX`: the
    /// engine's default material), and those looks.
    fn looks(&self) -> (Arc<[u16]>, Vec<Look>);

    /// Identities of the columns [`looks`](Self::looks) and
    /// [`corner_uvs`](Self::corner_uvs) read: with equal topology, equal
    /// columns mean unchanged looks and coordinates.
    fn columns(&self) -> Vec<u64>;

    /// The attributes named `names` (a name the source lacks is left out).
    /// Color attributes go through `to_render`, which converts the scene's
    /// working space to the render's.
    fn attributes(
        &self,
        names: &[Arc<str>],
        to_render: &dyn Fn([f32; 3]) -> [f32; 3],
    ) -> Vec<MeshAttribute>;

    /// Identities of the values `names` bind to: equal identities mean the
    /// same values, which are not resent.
    fn attribute_identity(&self, names: &[Arc<str>]) -> Vec<u64>;

    /// Changes whenever any column changes (looks, coordinates, any
    /// attribute).
    fn revision(&self) -> u64;

    /// The points moved since the source with identity `previous`, when
    /// this source knows (a drag's snapshots do); `None`: unknown, which
    /// the host treats as every point.
    fn moved_since(&self, previous: &Arc<()>) -> Option<Ranges>;
}
