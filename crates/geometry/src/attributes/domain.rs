//! [`Domain`]: which elements an attribute or group has one value for.

/// The elements an attribute or group has one value per.
///
/// Which domains a value has, and how many elements each holds, is the
/// value's own: [`Geometry::count`](crate::Geometry::count),
/// [`Body::count`](crate::brep::Body::count). A value refuses an attribute
/// on a domain it doesn't have. New domains may be added; a reader that
/// doesn't know one leaves its attributes as they are (they pass through
/// edits unchanged) and shows them as unreadable.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum Domain {
    /// The whole value: one element. An array attribute here is a list
    /// (an L-system's rules, a node's metadata).
    Detail,
    /// Each object: a B-rep body (one element in its own attributes).
    Object,
    /// Each placement of an instanced object.
    Instance,
    /// Each primitive: a polygon or polyline, a curve, or a B-rep body's
    /// face.
    Primitive,
    /// Each edge, in the value's edge order: a polygon value's canonical
    /// order ([`Primitives::edges`](crate::Primitives::edges)), a B-rep body's
    /// edge ids.
    Edge,
    /// Each point: a position, or a B-rep body's vertex.
    Point,
    /// Each corner of a primitive (a vertex), in primitive order.
    Vertex,
}

impl Domain {
    /// Every domain, in order.
    pub const ALL: [Domain; 7] = [
        Domain::Detail,
        Domain::Object,
        Domain::Instance,
        Domain::Primitive,
        Domain::Edge,
        Domain::Point,
        Domain::Vertex,
    ];

    /// Its name in messages and files: `detail`, `object`, `instance`,
    /// `primitive`, `edge`, `point`, `vertex`. Never renamed.
    pub fn name(self) -> &'static str {
        match self {
            Domain::Detail => "detail",
            Domain::Object => "object",
            Domain::Instance => "instance",
            Domain::Primitive => "primitive",
            Domain::Edge => "edge",
            Domain::Point => "point",
            Domain::Vertex => "vertex",
        }
    }

    /// The domain named `name` ([`Domain::name`]).
    pub fn from_name(name: &str) -> Option<Domain> {
        Domain::ALL.into_iter().find(|d| d.name() == name)
    }
}
