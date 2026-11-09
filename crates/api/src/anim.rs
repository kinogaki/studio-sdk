//! Keys: a channel's value at a moment of the scene, and how it runs to
//! the next key. Every component of a parameter is a channel of its own (a
//! vector's X, Y and Z, a color's R, G and B); a parameter of one value (a
//! number, a toggle, a ramp) is one channel. Studio's core keeps a node's
//! channels by parameter and evaluates them at the scene's time before the
//! node cooks, so a cook reads plain [`Value`]s.
use crate::Time;
use crate::params::Value;

/// One key of a channel: at `time`, the channel is `value`; `interp` says
/// how it runs from here to the next key, `handles` the curve's tangents
/// either side of it where they are Bézier.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq)]
pub struct Key {
    /// When.
    pub time: Time,
    /// The channel's value then: a number for a component, else of the
    /// variant its parameter's kind holds.
    pub value: Value,
    /// How the channel runs from this key to the next.
    pub interp: Interp,
    /// Its tangents, read on the sides whose segment is Bézier.
    pub handles: Handles,
}

impl Key {
    /// A key at `time` holding `value`, running on by `interp`, its
    /// tangents automatic.
    pub fn new(time: Time, value: Value, interp: Interp) -> Self {
        Self {
            time,
            value,
            interp,
            handles: Handles::Auto,
        }
    }

    /// It, with `handles`.
    pub fn with(mut self, handles: Handles) -> Self {
        self.handles = handles;
        self
    }
}

/// How a channel runs from a key to the next. Before the first key and
/// after the last a channel holds their values. Kinds whose values don't
/// blend (toggles, choices) hold whatever a key says.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Interp {
    /// The key's value up to the next key.
    Held,
    /// A straight line to the next key's value.
    Linear,
    /// A cubic Bézier through the keys' handles ([`Handles`]).
    #[default]
    Bezier,
}

/// A key's tangents: each side an offset `[Δtime in seconds, Δvalue]` from
/// the key, so its length counts (weighted, as Blender's and USD's splines
/// are). A closed set, as lib-prism's keyframes hold it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Handles {
    /// Made from the neighbouring keys, clamped: a peak or a trough is
    /// flat, and no segment overshoots its keys' values.
    #[default]
    Auto,
    /// Stored, the two sides collinear: moving one turns the other.
    Aligned {
        /// The incoming side.
        left: [f64; 2],
        /// The outgoing side.
        right: [f64; 2],
    },
    /// Stored, the two sides independent (broken).
    Free {
        /// The incoming side.
        left: [f64; 2],
        /// The outgoing side.
        right: [f64; 2],
    },
}

impl Handles {
    /// The incoming side, when stored.
    pub fn left(self) -> Option<[f64; 2]> {
        match self {
            Handles::Auto => None,
            Handles::Aligned { left, .. } | Handles::Free { left, .. } => Some(left),
        }
    }

    /// The outgoing side, when stored.
    pub fn right(self) -> Option<[f64; 2]> {
        match self {
            Handles::Auto => None,
            Handles::Aligned { right, .. } | Handles::Free { right, .. } => Some(right),
        }
    }
}
