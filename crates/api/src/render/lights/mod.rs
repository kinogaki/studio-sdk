//! Lights, the SDK's side of them in one place. The schema of the standard
//! UsdLux lights and their decoding are render-api's
//! ([`kinogaki_render_api::standard`]); here are what the rest of Studio and
//! its plugins hold of a light:
//!
//! | Part | What |
//! |---|---|
//! | `value` | a light as a data type: [`light_value`] makes one, [`LightCodec`] saves it (an import's lights, a reference's) |
//! | `kind` | [`DeclaredKind`]: an engine's object kind of lights (and of its atmosphere), its objects prims of the types it declares |
//! | `view` | how a kind's lights show in the viewport: their glyphs and handles |
mod kind;
mod value;
mod view;

pub use kind::DeclaredKind;
pub use value::{LightCodec, light_value};
