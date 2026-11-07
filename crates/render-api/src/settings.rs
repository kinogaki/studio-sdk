//! Settings: what every view shares ([`SceneSettings`]) and the
//! values an engine describes itself ([`Parameter`]s in [`Section`]s, kept
//! by name). The same parameters describe the values of the prim types an
//! engine declares ([`crate::PrimType`]): a light's size, a fog's density.
//! A parameter says what a value holds ([`ValueKind`](kinogaki_value::ValueKind),
//! the SDK's one value model), and when it is shown or applies, so a host
//! draws any engine's rows without knowing it.
//!
//! [`Section`]: kinogaki_value::Section
//!
//! Values are authored [`Value`](kinogaki_value::Value)s by name ([`Parameters`]). An image value
//! of a declared prim ([`ValueKind::File`](kinogaki_value::ValueKind::File)) is authored as its
//! file's path, and a material node's image is the host's Image node wired
//! into it ([`Socket::Image`](crate::Socket::Image)); an engine receives the
//! Texture prim the host read either to, in the prim's or node's `textures`,
//! by the value's or input's name.

use kinogaki_geometry::{Error, Rate, Units};
use kinogaki_value::{Parameter, Parameters};
use std::collections::BTreeMap;

use crate::geometry::image::Image;
use crate::id::Id;
use crate::prim::Camera;

/// The Texture prims the host read images to, by the value's or input's
/// name: what an engine reads for each image (one missing: no image, or one
/// not read yet).
pub(crate) type Textures = BTreeMap<String, Id<Image>>;

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

/// Every setting's default.
pub fn defaults(settings: &[Parameter]) -> Parameters {
    (settings.iter())
        .map(|d| (d.name.clone(), d.value.clone()))
        .collect()
}

/// `values` made to fit `settings`: each setting's value from `values` when
/// it takes it, else its default. Values no setting names are dropped.
pub fn fit(settings: &[Parameter], values: &Parameters) -> Parameters {
    (settings.iter())
        .map(|d| {
            let v = (values.get(&d.name))
                .filter(|v| d.accepts(v))
                .unwrap_or(&d.value);
            (d.name.clone(), v.clone())
        })
        .collect()
}

/// Check `values` against `settings`: each names one and fits it.
pub fn validate(settings: &[Parameter], values: &Parameters) -> Result<(), Error> {
    for (name, value) in values {
        let d = (settings.iter())
            .find(|d| d.name == *name)
            .ok_or_else(|| Error::invalid(format_args!("unknown setting {name:?}")))?;
        if !d.accepts(value) {
            let name = &d.name;
            return Err(Error::invalid(format_args!(
                "setting {name:?} is out of range"
            )));
        }
    }
    Ok(())
}
