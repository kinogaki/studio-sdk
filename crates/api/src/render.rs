//! Neutral render items: how any data type reaches every render engine
//! ([`TriangleSource`], [`RenderItem`]).
//!
//! Engines never see kernel types: a CAD body renders as its
//! tessellation's triangles, a mesh as its display triangulation. The host
//! keeps engine scenes in step with sources cheaply, so a drag sends only
//! the points that moved:
//!
//! - an unchanged source (the same [`identity`](TriangleSource::identity))
//!   is skipped;
//! - a source with the same [`topology`](TriangleSource::topology) is a
//!   points update, never a rebuild, narrowed by
//!   [`moved_since`](TriangleSource::moved_since);
//! - looks, texture coordinates and attributes are resent only when their
//!   [`columns`](TriangleSource::columns) or
//!   [`attribute_identity`](TriangleSource::attribute_identity) change.
//!
//! The source contract here ([`TriangleSource`], [`Ranges`], [`Look`],
//! [`MeshAttribute`], [`AttributeDomain`]) is a placeholder until the
//! SDK's `render-api` crate lands: that crate will own it and this module
//! will re-export it from there. [`RenderItem`] stays in this crate.
use crate::math::Mat4;
use std::fmt;
use std::ops::Range;
use std::sync::Arc;

/// Point ranges that changed: sorted, disjoint spans, or every point.
/// Adding joins overlapping and touching spans; the host decides when many
/// spans are worth a whole upload.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Ranges {
    spans: Vec<Range<u32>>,
    whole: bool,
}

impl Ranges {
    /// Nothing changed.
    pub fn new() -> Ranges {
        Ranges::default()
    }

    /// Everything changed.
    pub fn whole() -> Ranges {
        Ranges {
            spans: Vec::new(),
            whole: true,
        }
    }

    /// Only `range` changed.
    pub fn of(range: Range<u32>) -> Ranges {
        let mut ranges = Ranges::new();
        ranges.add(range);
        ranges
    }

    /// Whether everything changed.
    pub fn is_whole(&self) -> bool {
        self.whole
    }

    /// Whether nothing changed.
    pub fn is_empty(&self) -> bool {
        !self.whole && self.spans.is_empty()
    }

    /// The spans, sorted and disjoint (empty when whole).
    pub fn spans(&self) -> &[Range<u32>] {
        &self.spans
    }

    /// Adds `range`, joining the spans it overlaps or touches.
    pub fn add(&mut self, mut range: Range<u32>) {
        if self.whole || range.start >= range.end {
            return;
        }
        let first = self.spans.partition_point(|r| r.end < range.start);
        let mut end = first;
        while end < self.spans.len() && self.spans[end].start <= range.end {
            range.start = range.start.min(self.spans[end].start);
            range.end = range.end.max(self.spans[end].end);
            end += 1;
        }
        self.spans.splice(first..end, [range]);
    }

    /// Adds every span of `other` (whole when either is).
    pub fn merge(&mut self, other: &Ranges) {
        if other.whole {
            *self = Ranges::whole();
        } else {
            for range in &other.spans {
                self.add(range.clone());
            }
        }
    }
}

/// What a triangle renders as.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Look {
    /// The material's path (a Material's node path, a USD material path,
    /// an OBJ material name); also the name Cryptomatte gives the
    /// material.
    pub material: Arc<str>,
}

/// Where a [`MeshAttribute`]'s values sit, and how a hit reads them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AttributeDomain {
    /// One value a point, interpolated across each triangle.
    Point,
    /// Three values a triangle (one a corner, in its corner order),
    /// interpolated across it.
    Corner,
    /// One value a triangle.
    Triangle,
    /// One value for the whole source.
    Constant,
}

/// A named attribute materials read (`Cd`, `roughness`).
#[derive(Clone, Debug, PartialEq)]
pub struct MeshAttribute {
    /// Its name, as Attribute nodes name it.
    pub name: Arc<str>,
    /// Where its values sit.
    pub domain: AttributeDomain,
    /// Floats a value, 1 to 4.
    pub components: u32,
    /// Whether it holds colors (in the render's space: see
    /// [`TriangleSource::attributes`]).
    pub color: bool,
    /// Whether it holds integers (ids, exact below 2^24): hits take the
    /// nearest corner's value instead of a blend.
    pub integer: bool,
    /// The values, `components` floats each, interleaved.
    pub values: Arc<[f32]>,
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

/// What a value renders as.
#[derive(Clone)]
pub enum RenderItem {
    /// Triangles placed once or many times.
    Triangles {
        /// A key stable across recooks for this item of its object (a CAD
        /// body's index): the host matches items of successive values by
        /// it.
        part: u32,
        /// The triangles.
        source: Arc<dyn TriangleSource>,
        /// Placements relative to the object, which the host places in the
        /// world: the first is the item itself, the rest are copies that
        /// share its acceleration structure. Empty: not rendered.
        placements: Arc<[Mat4]>,
        /// Its name (a body's), the name Cryptomatte gives the object.
        label: Option<Arc<str>>,
    },
}

impl RenderItem {
    /// `source` as part `part`, placed once where the object is.
    pub fn triangles(part: u32, source: Arc<dyn TriangleSource>) -> Self {
        Self::Triangles {
            part,
            source,
            placements: Arc::from([Mat4::IDENTITY]),
            label: None,
        }
    }
}

impl fmt::Debug for RenderItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Triangles {
                part,
                source,
                placements,
                label,
            } => write!(
                f,
                "Triangles(part {part}, {} × {}, {label:?})",
                source.triangle_count(),
                placements.len()
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranges_join_overlapping_and_touching_spans() {
        let mut r = Ranges::of(10..20);
        r.add(30..40);
        r.add(20..25);
        assert_eq!(r.spans(), &[10..25, 30..40]);
        r.add(5..31);
        assert_eq!(r.spans().to_vec(), vec![5..40]);
        r.add(3..3);
        assert_eq!(r.spans().len(), 1);
        let mut whole = Ranges::new();
        assert!(whole.is_empty());
        whole.merge(&Ranges::whole());
        assert!(whole.is_whole() && whole.spans().is_empty());
        whole.add(0..4);
        assert!(whole.is_whole());
    }
}
