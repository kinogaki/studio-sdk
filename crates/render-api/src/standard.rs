//! The standard prim types: UsdLux's lights and exponential height fog,
//! as schemas ([`area_light`], [`dome_light`], [`distant_light`],
//! [`spot_light`], [`fog`]) every engine that renders them declares as
//! they are here, and as typed values ([`Light`], [`Fog`]) an engine
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

use std::sync::Arc;

use crate::declared::{Declared, PrimType, Role};
use crate::id::TextureId;
use crate::prim::{IDENTITY, Matrix};
use crate::settings::{
    SettingDescriptor, SettingGroup, SettingKind, SettingValue, SettingValues, Trailing, Unit,
    set_value,
};
use crate::vocabulary::Choice;

/// The area light's id.
pub const AREA_LIGHT: &str = "kinogaki.render.area_light";
/// The dome light's id.
pub const DOME_LIGHT: &str = "kinogaki.render.dome_light";
/// The distant light's id.
pub const DISTANT_LIGHT: &str = "kinogaki.render.distant_light";
/// The spot light's id.
pub const SPOT_LIGHT: &str = "kinogaki.render.spot_light";
/// The fog's id.
pub const FOG: &str = "kinogaki.render.fog";

/// The group a light's parameters sit in.
pub const LIGHT_GROUP: &str = "light";

/// The published illuminants an emission may name: their CIE names (the
/// keys in files) and labels, in the order a UI lists them.
pub const ILLUMINANTS: [(&str, &str); 13] = [
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
    /// ([`ILLUMINANTS`]).
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
pub fn emission_params(intensity: f64) -> Vec<SettingDescriptor> {
    let choices = [
        Choice::new("color", "Color"),
        Choice::new("temperature", "Color Temperature"),
    ]
    .into_iter()
    .chain(ILLUMINANTS.iter().map(|(n, l)| Choice::new(n, l)))
    .collect();
    let number = |default, min, max, unit, step, decimals| SettingKind::Float {
        default,
        min,
        max,
        soft: (min, max),
        decimals,
        unit,
        slider: false,
        step,
    };
    vec![
        SettingDescriptor::new(
            "emission",
            "Emission",
            LIGHT_GROUP,
            SettingKind::Choice {
                choices,
                default: Arc::from("color"),
                menu: true,
            },
        )
        .trailing(Trailing::EmissionSwatch),
        SettingDescriptor::new(
            "color",
            "Color",
            LIGHT_GROUP,
            SettingKind::Color { default: [1.0; 3] },
        )
        .shown_when("emission", choice("color")),
        SettingDescriptor::new(
            "colorTemperature",
            "Temperature",
            LIGHT_GROUP,
            number(
                6500.0,
                1000.0,
                40_000.0,
                Unit::Suffix(Arc::from(" K")),
                10.0,
                0,
            ),
        )
        .requires(
            "emission",
            choice("temperature"),
            "Used by the Color Temperature emission",
        ),
        SettingDescriptor::new(
            "intensity",
            "Intensity",
            LIGHT_GROUP,
            number(intensity, 0.0, 1e12, Unit::None, 0.05, 2),
        ),
        SettingDescriptor::new(
            "exposure",
            "Exposure",
            LIGHT_GROUP,
            number(0.0, -100.0, 100.0, Unit::Suffix(Arc::from(" EV")), 0.05, 2),
        ),
    ]
}

/// A choice value.
pub fn choice(name: &str) -> SettingValue {
    SettingValue::Choice(Arc::from(name))
}

/// A length in scene units, `min..=1e12`, dragged 0.01 a pixel.
fn length(default: f64, min: f64) -> SettingKind {
    SettingKind::Float {
        default,
        min,
        max: 1e12,
        soft: (min, 1e12),
        decimals: 3,
        unit: Unit::Length,
        slider: false,
        step: 0.01,
    }
}

/// An angle in degrees, 0 to 180, its drag over `soft`.
fn angle(default: f64, soft: (f64, f64)) -> SettingKind {
    SettingKind::Float {
        default,
        min: 0.0,
        max: 180.0,
        soft,
        decimals: 4,
        unit: Unit::Angle,
        slider: false,
        step: 0.5,
    }
}

/// A slider over `min..=max` with three decimals unless `decimals`.
pub fn slider(default: f64, min: f64, max: f64, soft: (f64, f64), decimals: u8) -> SettingKind {
    SettingKind::Float {
        default,
        min,
        max,
        soft,
        decimals,
        unit: Unit::None,
        slider: true,
        step: 0.01,
    }
}

fn toggle(name: &str, label: &str, group: &str, default: bool) -> SettingDescriptor {
    SettingDescriptor::new(name, label, group, SettingKind::Bool { default })
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
pub fn area_light() -> PrimType {
    let shape = SettingDescriptor::new(
        "shape",
        "Shape",
        LIGHT_GROUP,
        SettingKind::Choice {
            choices: vec![
                Choice::new("rectangle", "Rectangle"),
                Choice::new("disk", "Disk"),
            ],
            default: Arc::from("rectangle"),
            menu: true,
        },
    );
    let rectangle = || choice("rectangle");
    light_type(
        AREA_LIGHT,
        "Area Light",
        vec![
            shape,
            SettingDescriptor::new("width", "Width", LIGHT_GROUP, length(1.0, 0.0))
                .shown_when("shape", rectangle()),
            SettingDescriptor::new("height", "Height", LIGHT_GROUP, length(1.0, 0.0))
                .shown_when("shape", rectangle()),
            SettingDescriptor::new("radius", "Radius", LIGHT_GROUP, length(0.5, 0.0))
                .shown_when("shape", choice("disk")),
        ],
        1.0,
        false,
        Vec::new(),
    )
}

/// UsdLux's DomeLight: light from every direction, optionally a lat-long
/// image whose center looks down the light's -Z, its top +Y (`texture`,
/// set by the host); without an image, below the horizon it may shine
/// `groundColor` instead (`ground`: a hemisphere gradient).
pub fn dome_light() -> PrimType {
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
                SettingKind::Color {
                    default: [0.1, 0.09, 0.07],
                },
            )
            .requires("ground", SettingValue::Bool(true), "Used with Ground on"),
            SettingDescriptor::new("texture", "Texture", LIGHT_GROUP, SettingKind::Texture)
                .hidden(),
        ],
    )
}

/// UsdLux's DistantLight: parallel light down its -Z from infinitely far,
/// the sun's size across (`angle`, 0.53°) and intensity 50,000 (an
/// illuminance of about 3.4) by default.
pub fn distant_light() -> PrimType {
    light_type(
        DISTANT_LIGHT,
        "Distant Light",
        vec![SettingDescriptor::new(
            "angle",
            "Angle",
            LIGHT_GROUP,
            angle(0.53, (0.0, 10.0)),
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
pub fn spot_light() -> PrimType {
    light_type(
        SPOT_LIGHT,
        "Spot Light",
        vec![
            SettingDescriptor::new("radius", "Radius", LIGHT_GROUP, length(0.1, 1e-6)),
            SettingDescriptor::new(
                "coneAngle",
                "Cone Angle",
                LIGHT_GROUP,
                angle(30.0, (0.0, 90.0)),
            ),
            SettingDescriptor::new(
                "coneSoftness",
                "Cone Softness",
                LIGHT_GROUP,
                slider(0.2, 0.0, 1.0, (0.0, 1.0), 2),
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
pub fn fog() -> PrimType {
    const GROUP: &str = "fog";
    let along = |default, min| SettingKind::Float {
        default,
        min,
        max: 1e6,
        soft: (min, 1e6),
        decimals: 3,
        unit: Unit::Length,
        slider: false,
        step: 0.01,
    };
    PrimType {
        id: Arc::from(FOG),
        label: Arc::from("Fog"),
        role: Role::Atmosphere,
        groups: vec![SettingGroup::new(GROUP, "Fog", true)],
        params: vec![
            SettingDescriptor::new(
                "density",
                "Density",
                GROUP,
                slider(0.1, 0.0, 1e6, (0.0, 1.0), 3),
            ),
            SettingDescriptor::new(
                "color",
                "Color",
                GROUP,
                SettingKind::Color {
                    default: [0.3, 0.34, 0.42],
                },
            ),
            SettingDescriptor::new("start", "Start", GROUP, along(0.0, 0.0)),
            SettingDescriptor::new("height", "Height", GROUP, along(0.0, -1e6)),
            SettingDescriptor::new(
                "falloff",
                "Falloff",
                GROUP,
                slider(0.5, 0.0, 1e6, (0.0, 4.0), 3),
            ),
            toggle("inscatter", "In-scatter", GROUP, false),
            SettingDescriptor::new(
                "anisotropy",
                "Anisotropy",
                GROUP,
                slider(0.3, -0.9, 0.9, (-0.9, 0.9), 3),
            )
            .requires(
                "inscatter",
                SettingValue::Bool(true),
                "Used with In-scatter on",
            ),
        ],
    }
}

/// A standard light's shape.
#[derive(Clone, Copy, Debug, PartialEq)]
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
        /// The image (none: its color, evenly).
        texture: Option<TextureId>,
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
    /// Its color, linear ACEScg: the spectrum's in [`Emission::Color`].
    pub color: [f32; 3],
    /// `inputs:intensity`.
    pub intensity: f32,
    /// `inputs:exposure`: the intensity is scaled by 2 to this power.
    pub exposure: f32,
    /// `inputs:normalize`: its power does not change with its size.
    pub normalize: bool,
    /// The blackbody's kelvin in [`Emission::Temperature`].
    pub color_temperature: f32,
    /// Light to world.
    pub transform: Matrix,
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
            transform: IDENTITY,
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
                texture: prim
                    .value("texture")
                    .and_then(SettingValue::as_texture)
                    .flatten(),
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
            transform: prim.transform,
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
        let mut set = |name: &str, v: SettingValue| set_value(&mut values, name, v);
        let float = |v: f32| SettingValue::Float(f64::from(v));
        let color = |c: [f32; 3]| SettingValue::Color(c.map(f64::from));
        match self.shape {
            Shape::Rect { width, height } => {
                set("shape", choice("rectangle"));
                set("width", float(width));
                set("height", float(height));
            }
            Shape::Disk { radius } => {
                set("shape", choice("disk"));
                set("radius", float(radius));
            }
            Shape::Dome { texture, ground } => {
                set("texture", SettingValue::Texture(texture));
                set("ground", SettingValue::Bool(ground.is_some()));
                if let Some(g) = ground {
                    set("groundColor", color(g));
                }
            }
            Shape::Distant { angle } => set("angle", float(angle)),
            Shape::Spot {
                radius,
                cone_angle,
                cone_softness,
            } => {
                set("radius", float(radius));
                set("coneAngle", float(cone_angle));
                set("coneSoftness", float(cone_softness));
            }
        }
        set("emission", choice(self.emission.key()));
        set("color", color(self.color));
        set("intensity", float(self.intensity));
        set("exposure", float(self.exposure));
        set("normalize", SettingValue::Bool(self.normalize));
        set("colorTemperature", float(self.color_temperature));
        Declared {
            type_id: ty.id,
            values,
            transform: self.transform,
            visible: self.visible,
        }
    }
}

/// How a spot's light fades from its axis to the cone's edge (ShapingAPI):
/// 1 inside `cone_angle × (1 - softness)`, 0 past `cone_angle`, a smoothstep
/// between. `cos_axis` is the cosine between the light's -Z and the
/// direction from the light to the lit point.
pub fn cone_falloff(cos_axis: f32, cone_angle: f32, cone_softness: f32) -> f32 {
    let outer = cone_angle.clamp(0.0, 180.0).to_radians().cos();
    let inner = (cone_angle.clamp(0.0, 180.0) * (1.0 - cone_softness.clamp(0.0, 1.0)))
        .to_radians()
        .cos();
    if inner - outer <= 1e-7 {
        return if cos_axis >= outer { 1.0 } else { 0.0 };
    }
    let t = ((cos_axis - outer) / (inner - outer)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Fog filling the scene, decoded ([`fog`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fog {
    /// Extinction a scene unit at `height` (0: no fog).
    pub density: f32,
    /// Its color, linear ACEScg: what it fades toward, or, lit, its
    /// scattering color.
    pub color: [f32; 3],
    /// How far from the camera it begins, scene units.
    pub start: f32,
    /// The height `density` holds at, scene units.
    pub height: f32,
    /// How fast it thins above `height` (e-folds a scene unit up).
    pub falloff: f32,
    /// Whether the lights light it (marched as a medium, with shafts where
    /// surfaces shadow it) rather than a flat haze of its color.
    pub inscatter: bool,
    /// Lit fog's Henyey–Greenstein g, −0.9 to 0.9.
    pub anisotropy: f32,
}

impl Default for Fog {
    fn default() -> Fog {
        Fog::decode(&Declared::new(FOG, fog().defaults())).expect("fog")
    }
}

impl Fog {
    /// `prim` as fog (none when it is of another type).
    pub fn decode(prim: &Declared) -> Option<Fog> {
        (&*prim.type_id == FOG).then(|| Fog {
            density: prim.float("density"),
            color: prim.color("color"),
            start: prim.float("start"),
            height: prim.float("height"),
            falloff: prim.float("falloff"),
            inscatter: prim.bool("inscatter"),
            anisotropy: prim.float("anisotropy"),
        })
    }

    /// It as a prim.
    pub fn declared(&self) -> Declared {
        let float = |v: f32| SettingValue::Float(f64::from(v));
        let values: SettingValues = vec![
            (Arc::from("density"), float(self.density)),
            (
                Arc::from("color"),
                SettingValue::Color(self.color.map(f64::from)),
            ),
            (Arc::from("start"), float(self.start)),
            (Arc::from("height"), float(self.height)),
            (Arc::from("falloff"), float(self.falloff)),
            (Arc::from("inscatter"), SettingValue::Bool(self.inscatter)),
            (Arc::from("anisotropy"), float(self.anisotropy)),
        ];
        Declared::new(FOG, values)
    }
}
