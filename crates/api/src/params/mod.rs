//! Parameters: what a node's or a format's settings hold, the value
//! model's [`Value`]s by name ([`Parameters`]) of [`ValueKind`]s; how the
//! host presents them ([`Schema`]); and the [`Diagnostic`]s cooks and
//! readers report.
//!
//! These are the API's own types. Studio draws a [`Schema`] with its
//! inspector and stores [`Value`]s in projects; a package never sees the
//! host's UI types.
mod diagnostic;
mod schema;

pub use diagnostic::{Diagnostic, Severity, Span};
pub use kinogaki_value::{
    Choice, CurveChannel, Fits, Number, Parameters, RefFilter, Unit, Value, ValueKind, ramp,
    tone_curve,
};
pub use schema::{Button, Parameter, Schema, SchemaItem, Section};
