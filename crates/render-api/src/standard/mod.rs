//! The standard prim types: UsdLux's lights, one type a UsdLux kind, and
//! exponential height fog, as schemas every engine that renders them
//! declares as they are here ([`lights`], [`fog`]), and as typed values
//! ([`Light`], [`Fog`]) an engine decodes a [`Declared`](crate::Declared)
//! prim into and a host or test encodes one from.
//!
//! Lights are shaped as UsdLux shapes them (OpenUSD's
//! `pxr/usd/usdLux/schema.usda`): a RectLight or DiskLight in its XY plane
//! shining down its -Z; a SphereLight at its origin; a CylinderLight along
//! its X axis, shining from its side and not its caps; the DistantLight,
//! parallel light down its -Z (the sun); the DomeLight (DomeLight_1's pole
//! axis among its values). Brightness is `intensity × 2^exposure` times the
//! emission's color: the radiance of the light's surface, divided by its
//! area when it normalizes. A Distant light's radiance fills a disk of
//! `angle` degrees across, so its illuminance at a surface facing it is the
//! radiance × π sin²(angle / 2) (normalized: the illuminance is the
//! intensity); at angle 0 it is a delta direction whose illuminance is the
//! intensity.
//!
//! Every light but the dome and the distant light takes ShapingAPI's
//! shaping ([`Shaping`]: a cone, focus, an IES profile, each read against
//! the light's -Z per emission direction), every light ShadowAPI's
//! ([`Shadow`]), LightAPI's diffuse and specular multipliers, and whether
//! the camera sees it (off by default). Parameter names are UsdLux's inputs
//! in camelCase (`coneAngle`, `colorTemperature`).
mod emission;
mod fog;
pub mod ies;
mod light;
mod schema;

pub use emission::{Emission, ILLUMINANTS, blackbody, illuminant};
pub use fog::{Fog, fog};
pub use light::{DomeFormat, Light, LightShape, Pole, Preset, Shadow, Shaping, presets};
#[cfg(test)]
pub(crate) use schema::{disk_light, dome_light, sphere_light};
pub use schema::{emission_params, lights};

use kinogaki_value::{PrimTypeId, Value};
use std::sync::Arc;

/// The rect light's id.
pub const RECT_LIGHT: PrimTypeId = PrimTypeId::declared("kinogaki.core.rect_light");
/// The disk light's id.
pub const DISK_LIGHT: PrimTypeId = PrimTypeId::declared("kinogaki.core.disk_light");
/// The sphere light's id.
pub const SPHERE_LIGHT: PrimTypeId = PrimTypeId::declared("kinogaki.core.sphere_light");
/// The cylinder light's id.
pub const CYLINDER_LIGHT: PrimTypeId = PrimTypeId::declared("kinogaki.core.cylinder_light");
/// The distant light's id.
pub const DISTANT_LIGHT: PrimTypeId = PrimTypeId::declared("kinogaki.core.distant_light");
/// The dome light's id.
pub const DOME_LIGHT: PrimTypeId = PrimTypeId::declared("kinogaki.core.dome_light");
/// The fog's id.
pub const FOG: PrimTypeId = PrimTypeId::declared("kinogaki.core.fog");

/// The parameter saying where an emitter's spectrum comes from.
pub const EMISSION: &str = "emission";

/// The group a light's parameters sit in.
pub const LIGHT_GROUP: &str = "light";
/// The group of ShapingAPI's parameters, opened by its toggle.
const SHAPING_GROUP: &str = "shaping";
/// The group of ShadowAPI's parameters.
const SHADOW_GROUP: &str = "shadow";

/// A choice's value.
pub fn choice(key: &str) -> Value {
    Value::Choice(Arc::from(key))
}
