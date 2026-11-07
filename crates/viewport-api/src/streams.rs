//! [`Streams`]: the parts of a surface packet only some views draw, which
//! its maker builds when a host asks for them.
//!
//! Every packet holds what any view draws and picks by: positions, the
//! vertex records with their normals and ids, the triangles, the point
//! markers (point picks and point snapping find points through them, at any
//! level and on any object) and the line classes that are the value's own
//! drawing (polylines, curves, a CAD body's hulls). The rest is drawn by
//! some views only, and costs time and memory in proportion to the value:
//! those are the streams. A host asks for the streams its views draw
//! (`kinogaki-studio-api`'s `DisplayRequest::streams`), and asks again with
//! more when a view starts drawing one; a maker adds the missing ones to
//! what it drew before, the packet's update rewriting them alone.
use std::ops::{BitOr, BitOrAssign};

/// A set of a packet's optional streams.
///
/// ```
/// use kinogaki_viewport_api::Streams;
///
/// let asked = Streams::WIRES | Streams::CURVATURE;
/// assert!(asked.contains(Streams::WIRES));
/// assert_eq!(asked.without(Streams::WIRES), Streams::CURVATURE);
/// assert!(Streams::NONE.is_empty() && Streams::ALL.contains(asked));
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Streams(u8);

impl Streams {
    /// None of them.
    pub const NONE: Self = Self(0);
    /// The wire classes' segments ([`LineStyle::wire`](crate::LineStyle)):
    /// a surface's edges, drawn over its faces where a view shows wires,
    /// alone in a wireframe view, and picked at an edge level. Without
    /// them a packet holds its other classes alone, with the same ids.
    pub const WIRES: Self = Self(1);
    /// The vertex records' curvature ([`Vertex::curvature`](crate::Vertex),
    /// [`SurfaceSource::curvature_range`](crate::SurfaceSource)), drawn
    /// where a view shades by it; zero without it.
    pub const CURVATURE: Self = Self(2);
    /// Every one.
    pub const ALL: Self = Self(3);

    /// Those of either.
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Whether every one of `other` is here.
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// These but `other`'s: what a packet holding `other` lacks of them.
    pub const fn without(self, other: Self) -> Self {
        Self(self.0 & !other.0)
    }

    /// Whether there are none.
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

impl BitOr for Streams {
    type Output = Self;
    fn bitor(self, other: Self) -> Self {
        self.union(other)
    }
}

impl BitOrAssign for Streams {
    fn bitor_assign(&mut self, other: Self) {
        *self = self.union(other);
    }
}
