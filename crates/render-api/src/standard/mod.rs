//! The standard prim types: UsdLux's lights and exponential height fog,
//! as schemas (`area_light`, `dome_light`, `distant_light`,
//! `spot_light`, `fog`) every engine that renders them declares as
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

use kinogaki_geometry::{Sampled, Xform};
use std::sync::Arc;

mod emission;

pub use emission::{Emission, ILLUMINANTS, blackbody, illuminant};

use crate::declared::{Declared, PrimType, Role};
use crate::prim::Visibility;
use kinogaki_value::{Choice, Condition, Number, NumberLook, PrimTypeId, Unit, Value, ValueKind};
use kinogaki_value::{Parameter, Section};

/// The area light's id.
pub const AREA_LIGHT: PrimTypeId = PrimTypeId::declared("kinogaki.render.area_light");
/// The dome light's id.
pub const DOME_LIGHT: PrimTypeId = PrimTypeId::declared("kinogaki.render.dome_light");
/// The distant light's id.
pub const DISTANT_LIGHT: PrimTypeId = PrimTypeId::declared("kinogaki.render.distant_light");
/// The spot light's id.
pub const SPOT_LIGHT: PrimTypeId = PrimTypeId::declared("kinogaki.render.spot_light");
/// The fog's id.
pub const FOG: PrimTypeId = PrimTypeId::declared("kinogaki.render.fog");

/// The parameter saying where an emitter's spectrum comes from.
pub const EMISSION: &str = "emission";

/// The group a light's parameters sit in.
pub const LIGHT_GROUP: &str = "light";

/// The emission settings every standard light has, in [`LIGHT_GROUP`]
/// (and an engine's own light takes): Emission, Color, Temperature,
/// Intensity and Exposure, the light's intensity `intensity` by default.
pub fn emission_params(intensity: f64) -> Vec<Parameter> {
    let choices = [
        Choice::new("color", "Color"),
        Choice::new("temperature", "Color Temperature"),
    ]
    .into_iter()
    .chain(ILLUMINANTS.iter().map(|(n, l, _)| Choice::new(n, l)))
    .collect();
    let number = |min, max, unit| ValueKind::Number(Number::new(unit).range(min, max));
    let look = |speed, decimals| NumberLook::default().speed(speed).decimals(decimals);
    let emission = |key| Condition::when("emission", choice(key));
    vec![
        Parameter::new(
            "emission",
            "Emission",
            ValueKind::choice(choices),
            choice("color"),
        )
        .group(LIGHT_GROUP),
        Parameter::new("color", "Color", ValueKind::Color, [1.0; 3])
            .group(LIGHT_GROUP)
            .shown_when(emission("color")),
        Parameter::new(
            "colorTemperature",
            "Temperature",
            number(1000.0, 40_000.0, Unit::Suffix(Arc::from(" K"))),
            6500.0,
        )
        .group(LIGHT_GROUP)
        .look(look(10.0, 0))
        .requires(
            emission("temperature"),
            "Used by the Color Temperature emission",
        ),
        Parameter::new(
            "intensity",
            "Intensity",
            number(0.0, 1e12, Unit::Plain),
            intensity,
        )
        .group(LIGHT_GROUP)
        .look(look(0.05, 2)),
        Parameter::new(
            "exposure",
            "Exposure",
            number(-100.0, 100.0, Unit::Suffix(Arc::from(" EV"))),
            0.0,
        )
        .group(LIGHT_GROUP)
        .look(look(0.05, 2)),
    ]
}

/// A choice's value.
pub fn choice(key: &str) -> Value {
    Value::Choice(Arc::from(key))
}

/// Angle setting `name` in degrees, 0 to 180, at `default`, its drag over
/// `soft`.
fn angle(name: &str, label: &str, soft: (f64, f64), default: f64) -> Parameter {
    let kind = ValueKind::Number(Number::angle().range(0.0, 180.0).soft(soft.0, soft.1));
    Parameter::new(name, label, kind, default)
        .group(LIGHT_GROUP)
        .look(NumberLook::default().decimals(4).speed(0.5))
}

/// A standard light type: its `shape` parameters, then the emission's,
/// then Normalize (on when `normalize`), then `after`.
fn light_type(
    id: PrimTypeId,
    label: &str,
    shape: Vec<Parameter>,
    intensity: f64,
    normalize: bool,
    after: Vec<Parameter>,
) -> PrimType {
    let mut params = shape;
    params.extend(emission_params(intensity));
    params.push(Parameter::toggle("normalize", "Normalize", normalize).group(LIGHT_GROUP));
    params.extend(after);
    PrimType {
        id,
        label: Arc::from(label),
        role: Role::Light,
        groups: vec![Section::new(LIGHT_GROUP, "Light", true)],
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
    let shape = Parameter::new(
        "shape",
        "Shape",
        ValueKind::choice(shapes),
        choice("rectangle"),
    )
    .group(LIGHT_GROUP);
    let shaped = |key| Condition::when("shape", choice(key));
    // Above 0: a light of no size is no light value.
    let size = |name, label, default| {
        Parameter::length(name, label, (1e-6, 1e12), None, 3, default).group(LIGHT_GROUP)
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
            Parameter::toggle("ground", "Ground", false).group(LIGHT_GROUP),
            Parameter::new(
                "groundColor",
                "Ground Color",
                ValueKind::Color,
                [0.1, 0.09, 0.07],
            )
            .group(LIGHT_GROUP)
            .requires(ground(true), "Used with Ground on"),
            Parameter::new("texture", "Image", ValueKind::File, "")
                .group(LIGHT_GROUP)
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
        vec![angle("angle", "Angle", (0.0, 10.0), 0.53)],
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
            Parameter::length("radius", "Radius", (1e-6, 1e12), None, 3, 0.1).group(LIGHT_GROUP),
            angle("coneAngle", "Cone Angle", (0.0, 90.0), 30.0),
            Parameter::slider(
                "coneSoftness",
                "Cone Softness",
                (0.0, 1.0),
                (0.0, 1.0),
                2,
                0.2,
            )
            .group(LIGHT_GROUP),
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
/// height))` at height `h` along the project's up axis, from
/// `start` along each view ray on (falloff 0: even distance fog). A scene
/// has at most one.
pub fn fog() -> PrimType {
    const GROUP: &str = "fog";
    PrimType {
        id: FOG,
        label: Arc::from("Fog"),
        role: Role::Atmosphere,
        groups: vec![Section::new(GROUP, "Fog", true)],
        params: vec![
            Parameter::slider("density", "Density", (0.0, 1e6), (0.0, 1.0), 3, 0.1).group(GROUP),
            Parameter::new("color", "Color", ValueKind::Color, [0.3, 0.34, 0.42]).group(GROUP),
            Parameter::length("start", "Start", (0.0, 1e6), None, 3, 0.0).group(GROUP),
            Parameter::length("height", "Height", (-1e6, 1e6), None, 3, 0.0).group(GROUP),
            Parameter::slider("falloff", "Falloff", (0.0, 1e6), (0.0, 4.0), 3, 0.5).group(GROUP),
            Parameter::toggle("inscatter", "In-scatter", false).group(GROUP),
            Parameter::slider("anisotropy", "Anisotropy", (-0.9, 0.9), (-0.9, 0.9), 3, 0.3)
                .group(GROUP)
                .requires(
                    Condition::when("inscatter", true),
                    "Used with In-scatter on",
                ),
        ],
    }
}

/// A standard fog ([`FOG`]) as a typed value: exponential height fog, its
/// values within [`fog`]'s limits as a host sends them.
#[derive(Clone, Debug, PartialEq)]
pub struct Fog {
    /// Extinction per scene unit at [`height`](Self::height).
    pub density: f32,
    /// Its color, linear (ACEScg in a prim an engine receives).
    pub color: [f32; 3],
    /// How far from the camera it begins.
    pub start: f32,
    /// Where it has its density; above, it thins by
    /// [`falloff`](Self::falloff) per unit.
    pub height: f32,
    /// How fast it thins with height.
    pub falloff: f32,
    /// It scatters the lights' light toward the camera.
    pub inscatter: bool,
    /// Its phase's anisotropy, with in-scatter (−0.9 back to 0.9 forward).
    pub anisotropy: f32,
}

impl Fog {
    /// `prim` as a standard fog (none when it is of another type).
    pub fn decode(prim: &Declared) -> Option<Fog> {
        (prim.type_id == FOG).then(|| Fog {
            density: prim.float("density"),
            color: prim.color("color"),
            start: prim.float("start"),
            height: prim.float("height"),
            falloff: prim.float("falloff"),
            inscatter: prim.bool("inscatter"),
            anisotropy: prim.float("anisotropy"),
        })
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
    /// [`crate::RENDER_SPACE`]): the spectrum's in [`Emission::Color`].
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
    /// Whether the camera sees the light itself (it lights the scene either
    /// way).
    pub camera: bool,
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
            camera: true,
        }
    }
}

impl Light {
    /// `prim` as a standard light (none when it is of another type).
    pub fn decode(prim: &Declared) -> Option<Light> {
        let shape = match prim.type_id {
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
            camera: prim.visibility.camera,
        })
    }

    /// Its radiance as an RGB engine takes it, linear ACEScg: the
    /// emission's color × intensity × 2^exposure (white for an illuminant
    /// [`ILLUMINANTS`] doesn't hold).
    pub fn radiance(&self) -> [f32; 3] {
        let color = self.color.map(f64::from);
        let kelvin = f64::from(self.color_temperature);
        let rgb = self.emission.color(color, kelvin).unwrap_or([1.0; 3]);
        let k = f64::from(self.intensity) * f64::from(self.exposure).exp2();
        rgb.map(|c| (c * k) as f32)
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
            visibility: Visibility {
                visible: true,
                camera: self.camera,
            },
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
