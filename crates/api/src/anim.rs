//! Keys: a parameter's value at a moment of the scene, and how it runs to
//! the next key. Studio's core keeps a node's keys by parameter and
//! evaluates them at the scene's time before the node cooks, so a cook
//! reads plain [`Value`]s.
use crate::Time;
use crate::params::Value;

/// One key of a parameter's channel: at `time`, the parameter is `value`;
/// `interp` says how it runs from here to the next key.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq)]
pub struct Key {
    /// When.
    pub time: Time,
    /// The parameter's value then, of the variant its kind holds.
    pub value: Value,
    /// How the channel runs from this key to the next.
    pub interp: Interp,
}

impl Key {
    /// A key at `time` holding `value`, running on by `interp`.
    pub fn new(time: Time, value: Value, interp: Interp) -> Self {
        Self {
            time,
            value,
            interp,
        }
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
    #[default]
    Linear,
    /// A smooth cubic through the keys. Its tangents are automatic: each
    /// key's points from the key before it to the key after it.
    Bezier,
}
