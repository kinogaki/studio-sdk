//! Parameters: what a node's or a format's settings hold, the value
//! model's [`Value`]s by name ([`Parameters`]) of [`ValueKind`]s; how the
//! host presents them ([`Schema`]).
//!
//! These are the API's own types. Studio draws a [`Schema`] with its
//! inspector and stores [`Value`]s in projects; a package never sees the
//! host's UI types.
pub mod code;
mod schema;

pub use kinogaki_value::{
    Choice, CurveChannel, Fits, Number, NumberLook, NumberStyle, Parameter, Parameters,
    ParametersExt, RefFilter, Section, Unit, Value, ValueKind, ramp, tone_curve,
};
pub use schema::{Button, Schema, SchemaItem};
