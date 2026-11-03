//! What a host renders an object from: a shape and the primvars a render
//! reads from it ([`ShapeSource`]), whatever made it (a polygon value's
//! display triangulation, its curves, a CAD tessellation, a toolpath
//! preview).
//!
//! Hosts keep engine scenes in step with sources cheaply, so a drag sends
//! only the points that moved:
//!
//! - an unchanged source (the same [`identity`](ShapeSource::identity)) is
//!   skipped;
//! - a source of the same [`topology`](ShapeSource::topology) is a points
//!   update, never a rebuild, narrowed by
//!   [`moved_since`](ShapeSource::moved_since);
//! - a primvar or the subsets are sent again only when their columns aren't
//!   the same columns.
use crate::Ranges;
use crate::prim::Topology;
use kinogaki_geometry::{Attributes, Column};
use std::sync::Arc;

/// A shape and what a render reads from it: an immutable snapshot whose
/// columns are shared, never copied by the host.
///
/// **Invariants** (the conformance suite checks them):
/// - the same identity: everything is identical;
/// - the same topology: identical connectivity;
/// - an unchanged primvar or subset is the same column in the next snapshot.
///
/// Every method is called from any thread and must be quick except the
/// column getters, which may build their column on first use.
pub trait ShapeSource: Send + Sync {
    /// This snapshot's identity: a new snapshot gets a new one. A source
    /// whose identity is the last one sent is skipped.
    fn identity(&self) -> &Arc<()>;

    /// How its points connect: while it stands (by its
    /// [`key`](Topology::key)), an edit is a points update, not a rebuild.
    fn topology(&self) -> Topology;

    /// The point positions, in the value's own space.
    fn points(&self) -> Column<[f32; 3]>;

    /// The primvars named `wanted` it has, each in its render domain
    /// ([`Shape`](crate::Shape)'s table) in the storage and space its value
    /// holds it (a host converts colors, once, where it sends them). A name
    /// it lacks is left out. Shading normals are `N`, which a source builds
    /// only when asked: a host leaves it out during a drag.
    fn primvars(&self, wanted: &[Arc<str>]) -> Attributes;

    /// Each primitive's index into the distinct material paths
    /// ([`Subsets::DEFAULT`](crate::Subsets::DEFAULT): the engine's default),
    /// and those paths (a Material's node path, a USD material path, an OBJ
    /// material name).
    fn subsets(&self) -> (Column<u16>, Vec<Arc<str>>);

    /// The points moved since the source with identity `previous`, when
    /// this source knows (a drag's snapshots do); `None`: unknown, which
    /// the host treats as every point.
    fn moved_since(&self, previous: &Arc<()>) -> Option<Ranges>;
}

/// A [`ShapeSource`] of fixed points and triangles: flat-shaded, in the
/// engine's default material, with no primvars or drag history. It suits a
/// data type whose renders need nothing more (a primitive, a preview); a type
/// with materials, normals or interactive drags implements [`ShapeSource`]
/// itself.
///
/// ```
/// use kinogaki_render_api::{PlainTriangles, ShapeSource};
///
/// let points = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
/// let source = PlainTriangles::new(points.to_vec().into(), vec![[0, 1, 2]].into());
/// assert_eq!(source.topology().primitives(), 1);
/// assert!(source.subsets().0.is_empty());
/// ```
#[derive(Clone, Debug)]
pub struct PlainTriangles {
    identity: Arc<()>,
    points: Column<[f32; 3]>,
    triangles: Column<[u32; 3]>,
}

impl PlainTriangles {
    /// `triangles`, three indices into `points` each, as a new snapshot.
    /// Sources over the same `triangles` column have the same topology, so
    /// a resized box keeps its column and a box turned into another shape
    /// gets a new one.
    pub fn new(points: Column<[f32; 3]>, triangles: Column<[u32; 3]>) -> Self {
        Self {
            identity: Arc::new(()),
            points,
            triangles,
        }
    }
}

impl ShapeSource for PlainTriangles {
    fn identity(&self) -> &Arc<()> {
        &self.identity
    }

    fn topology(&self) -> Topology {
        Topology::Triangles(self.triangles.clone())
    }

    fn points(&self) -> Column<[f32; 3]> {
        self.points.clone()
    }

    /// None: flat, zero texture coordinates.
    fn primvars(&self, _: &[Arc<str>]) -> Attributes {
        Attributes::new()
    }

    /// The engine's default material on every triangle.
    fn subsets(&self) -> (Column<u16>, Vec<Arc<str>>) {
        (Column::default(), Vec::new())
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
        let points = vec![[0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [1.0; 3]].into();
        let triangles: Column<[u32; 3]> = vec![[0, 1, 2], [1, 3, 2]].into();
        let source = PlainTriangles::new(points, triangles.clone());
        assert_eq!(source.topology().primitives(), 2);
        assert_eq!(source.points().len(), 4);
        assert!(source.primvars(&[Arc::from("Cd")]).is_empty());
        assert!(source.subsets().1.is_empty());
        // Each snapshot is new; a clone is the same snapshot.
        let next = PlainTriangles::new(source.points(), triangles);
        assert!(!Arc::ptr_eq(source.identity(), next.identity()));
        assert!(Arc::ptr_eq(source.identity(), source.clone().identity()));
        assert_eq!(source.topology().key(), next.topology().key());
        assert_eq!(next.moved_since(source.identity()), None);
    }
}
