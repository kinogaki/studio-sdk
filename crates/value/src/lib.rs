//! Kinogaki Studio's value model: the one way the SDK says what a value
//! is and holds.
//!
//! | Module | What |
//! |---|---|
//! | [`id`] | declared ids (`<vendor>.<package>.<name>`), one type per kind of thing |
//! | [`data`] | [`Data`]: a type-erased value of a registered data type, and a kernel's [`Typed`] handle |
//! | [`value`] | [`Value`]: what a parameter, a render setting, a declared prim's value or a material port holds; [`Parameters`], values by name |
//! | [`reads`] | [`ParametersExt`]: a value of [`Parameters`] read by name as its variant, or an error naming it |
//! | [`kind`] | [`ValueKind`]: what a value holds, its limits and the options its kind takes ([`Number`], [`Unit`], [`Choice`]) |
//! | [`look`] | [`NumberLook`]: how a row of numbers shows and moves, carried beside a kind by what has a row |
//! | [`condition`] | [`Condition`]: when something applies, as values holding values |
//! | [`ramp`], [`tone_curve`] | the structured values: [`Ramp`] and [`ToneCurve`] |
//! | [`reference`](mod@reference) | [`RefFilter`]: what a reference may name; [`ObjectRef`]: an object, by the host's id |
//!
//! Integers are exact (`i64`), numbers `f64` wherever they are held; a
//! choice is held and saved by its key; a unit's suffix is owned text; an
//! image is authored as its path and reaches an engine as its texture.
//! How a value is presented (segments or a menu, a remembered tool
//! setting, a read-only row) is the UI layer's; a number's look travels
//! beside its kind, never in it.
pub mod condition;
pub mod data;
pub mod id;
pub mod kind;
pub mod look;
pub mod ramp;
pub mod reads;
pub mod reference;
pub mod tone_curve;
pub mod value;

pub use condition::Condition;
pub use data::{Data, Typed};
pub use id::{
    ContextId, CreationId, DataTypeId, EngineId, FormatId, MesherId, NodeTypeId, ObjectKindId,
    PackageId, PrimTypeId, SlotId, StoredId,
};
pub use kind::{Choice, CurveChannel, Number, Unit, ValueKind};
pub use look::{NumberLook, NumberStyle};
pub use ramp::Ramp;
pub use reads::ParametersExt;
pub use reference::{Fits, ObjectRef, RefFilter};
pub use tone_curve::ToneCurve;
pub use value::{Parameters, Value};
