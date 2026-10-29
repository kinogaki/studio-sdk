//! Kinogaki Studio's UI layer for plugins.
//!
//! **The host draws; plugins never see its UI toolkit.** A plugin's
//! panels, tool cards, overlays and settings pages are written against
//! this crate's own types and traits, and Studio implements them over its
//! private UI library. So the SDK stays self-contained (this crate depends
//! on the standard library and `kinogaki-studio-api` alone), and Studio
//! can change how it draws without breaking a plugin.
//!
//! What a plugin uses, from the most declarative to the most direct:
//!
//! - **Data the host draws.** A [`sheet::Schema`] of typed fields is an
//!   inspector, a tool's card body or a settings page
//!   ([`Ui::sheet`], [`UiExt::sheet_keyed`]); a [`CardSpec`] or a
//!   [`PopoverSpec`] frames floating content; [`ValueFields`] are the
//!   heads-up boxes a drawing tool types into ([`Ui::value_fields`]).
//! - **Immediate widgets** on [`Ui`]: labels, buttons, toggles, number
//!   fields, sliders, text fields, choices, swatches, menus, and layout
//!   (`horizontal`, `vertical`, `child`, `scroll_vertical`, `enabled`).
//! - **A [`Painter`]** for overlays: segments, circles, rectangles,
//!   polylines, text and icons, single or in batches, measured text, and
//!   reserved [`Slot`]s. Meshes and surfaces are not painted: they are the
//!   data types' display surfaces, drawn by the host's viewport.
//! - **[`Input`]**: this frame's pointer, keys and typing as plain data.
//!
//! And the plain values they exchange: [`geom`] (points, vectors,
//! rectangles, alignment, in `f32` screen points), [`Color`] (premultiplied
//! sRGBA bytes), the host-filled [`Theme`], [`Font`]s by role and icons by
//! name (names come from the host's icon set; an unknown name draws
//! nothing), and [`Id`]s.
//!
//! Pure viewport maths the host's own tools use is here too, so a
//! plugin's tools behave exactly like Studio's: the transform gizmo's
//! geometry and hit testing ([`gizmo`]) and point-label placement
//! ([`labels`]), over a host-provided [`Projection`].
//!
//! **The host interface** ([`host`]) is what a package's UI asks of
//! Studio: undo and gestures, the session's slots, selections, the
//! document, views, status, the shell, the transform tool and settings,
//! reached through one call context ([`host::Cx`]). **A package's UI**
//! ([`package`]) is its contexts, each a set of parts (levels, a viewport
//! layer, an inspector, a vocabulary…) over the package's own state,
//! which the host keeps and lends for one call at a time.
//!
//! [`testing`] has headless fakes ([`testing::RecordingUi`],
//! [`testing::RecordingPainter`], and [`testing::FakeHost`] with
//! [`testing::Hosting`] for a package's parts) for plugin unit tests.
mod card;
mod color;
pub mod geom;
pub mod gizmo;
pub mod host;
mod id;
mod input;
pub mod labels;
pub mod package;
mod paint;
pub mod projection;
mod response;
pub mod sheet;
pub mod testing;
mod text;
mod theme;
mod ui;
pub mod value_fields;
mod vec3;
mod widgets;

pub use card::{CardResponse, CardSpec, Order, PopoverLook, PopoverSpec};
pub use color::Color;
pub use geom::{Align, Align2, Pos, Rect, Vec2, pos, vec2};
pub use id::Id;
pub use input::{Event, Input, Key, KeyFilter, Modifiers, Pointer, PointerButton, ScrollUnit};
pub use paint::{
    Circle, IconShape, Painter, RectShape, Segment, Shape, Slot, Stroke, StrokeKind, TextShape,
};
pub use projection::{ClipMatrix, Projection, Ray};
pub use response::{InnerResponse, Response, Sense};
pub use sheet::{Edits, FieldKey, SheetStyle};
pub use text::{Font, FontRole, TextStyle};
pub use theme::{Marks, Metrics, Radii, Theme};
pub use ui::{Body, RowMenu, Ui, UiExt};
pub use value_fields::{ValueField, ValueFields};
pub use widgets::{ButtonSpec, ChoiceLook, MenuItem, ScrollSpec, TextEditSpec};
