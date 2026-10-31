//! [`Class`] and [`TypeInfo`]: which elements an attribute has a value
//! for, and what its vectors mean.

/// The elements an attribute or group has one value for.
///
/// Which classes a value has, and how many elements each holds, is the
/// value's own: [`Polygons::count`](crate::Polygons::count),
/// [`Curves::count`](crate::Curves::count). New classes may be added; a
/// reader that doesn't know one leaves its attributes as they are (they
/// pass through edits unchanged) and shows them as unreadable.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum Class {
    /// The whole value: one element.
    Detail,
    /// Each object (a B-rep's bodies).
    Object,
    /// Each placement of an instanced object.
    Instance,
    /// Each primitive: a polygon or polyline, or a curve.
    Primitive,
    /// Each primitive again, in a namespace of its own: the pieces a
    /// kernel groups primitives into. A reader wanting a primitive's value
    /// reads [`Primitive`](Class::Primitive) first, then `Piece`.
    Piece,
    /// Each unique edge, in the canonical edge order
    /// ([`Topology::edges`](crate::Topology::edges)).
    Edge,
    /// Each point.
    Point,
    /// Each corner of a primitive (a vertex), in primitive order.
    Vertex,
}

impl Class {
    /// Every class, in order.
    pub const ALL: [Class; 8] = [
        Class::Detail,
        Class::Object,
        Class::Instance,
        Class::Primitive,
        Class::Piece,
        Class::Edge,
        Class::Point,
        Class::Vertex,
    ];

    /// Its name in messages and files: `detail`, `object`, `instance`,
    /// `primitive`, `piece`, `edge`, `point`, `vertex`. Never renamed.
    pub fn name(self) -> &'static str {
        match self {
            Class::Detail => "detail",
            Class::Object => "object",
            Class::Instance => "instance",
            Class::Primitive => "primitive",
            Class::Piece => "piece",
            Class::Edge => "edge",
            Class::Point => "point",
            Class::Vertex => "vertex",
        }
    }

    /// The class named `name` ([`Class::name`]).
    pub fn from_name(name: &str) -> Option<Class> {
        Class::ALL.into_iter().find(|c| c.name() == name)
    }
}

/// What a vector attribute's values mean, so tools transform them right
/// (a point moves, a normal turns, a texture coordinate stays). New kinds
/// may be added; a reader that doesn't know one treats the values as
/// [`TypeInfo::None`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum TypeInfo {
    /// Plain numbers.
    #[default]
    None,
    /// Positions: transformed as points.
    Point,
    /// Directions with a length: transformed as vectors.
    Vector,
    /// Surface normals: transformed by the inverse transpose.
    Normal,
    /// Colors: scene-linear in the project's working space, never
    /// transformed.
    Color,
    /// Texture coordinates (u, v, w): never transformed.
    TextureCoord,
}

impl TypeInfo {
    /// Every kind, in order.
    pub const ALL: [TypeInfo; 6] = [
        TypeInfo::None,
        TypeInfo::Point,
        TypeInfo::Vector,
        TypeInfo::Normal,
        TypeInfo::Color,
        TypeInfo::TextureCoord,
    ];

    /// Its name in messages and files: `none`, `point`, `vector`,
    /// `normal`, `color`, `texture_coord`. Never renamed.
    pub fn name(self) -> &'static str {
        match self {
            TypeInfo::None => "none",
            TypeInfo::Point => "point",
            TypeInfo::Vector => "vector",
            TypeInfo::Normal => "normal",
            TypeInfo::Color => "color",
            TypeInfo::TextureCoord => "texture_coord",
        }
    }

    /// The kind named `name` ([`TypeInfo::name`]).
    pub fn from_name(name: &str) -> Option<TypeInfo> {
        TypeInfo::ALL.into_iter().find(|t| t.name() == name)
    }
}
