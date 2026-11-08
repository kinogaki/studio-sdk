//! Settings: what every view shares ([`SceneSettings`]) and the
//! values an engine describes itself (a [`Schema`] of parameters, kept by
//! name). A schema describes the values of the prim types an engine
//! declares too ([`crate::PrimType`]): a light's size, a fog's density.
//! A parameter says what a value holds ([`ValueKind`](kinogaki_value::ValueKind),
//! the SDK's one value model), and when it is shown or applies, so a host
//! draws any engine's rows without knowing it; the schema's
//! [`fit`](kinogaki_value::Schema::fit) and [`validate`](kinogaki_value::Schema::validate) take values to
//! it.
//!
//! [`Schema`]: kinogaki_value::Schema
//!
//! Values are authored [`Value`](kinogaki_value::Value)s by name ([`Parameters`](kinogaki_value::Parameters)). An image value
//! of a declared prim ([`ValueKind::File`](kinogaki_value::ValueKind::File)) is authored as its
//! file's path, or `op:` and the path of the host's Image node it reads, and a material node's
//! image is the host's Image node wired
//! into it (a `filename` input); an engine receives the
//! Texture prim the host read either to, in the prim's or node's `textures`,
//! by the value's or input's name.

use kinogaki_geometry::{Rate, Units};
use std::collections::BTreeMap;

use crate::geometry::image::TextureImage;
use crate::id::Id;
use crate::prim::Camera;

/// The Texture prims the host read images to, by the value's or input's
/// name: what an engine reads for each image (one missing: no image, or one
/// not read yet).
pub(crate) type Textures = BTreeMap<String, Id<TextureImage>>;

/// What every view of an engine shares. What differs between views travels
/// with the view ([`crate::ViewDesc::values`]).
#[derive(Clone, Debug, PartialEq)]
pub struct SceneSettings {
    /// The Camera prim whose shutter placements blur across (none: nothing
    /// blurs).
    pub shutter: Option<Id<Camera>>,
    /// The scene's units: what real sizes (a lens's millimetres, an
    /// anodized part's grain) are measured against, and the axis that is
    /// up to sky light and height fog.
    pub units: Units,
    /// The scene's frames a second: how long a frame is, which sample times
    /// (in frames) and velocities (in scene units a second) meet at.
    pub rate: Rate,
}

impl Default for SceneSettings {
    fn default() -> SceneSettings {
        SceneSettings {
            shutter: None,
            units: Units::METERS,
            rate: Rate::FILM,
        }
    }
}

/// The names of the settings a host's render output holds of its own beside
/// its engine's: the image's size, the camera, the output file and the motion
/// samples. An engine names none of its settings so (the registry refuses
/// one that does).
pub const OUTPUT_SETTINGS: [&str; 4] = ["resolution", "camera", "output", "motionSamples"];
