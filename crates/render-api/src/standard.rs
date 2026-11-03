//! The standard prim types: UsdLux's lights and exponential height fog,
//! as schemas (`area_light`, `dome_light`, `distant_light`,
//! `spot_light`, `fog`) every engine that renders them declares as
//! they are here, and as typed values ([`Light`], `Fog`) an engine
//! decodes a [`Declared`] prim into and a host or test encodes one from.
//!
//! Lights are shaped as UsdLux shapes them: an area light is a RectLight
//! or DiskLight in its XY plane shining down its -Z; a dome is the
//! DomeLight; a distant light the DistantLight, parallel light down its -Z
//! (the sun); a spot a SphereLight with ShapingAPI's cone about its -Z.
//! Brightness is `intensity × 2^exposure` times the emission's color: the
//! radiance of the light's surface, divided by its area when it
//! normalizes. A Distant light's radiance fills a disk of `angle` degrees
//! across, so its illuminance at a surface facing it is the radiance ×
//! π sin²(angle / 2) (normalized: the illuminance is the intensity); at
//! angle 0 it is a delta direction whose illuminance is the intensity.
//!
//! Parameter names are UsdLux's inputs in camelCase (`coneAngle`,
//! `colorTemperature`).

use kinogaki_geometry::{Sampled, Xform};
use std::sync::Arc;

use crate::declared::{Declared, PrimType, Role};
use crate::settings::{SettingDescriptor, SettingGroup, Trailing};
use kinogaki_value::{Choice, Condition, Number, Unit, Value, ValueKind};

/// The area light's id.
pub const AREA_LIGHT: &str = "kinogaki.render.area_light";
/// The dome light's id.
pub const DOME_LIGHT: &str = "kinogaki.render.dome_light";
/// The distant light's id.
pub(crate) const DISTANT_LIGHT: &str = "kinogaki.render.distant_light";
/// The spot light's id.
pub(crate) const SPOT_LIGHT: &str = "kinogaki.render.spot_light";
/// The fog's id.
pub(crate) const FOG: &str = "kinogaki.render.fog";

/// The group a light's parameters sit in.
pub(crate) const LIGHT_GROUP: &str = "light";

/// The published illuminants an emission may name: their CIE names (the
/// keys in files) and labels, in the order a UI lists them.
pub(crate) const ILLUMINANTS: [(&str, &str); 13] = [
    ("D50", "Daylight D50"),
    ("D55", "Daylight D55"),
    ("D65", "Daylight D65"),
    ("D75", "Daylight D75"),
    ("A", "Incandescent A"),
    ("F2", "Fluorescent F2"),
    ("F7", "Fluorescent F7"),
    ("F11", "Fluorescent F11"),
    ("LED-B1", "LED B1"),
    ("LED-B2", "LED B2"),
    ("LED-B3", "LED B3"),
    ("LED-B4", "LED B4"),
    ("LED-B5", "LED B5"),
];

/// Where an emitter's spectrum comes from. Each is normalized to
/// luminance 1 before its intensity applies, so changing it changes the
/// color and not the brightness.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Default)]
pub enum Emission {
    /// The emitter's color, upsampled to a smooth spectrum.
    #[default]
    Color,
    /// A blackbody at the emitter's color temperature.
    Temperature,
    /// A published illuminant's spectrum, by its CIE name
    /// (`ILLUMINANTS`).
    Illuminant(Arc<str>),
}

impl Emission {
    /// Its key in files (`color`, `temperature`, the CIE name).
    pub fn key(&self) -> &str {
        match self {
            Emission::Color => "color",
            Emission::Temperature => "temperature",
            Emission::Illuminant(name) => name,
        }
    }

    /// The emission of key `key`.
    pub fn of_key(key: &str) -> Option<Emission> {
        match key {
            "color" => Some(Emission::Color),
            "temperature" => Some(Emission::Temperature),
            _ => ILLUMINANTS
                .iter()
                .find(|(name, _)| *name == key)
                .map(|(name, _)| Emission::Illuminant(Arc::from(*name))),
        }
    }
}

/// The emission settings every standard light has (and Pixel's rim):
/// Emission, Color, Temperature, Intensity and Exposure, the light's
/// intensity `intensity` by default.
pub(crate) fn emission_params(intensity: f64) -> Vec<SettingDescriptor> {
    let choices = [
        Choice::new("color", "Color"),
        Choice::new("temperature", "Color Temperature"),
    ]
    .into_iter()
    .chain(ILLUMINANTS.iter().map(|(n, l)| Choice::new(n, l)))
    .collect();
    let number = |min, max, unit, speed, decimals| {
        ValueKind::Number(
            Number::new(unit)
                .range(min, max)
                .speed(speed)
                .decimals(decimals),
        )
    };
    let emission = |key| Condition::when("emission", choice(key));
    vec![
        SettingDescriptor::new(
            "emission",
            "Emission",
            LIGHT_GROUP,
            ValueKind::choice(choices),
            choice("color"),
        )
        .trailing(Trailing::EmissionSwatch),
        SettingDescriptor::new("color", "Color", LIGHT_GROUP, ValueKind::Color, [1.0; 3])
            .shown_when(emission("color")),
        SettingDescriptor::new(
            "colorTemperature",
            "Temperature",
            LIGHT_GROUP,
            number(1000.0, 40_000.0, Unit::Suffix(Arc::from(" K")), 10.0, 0),
            6500.0,
        )
        .requires(
            emission("temperature"),
            "Used by the Color Temperature emission",
        ),
        SettingDescriptor::new(
            "intensity",
            "Intensity",
            LIGHT_GROUP,
            number(0.0, 1e12, Unit::Plain, 0.05, 2),
            intensity,
        ),
        SettingDescriptor::new(
            "exposure",
            "Exposure",
            LIGHT_GROUP,
            number(-100.0, 100.0, Unit::Suffix(Arc::from(" EV")), 0.05, 2),
            0.0,
        ),
    ]
}

/// A choice's value.
pub fn choice(key: &str) -> Value {
    Value::Choice(Arc::from(key))
}

/// A length in scene units, `min..=1e12`, dragged 0.01 a pixel.
fn length(min: f64) -> ValueKind {
    ValueKind::Number(Number::length().range(min, 1e12).decimals(3).speed(0.01))
}

/// An angle in degrees, 0 to 180, its drag over `soft`.
fn angle(soft: (f64, f64)) -> ValueKind {
    let number = Number::angle().range(0.0, 180.0).soft(soft.0, soft.1);
    ValueKind::Number(number.decimals(4).speed(0.5))
}

/// A slider over `min..=max`, its span `soft`, with `decimals`.
pub fn slider(min: f64, max: f64, soft: (f64, f64), decimals: u8) -> ValueKind {
    let number = Number::plain().range(min, max).soft(soft.0, soft.1);
    ValueKind::Number(number.slider().decimals(decimals).speed(0.01))
}

fn toggle(name: &str, label: &str, group: &str, default: bool) -> SettingDescriptor {
    SettingDescriptor::new(name, label, group, ValueKind::Toggle, default)
}

/// A standard light type: its `shape` parameters, then the emission's,
/// then Normalize (on when `normalize`), then `after`.
fn light_type(
    id: &str,
    label: &str,
    shape: Vec<SettingDescriptor>,
    intensity: f64,
    normalize: bool,
    after: Vec<SettingDescriptor>,
) -> PrimType {
    let mut params = shape;
    params.extend(emission_params(intensity));
    params.push(toggle("normalize", "Normalize", LIGHT_GROUP, normalize));
    params.extend(after);
    PrimType {
        id: Arc::from(id),
        label: Arc::from(label),
        role: Role::Light,
        groups: vec![SettingGroup::new(LIGHT_GROUP, "Light", true)],
        params,
    }
}

/// UsdLux's RectLight and DiskLight: a rectangle (`width` × `height`) or
/// a disk (`radius`) in its XY plane, shining down its -Z; a 1 × 1
/// rectangle of intensity 1 by default.
pub(crate) fn area_light() -> PrimType {
    let shapes = vec![
        Choice::new("rectangle", "Rectangle"),
        Choice::new("disk", "Disk"),
    ];
    let shape = SettingDescriptor::new(
        "shape",
        "Shape",
        LIGHT_GROUP,
        ValueKind::choice(shapes),
        choice("rectangle"),
    );
    let shaped = |key| Condition::when("shape", choice(key));
    let size = |name, label, default| {
        SettingDescriptor::new(name, label, LIGHT_GROUP, length(0.0), default)
    };
    light_type(
        AREA_LIGHT,
        "Area Light",
        vec![
            shape,
            size("width", "Width", 1.0).shown_when(shaped("rectangle")),
            size("height", "Height", 1.0).shown_when(shaped("rectangle")),
            size("radius", "Radius", 0.5).shown_when(shaped("disk")),
        ],
        1.0,
        false,
        Vec::new(),
    )
}

/// UsdLux's DomeLight: light from every direction, optionally a lat-long
/// image whose center looks down the light's -Z, its top +Y (`texture`:
/// its file, which the host sends decoded); without an image, below the
/// horizon it may shine `groundColor` instead (`ground`: a hemisphere
/// gradient, the image's row hidden while it is on).
pub(crate) fn dome_light() -> PrimType {
    let ground = |on: bool| Condition::when("ground", on);
    light_type(
        DOME_LIGHT,
        "Dome Light",
        Vec::new(),
        1.0,
        false,
        vec![
            toggle("ground", "Ground", LIGHT_GROUP, false),
            SettingDescriptor::new(
                "groundColor",
                "Ground Color",
                LIGHT_GROUP,
                ValueKind::Color,
                [0.1, 0.09, 0.07],
            )
            .requires(ground(true), "Used with Ground on"),
            SettingDescriptor::new("texture", "Image", LIGHT_GROUP, ValueKind::File, "")
                .shown_when(ground(false)),
        ],
    )
}

/// UsdLux's DistantLight: parallel light down its -Z from infinitely far,
/// the sun's size across (`angle`, 0.53°) and intensity 50,000 (an
/// illuminance of about 3.4) by default.
pub(crate) fn distant_light() -> PrimType {
    light_type(
        DISTANT_LIGHT,
        "Distant Light",
        vec![SettingDescriptor::new(
            "angle",
            "Angle",
            LIGHT_GROUP,
            angle((0.0, 10.0)),
            0.53,
        )],
        50_000.0,
        false,
        Vec::new(),
    )
}

/// UsdLux's SphereLight with ShapingAPI's cone: a sphere at its origin
/// (`radius`) whose light leaves within `coneAngle` of its -Z, fading over
/// `coneSoftness` of the cone; a 0.1 sphere in a 30° cone, normalized, of
/// intensity 100 by default.
pub(crate) fn spot_light() -> PrimType {
    light_type(
        SPOT_LIGHT,
        "Spot Light",
        vec![
            SettingDescriptor::new("radius", "Radius", LIGHT_GROUP, length(1e-6), 0.1),
            SettingDescriptor::new(
                "coneAngle",
                "Cone Angle",
                LIGHT_GROUP,
                angle((0.0, 90.0)),
                30.0,
            ),
            SettingDescriptor::new(
                "coneSoftness",
                "Cone Softness",
                LIGHT_GROUP,
                slider(0.0, 1.0, (0.0, 1.0), 2),
                0.2,
            ),
        ],
        100.0,
        true,
        Vec::new(),
    )
}

/// UsdLux's lights, in the order a UI lists them.
pub fn lights() -> Vec<PrimType> {
    vec![area_light(), dome_light(), distant_light(), spot_light()]
}

/// Exponential height fog: its density `density × e^(−falloff × (h −
/// height))` at height `h` along [`crate::RenderSettings::up`], from
/// `start` along each view ray on (falloff 0: even distance fog). A scene
/// has at most one.
pub(crate) fn fog() -> PrimType {
    const GROUP: &str = "fog";
    let along = |min| ValueKind::Number(Number::length().range(min, 1e6).decimals(3).speed(0.01));
    let fog =
        |name, label, kind, default: f64| SettingDescriptor::new(name, label, GROUP, kind, default);
    PrimType {
        id: Arc::from(FOG),
        label: Arc::from("Fog"),
        role: Role::Atmosphere,
        groups: vec![SettingGroup::new(GROUP, "Fog", true)],
        params: vec![
            fog("density", "Density", slider(0.0, 1e6, (0.0, 1.0), 3), 0.1),
            SettingDescriptor::new("color", "Color", GROUP, ValueKind::Color, [0.3, 0.34, 0.42]),
            fog("start", "Start", along(0.0), 0.0),
            fog("height", "Height", along(-1e6), 0.0),
            fog("falloff", "Falloff", slider(0.0, 1e6, (0.0, 4.0), 3), 0.5),
            toggle("inscatter", "In-scatter", GROUP, false),
            fog(
                "anisotropy",
                "Anisotropy",
                slider(-0.9, 0.9, (-0.9, 0.9), 3),
                0.3,
            )
            .requires(
                Condition::when("inscatter", true),
                "Used with In-scatter on",
            ),
        ],
    }
}

/// A standard light's shape.
#[derive(Clone, Debug, PartialEq)]
pub enum Shape {
    /// A RectLight: `width` × `height` in its XY plane, emitting down -Z.
    Rect {
        /// Along X.
        width: f32,
        /// Along Y.
        height: f32,
    },
    /// A DiskLight: a disk in its XY plane, emitting down -Z.
    Disk {
        /// Its radius.
        radius: f32,
    },
    /// A DomeLight.
    Dome {
        /// The image's file, as typed (none: its color, evenly); an engine
        /// reads the Texture prim the host decoded it to from its prim's
        /// [`textures`](Declared::textures).
        image: Option<Arc<str>>,
        /// An imageless dome's hemisphere gradient: below the horizon it
        /// shines this color, linear ACEScg, times its intensity and
        /// exposure; above it, its emission. None: one color all round.
        ground: Option<[f32; 3]>,
    },
    /// A DistantLight.
    Distant {
        /// Its size across, in degrees (0: a delta direction).
        angle: f32,
    },
    /// A SphereLight with ShapingAPI's cone.
    Spot {
        /// Its radius, above 0.
        radius: f32,
        /// The cone's half-angle, degrees (0 to 180).
        cone_angle: f32,
        /// The share of the cone (0 to 1) its light fades over to its
        /// edge; 0 a hard edge.
        cone_softness: f32,
    },
}

/// A standard light, decoded.
#[derive(Clone, Debug, PartialEq)]
pub struct Light {
    /// Its shape.
    pub shape: Shape,
    /// Where the spectrum comes from.
    pub emission: Emission,
    /// Its color, linear (ACEScg in a prim an engine receives,
    /// [`crate::color`]): the spectrum's in [`Emission::Color`].
    pub color: [f32; 3],
    /// `inputs:intensity`.
    pub intensity: f32,
    /// `inputs:exposure`: the intensity is scaled by 2 to this power.
    pub exposure: f32,
    /// `inputs:normalize`: its power does not change with its size.
    pub normalize: bool,
    /// The blackbody's kelvin in [`Emission::Temperature`].
    pub color_temperature: f32,
    /// Light to world across the shutter.
    pub xform: Sampled<Xform>,
    /// Whether the camera sees the light itself.
    pub visible: bool,
}

impl Default for Light {
    /// A 1 × 1 rectangle of intensity 1 (UsdLux's fallbacks).
    fn default() -> Light {
        Light {
            shape: Shape::Rect {
                width: 1.0,
                height: 1.0,
            },
            emission: Emission::Color,
            color: [1.0; 3],
            intensity: 1.0,
            exposure: 0.0,
            normalize: false,
            color_temperature: 6500.0,
            xform: Sampled::still(Xform::IDENTITY),
            visible: true,
        }
    }
}

impl Light {
    /// `prim` as a standard light (none when it is of another type).
    pub fn decode(prim: &Declared) -> Option<Light> {
        let shape = match &*prim.type_id {
            AREA_LIGHT if prim.choice("shape") == "disk" => Shape::Disk {
                radius: prim.float("radius"),
            },
            AREA_LIGHT => Shape::Rect {
                width: prim.float("width"),
                height: prim.float("height"),
            },
            DOME_LIGHT => Shape::Dome {
                image: (prim.value("texture").and_then(Value::text))
                    .filter(|path| !path.trim().is_empty())
                    .map(Arc::from),
                ground: prim.bool("ground").then(|| prim.color("groundColor")),
            },
            DISTANT_LIGHT => Shape::Distant {
                angle: prim.float("angle"),
            },
            SPOT_LIGHT => Shape::Spot {
                radius: prim.float("radius"),
                cone_angle: prim.float("coneAngle"),
                cone_softness: prim.float("coneSoftness"),
            },
            _ => return None,
        };
        Some(Light {
            shape,
            emission: Emission::of_key(prim.choice("emission")).unwrap_or_default(),
            color: prim.color("color"),
            intensity: prim.float("intensity"),
            exposure: prim.float("exposure"),
            normalize: prim.bool("normalize"),
            color_temperature: prim.float("colorTemperature"),
            xform: prim.xform.clone(),
            visible: prim.visible,
        })
    }

    /// Its type: the standard type its shape is.
    pub fn prim_type(&self) -> PrimType {
        match self.shape {
            Shape::Rect { .. } | Shape::Disk { .. } => area_light(),
            Shape::Dome { .. } => dome_light(),
            Shape::Distant { .. } => distant_light(),
            Shape::Spot { .. } => spot_light(),
        }
    }

    /// It as a prim: its type's defaults, with its own values.
    pub fn declared(&self) -> Declared {
        let ty = self.prim_type();
        let mut values = ty.defaults();
        let mut set = |name: &str, v: Value| _ = values.insert(name.to_owned(), v);
        let float = |v: f32| Value::Number(f64::from(v));
        let color = |c: [f32; 3]| Value::Color(c.map(f64::from));
        match &self.shape {
            Shape::Rect { width, height } => {
                set("shape", choice("rectangle"));
                set("width", float(*width));
                set("height", float(*height));
            }
            Shape::Disk { radius } => {
                set("shape", choice("disk"));
                set("radius", float(*radius));
            }
            Shape::Dome { image, ground } => {
                set("texture", Value::from(image.as_deref().unwrap_or("")));
                set("ground", Value::Bool(ground.is_some()));
                if let Some(g) = ground {
                    set("groundColor", color(*g));
                }
            }
            Shape::Distant { angle } => set("angle", float(*angle)),
            Shape::Spot {
                radius,
                cone_angle,
                cone_softness,
            } => {
                set("radius", float(*radius));
                set("coneAngle", float(*cone_angle));
                set("coneSoftness", float(*cone_softness));
            }
        }
        set("emission", choice(self.emission.key()));
        set("color", color(self.color));
        set("intensity", float(self.intensity));
        set("exposure", float(self.exposure));
        set("normalize", Value::Bool(self.normalize));
        set("colorTemperature", float(self.color_temperature));
        Declared {
            type_id: ty.id,
            values,
            textures: Default::default(),
            xform: self.xform.clone(),
            visible: self.visible,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_light_keeps_its_motion_through_its_prim() {
        let moved = |x: f64| Xform::translation(kinogaki_geometry::Vec3::new(x, 0.0, 0.0));
        let motion = Sampled::new([-0.25, 0.25], [moved(0.0), moved(1.0)]).unwrap();
        let light = Light {
            xform: motion.clone(),
            ..Light::default()
        };
        let prim = light.declared();
        assert_eq!(prim.xform, motion);
        assert_eq!(Light::decode(&prim).unwrap(), light);
    }
}
