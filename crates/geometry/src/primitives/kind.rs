//! [`Kind`] and [`Basis`]: what a primitive is, in one byte.
use std::fmt;

/// How a primitive passes through its points.
///
/// New bases may be added. A reader that doesn't know a primitive's basis
/// treats it as read-only: it passes through unchanged, displays as its
/// control hull and renders as nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum Basis {
    /// Straight segments between consecutive points: polygons and
    /// polylines. Degree 1.
    Linear,
    /// Bézier segments of the primitive's degree, sharing end points: an
    /// open curve has `k·degree + 1` control points, a closed one
    /// `k·degree`.
    Bezier,
    /// Uniform, non-rational B-splines of the primitive's degree: clamped
    /// when open (they start and end on their end points), wrapping when
    /// closed.
    BSpline,
    /// Catmull-Rom splines through every point (an open curve's ends
    /// reflect their neighbours). Degree 3.
    CatmullRom,
    /// Rational B-splines with explicit knots and optional weights.
    Nurbs,
}

impl Basis {
    /// Every basis, in order.
    pub const ALL: [Basis; 5] = [
        Basis::Linear,
        Basis::Bezier,
        Basis::BSpline,
        Basis::CatmullRom,
        Basis::Nurbs,
    ];

    /// Its name in messages and files: `linear`, `bezier`, `bspline`,
    /// `catmull_rom`, `nurbs`. Never renamed.
    pub fn name(self) -> &'static str {
        match self {
            Basis::Linear => "linear",
            Basis::Bezier => "bezier",
            Basis::BSpline => "bspline",
            Basis::CatmullRom => "catmull_rom",
            Basis::Nurbs => "nurbs",
        }
    }

    /// The basis named `name` ([`Basis::name`]).
    pub fn from_name(name: &str) -> Option<Basis> {
        Basis::ALL.into_iter().find(|b| b.name() == name)
    }

    /// Its code in a [`Kind`]'s low bits.
    fn code(self) -> u8 {
        match self {
            Basis::Linear => 0,
            Basis::Bezier => 1,
            Basis::BSpline => 2,
            Basis::CatmullRom => 3,
            Basis::Nurbs => 4,
        }
    }
}

/// A primitive's kind: its [`Basis`] in the low three bits, closed in
/// bit 3. A closed linear primitive is a polygon, an open one a polyline;
/// a curve is any other basis, open or closed (periodic). The byte is
/// what files hold, one per primitive.
///
/// ```
/// use kinogaki_geometry::{Basis, Kind};
///
/// assert!(Kind::POLYGON.is_polygon() && Kind::POLYGON.closed());
/// assert_eq!(Kind::POLYLINE.basis(), Basis::Linear);
/// let ring = Kind::curve(Basis::BSpline, true);
/// assert_eq!((ring.basis(), ring.closed(), ring.is_polygon()), (Basis::BSpline, true, false));
/// assert_eq!(Kind::from_byte(ring.byte()), Some(ring));
/// assert_eq!(Kind::from_byte(0x17), None);
/// ```
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Hash, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Kind(u8);

/// The closed bit.
const CLOSED: u8 = 1 << 3;

impl Kind {
    /// A closed linear primitive.
    pub const POLYGON: Kind = Kind(CLOSED);
    /// An open linear primitive.
    pub const POLYLINE: Kind = Kind(0);

    /// A primitive of `basis`, closed (periodic) or open.
    pub fn curve(basis: Basis, closed: bool) -> Kind {
        Kind(basis.code() | if closed { CLOSED } else { 0 })
    }

    /// The kind a byte holds, when it names one.
    pub fn from_byte(byte: u8) -> Option<Kind> {
        let basis = Basis::ALL.into_iter().find(|b| b.code() == byte & 0b111)?;
        (byte & !(0b111 | CLOSED) == 0).then(|| Kind::curve(basis, byte & CLOSED != 0))
    }

    /// Its byte.
    pub fn byte(self) -> u8 {
        self.0
    }

    /// How it passes through its points.
    pub fn basis(self) -> Basis {
        Basis::ALL
            .into_iter()
            .find(|b| b.code() == self.0 & 0b111)
            .expect("a kind holds a basis")
    }

    /// Whether it closes on its first point.
    pub fn closed(self) -> bool {
        self.0 & CLOSED != 0
    }

    /// Whether it is a polygon: closed and linear.
    pub fn is_polygon(self) -> bool {
        self == Kind::POLYGON
    }

    /// Whether it is linear (a polygon or a polyline).
    pub(crate) fn is_linear(self) -> bool {
        self.0 & 0b111 == 0
    }
}

impl fmt::Debug for Kind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (self.basis(), self.closed()) {
            (Basis::Linear, true) => write!(f, "Polygon"),
            (Basis::Linear, false) => write!(f, "Polyline"),
            (basis, closed) => write!(f, "{}{}", basis.name(), if closed { " closed" } else { "" }),
        }
    }
}
