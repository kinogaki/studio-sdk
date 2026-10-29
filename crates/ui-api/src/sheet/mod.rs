//! Property sheets as data: what an inspector, a tool's card or a
//! settings page shows, drawn by the host.
//!
//! An owner (a node's parameters, a tool, a settings page) describes its
//! properties as a [`Schema`]: sections and subsections of typed
//! [`Field`]s, each with its current value, whether it is enabled (and why
//! not) and its hover text. The host draws it with
//! [`Ui::sheet`](crate::Ui::sheet), lets the user edit the values in place
//! and reports [`Edits`]: which fields changed and which [`Action`]s were
//! taken; the owner applies them. Schemas hold no host state and no
//! callbacks: rebuild one from the data each frame.
//!
//! `K` is the owner's key for a field; the host's sheet takes
//! [`FieldKey`]s, and [`UiExt::sheet_keyed`](crate::UiExt::sheet_keyed)
//! converts. Values are the API's own [`Value`](kinogaki_studio_api::Value)s,
//! so a node's parameters ([`kinogaki_studio_api::params::Schema`]) become a
//! sheet with `From`.
//!
//! Every kind of value has its field kind here, drawn by Studio's own
//! widget for it: numbers, vectors, colors, linked channels, choices, text,
//! code, secrets, file paths (with the host's file dialog), paths in the
//! scene's tree, ramps, tone curves, palettes and material inputs'
//! connection buttons ([`Socket`]).
mod code;
mod edits;
mod from_params;
mod keyed;
mod kind;
mod number;
mod schema;
pub mod tone_curve;

pub use code::{CodeLanguage, Completion, Completions, PlainText, Token, TokenClass};
pub use edits::{Action, Edits};
pub use from_params::RAMP_INTERPOLATIONS;
pub use keyed::{FieldKey, Keyed};
pub use kind::{
    Chip, CurveChannel, FileDialog, FileFilter, Kind, PathChoice, Socket, SocketKind, Trailing,
};
pub use number::{Number, NumberStyle};
pub use schema::{Field, Item, Schema, Section};

use crate::id::Id;
use std::borrow::Cow;

/// Labels and other text: usually static, built when it must be.
pub type Text = Cow<'static, str>;

/// How the host draws a sheet.
#[derive(Clone, Copy)]
pub struct SheetStyle<'a> {
    /// Keeps the rows' widget state apart from other sheets'.
    pub id: Id,
    /// Tighter rows without separators, for floating cards (a docked
    /// inspector's full rows otherwise).
    pub compact: bool,
    /// How code fields are colored, completed and explained; `None` is
    /// plain text.
    pub code: Option<&'a dyn CodeLanguage>,
}

impl<'a> SheetStyle<'a> {
    /// A docked sheet with plain-text code fields, its rows under `id`.
    pub fn new(id: Id) -> Self {
        SheetStyle {
            id,
            compact: false,
            code: None,
        }
    }

    /// With a floating card's compact rows.
    pub fn compact(mut self) -> Self {
        self.compact = true;
        self
    }

    /// With code fields in `language`.
    pub fn code(mut self, language: &'a dyn CodeLanguage) -> Self {
        self.code = Some(language);
        self
    }
}

impl std::fmt::Debug for SheetStyle<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SheetStyle")
            .field("id", &self.id)
            .field("compact", &self.compact)
            .field("code", &self.code.is_some())
            .finish()
    }
}
