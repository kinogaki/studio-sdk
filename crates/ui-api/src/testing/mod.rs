//! Headless fakes of the host's UI for plugin unit tests:
//! [`RecordingUi`] and [`RecordingPainter`] record what a plugin asks to
//! show and draw, and play back scripted user actions, with no window and
//! no GPU; [`FakeHost`] stands in for Studio's host interface, and
//! [`Hosting`] calls a registered package's parts on it as the host does.
//!
//! ```
//! use studio_ui_api::testing::{Call, RecordingUi};
//! use studio_ui_api::{ButtonSpec, Pos, Rect, Ui, Vec2};
//!
//! let mut ui = RecordingUi::new(Rect::from_min_size(Pos::ZERO, Vec2::new(300.0, 200.0)));
//! ui.click("Apply");
//! assert!(ui.button(&ButtonSpec::new("Apply")).clicked);
//! assert_eq!(ui.calls(), [Call::Button("Apply".into())]);
//! ```
mod host;
mod painter;
mod ui;
mod ui_impl;

pub use host::{FakeHost, FakeObject, HostCall, Hosting};
pub use painter::{GLYPH_ADVANCE, RecordingPainter};
pub use ui::{Call, RecordingUi};
