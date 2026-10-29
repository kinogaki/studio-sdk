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

/// A [`TriangleSource`] of fixed points and triangles: flat-shaded, in the
/// engine's default material, with no texture coordinates, attributes or
/// drag history. It suits a data type whose renders need nothing more (a
/// primitive, a preview); a type with materials, normals or interactive
/// drags implements [`TriangleSource`] itself.
///
/// ```
/// use kinogaki_render_api::{PlainTriangles, TriangleSource};
///
/// let points = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
/// let source = PlainTriangles::new(points, [[0, 1, 2]], 1);
/// assert_eq!(source.triangle_count(), 1);
/// assert_eq!(source.looks().0.len(), 1);
/// ```
#[derive(Clone, Debug)]
pub struct PlainTriangles {
    identity: Arc<()>,
    topology: u64,
    points: Arc<[[f32; 3]]>,
    triangles: Arc<[[u32; 3]]>,
}

impl PlainTriangles {
    /// `triangles`, three indices into `points` each, as a new snapshot.
    /// `topology` is the caller's key for the triangles: sources with
    /// equal keys must have identical triangles
    /// ([`TriangleSource::topology`]), so a resized box keeps its key and
    /// a box turned into another shape gets a new one.
    pub fn new(
        points: impl Into<Arc<[[f32; 3]]>>,
        triangles: impl Into<Arc<[[u32; 3]]>>,
        topology: u64,
    ) -> Self {
        Self {
            identity: Arc::new(()),
            topology,
            points: points.into(),
            triangles: triangles.into(),
        }
    }
}

impl TriangleSource for PlainTriangles {
    fn identity(&self) -> &Arc<()> {
        &self.identity
    }

    fn topology(&self) -> u64 {
        self.topology
    }

    fn triangle_count(&self) -> usize {
        self.triangles.len()
    }

    fn triangles(&self) -> Arc<[[u32; 3]]> {
        self.triangles.clone()
    }

    fn points(&self) -> Arc<[[f32; 3]]> {
        self.points.clone()
    }

    /// Flat.
    fn corner_normals(&self) -> Option<Arc<[[f32; 3]]>> {
        None
    }

    /// Zero.
    fn corner_uvs(&self) -> Option<Arc<[[f32; 2]]>> {
        None
    }

    /// The engine's default material on every triangle.
    fn looks(&self) -> (Arc<[u16]>, Vec<Look>) {
        (vec![u16::MAX; self.triangles.len()].into(), Vec::new())
    }

    /// None: looks and coordinates never change.
    fn columns(&self) -> Vec<u64> {
        Vec::new()
    }

    /// None: the source has no attributes.
    fn attributes(
        &self,
        _names: &[Arc<str>],
        _to_render: &dyn Fn([f32; 3]) -> [f32; 3],
    ) -> Vec<MeshAttribute> {
        Vec::new()
    }

    fn attribute_identity(&self, names: &[Arc<str>]) -> Vec<u64> {
        vec![0; names.len()]
    }

    fn revision(&self) -> u64 {
        0
    }

    /// Unknown: a new snapshot is every point moved.
    fn moved_since(&self, _previous: &Arc<()>) -> Option<Ranges> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_triangles_are_flat_and_default_material() {
        let points = [[0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [1.0; 3]];
        let source = PlainTriangles::new(points, [[0, 1, 2], [1, 3, 2]], 9);
        assert_eq!((source.triangle_count(), source.topology()), (2, 9));
        assert_eq!(source.points().len(), 4);
        assert!(source.corner_normals().is_none() && source.corner_uvs().is_none());
        let (looks, distinct) = source.looks();
        assert_eq!((&looks[..], distinct.len()), (&[u16::MAX; 2][..], 0));
        let names: [Arc<str>; 1] = [Arc::from("Cd")];
        assert!(source.attributes(&names, &|c| c).is_empty());
        assert_eq!(source.attribute_identity(&names), vec![0]);
        // Each snapshot is new; a clone is the same snapshot.
        let next = PlainTriangles::new(source.points(), source.triangles(), 9);
        assert!(!Arc::ptr_eq(source.identity(), next.identity()));
        assert!(Arc::ptr_eq(source.identity(), source.clone().identity()));
        assert_eq!(next.moved_since(source.identity()), None);
    }
}
