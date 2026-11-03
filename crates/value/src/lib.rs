//! Kinogaki Studio's value model: the one way the SDK says what a value
//! is and holds.
//!
//! | Module | What |
//! |---|---|
//! | [`id`] | declared ids (`<vendor>.<package>.<name>`), one type per kind of thing |
//! | [`data`] | [`Data`]: a type-erased value of a registered data type, and a kernel's [`Typed`] handle |
//! | [`value`] | [`Value`]: what a parameter, a render setting, a declared prim's value or a material port holds; [`Parameters`], values by name |
//! | [`kind`] | [`ValueKind`]: what a value holds, its limits and the options its kind takes ([`Number`], [`Unit`], [`Choice`]) |
//! | [`condition`] | [`Condition`]: when something applies, as values holding values |
//! | [`ramp`], [`tone_curve`] | the structured values: [`Ramp`] and [`ToneCurve`] |
//! | [`reference`](mod@reference) | [`RefFilter`]: what a reference may name |
//!
//! Integers are exact (`i64`), numbers `f64` wherever they are held; a
//! choice is held and saved by its key; a unit's suffix is owned text; an
//! image is authored as its path and reaches an engine as its texture.
//! How a value is presented (segments or a menu, a remembered tool
//! setting, a read-only row) is the UI layer's.
pub mod condition;
pub mod data;
pub mod id;
pub mod kind;
pub mod ramp;
pub mod reference;
pub mod tone_curve;
pub mod value;

pub use condition::Condition;
pub use data::{Data, Typed};
pub use id::{
    ContextId, CreationId, DataTypeId, FormatId, MesherId, NodeTypeId, ObjectKindId, PackageId,
    SlotId, StoredId,
};
pub use kind::{Choice, CurveChannel, Number, Unit, ValueKind};
pub use ramp::Ramp;
pub use reference::{Fits, RefFilter};
pub use tone_curve::ToneCurve;
pub use value::{Parameters, Value};
