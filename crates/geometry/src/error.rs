//! [`GeometryError`]: why a standard value was refused.
use std::fmt;

/// Why a builder or constructor refused a value: what failed, and which
/// element when one element is at fault. Nothing is built when it fails.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GeometryError {
    kind: ErrorKind,
    at: Option<usize>,
    message: String,
}

/// What kind of rule a refused value broke. New kinds may be added; a
/// caller that doesn't know one treats it as any other refusal and shows
/// the message.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ErrorKind {
    /// A count is over a product limit ([`crate::limits`]).
    Limit,
    /// The primitives' offsets or vertices break a topology rule.
    Topology,
    /// A position is not finite, or too large.
    Position,
    /// An attribute or group is misnamed, mistyped or missized.
    Attribute,
    /// A curve breaks its basis's rules (degree, counts, knots, weights).
    Curve,
    /// A selection's indices or active element are inconsistent.
    Selection,
    /// A tolerance or a render detail is out of its range.
    Range,
    /// A polygon can't be triangulated strictly: its outline crosses or
    /// collapses ([`crate::Polygons::triangulate_strict`]).
    Triangulation,
}

impl GeometryError {
    /// A refusal of kind `kind`, at element `at` when one element is at
    /// fault, saying `message`.
    pub(crate) fn new(kind: ErrorKind, at: Option<usize>, message: impl Into<String>) -> Self {
        Self {
            kind,
            at,
            message: message.into(),
        }
    }

    /// A refusal blaming element `at`.
    pub(crate) fn at(kind: ErrorKind, at: usize, message: impl Into<String>) -> Self {
        Self::new(kind, Some(at), message)
    }

    /// A refusal blaming no one element.
    pub(crate) fn whole(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self::new(kind, None, message)
    }

    /// The kind of rule broken.
    pub fn kind(&self) -> ErrorKind {
        self.kind
    }

    /// The element at fault (a primitive, point, curve or index, as the
    /// message names it), when one is.
    pub fn element(&self) -> Option<usize> {
        self.at
    }

    /// What failed, for the user ("polygon 12 repeats a point").
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for GeometryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for GeometryError {}
