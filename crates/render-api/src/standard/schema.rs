//! The standard lights' schemas, one a UsdLux kind: each type's shape rows,
//! then the emission's, then LightAPI's Normalize, Diffuse, Specular and
//! whether the camera sees it, then its own; then the Shaping section
//! (ShapingAPI, opened by its toggle) on the lights it shapes, and the
//! Shadows section (ShadowAPI) on all. Defaults are UsdLux's fallbacks.
use super::{
    CYLINDER_LIGHT, DISK_LIGHT, DISTANT_LIGHT, DOME_LIGHT, ILLUMINANTS, LIGHT_GROUP, RECT_LIGHT,
    SHADOW_GROUP, SHAPING_GROUP, SPHERE_LIGHT, choice,
};
use crate::declared::{PrimType, Role};
use kinogaki_value::{
    Choice, Condition, Number, NumberLook, Parameter, PrimTypeId, Schema, Section, Unit, Value,
    ValueKind,
};
use std::sync::Arc;

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
            .requires(emission("color")),
        Parameter::new(
            "colorTemperature",
            "Temperature",
            number(1000.0, 40_000.0, Unit::Suffix(Arc::from(" K"))),
            6500.0,
        )
        .group(LIGHT_GROUP)
        .look(look(10.0, 0))
        .requires(emission("temperature")),
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

/// Angle setting `name` in degrees, 0 to 180, at `default`, its drag over
/// `soft`.
fn angle(name: &str, label: &str, soft: (f64, f64), default: f64) -> Parameter {
    let kind = ValueKind::Number(Number::angle().range(0.0, 180.0).soft(soft.0, soft.1));
    Parameter::new(name, label, kind, default)
        .group(LIGHT_GROUP)
        .look(NumberLook::default().decimals(4).speed(0.5))
}

/// A size above 0 (a light of no size is no light value), the least one
/// a light holds in single precision.
fn size(name: &str, label: &str, default: f64) -> Parameter {
    let least = f64::from(1e-6_f32);
    Parameter::length(name, label, (least, 1e12), None, 3, default).group(LIGHT_GROUP)
}

/// A file row: an image or a profile a light reads, none by default.
fn file(name: &str, label: &str, group: &str) -> Parameter {
    Parameter::new(name, label, ValueKind::Image, Value::Image(None)).group(group)
}

/// LightAPI's rows after the emission: Normalize (the dome has none: UsdLux
/// ignores it there), Diffuse, Specular, and Visible to Camera (off), shown
/// while `seen` holds (a point or line has nothing to see).
fn light_api(normalize: bool, seen: Option<Condition>) -> Vec<Parameter> {
    let multiplier = |name: &str, label: &str| {
        Parameter::slider(name, label, (0.0, 1e6), (0.0, 2.0), 2, 1.0).group(LIGHT_GROUP)
    };
    let mut camera = Parameter::toggle("cameraVisibility", "Visible to Camera", false)
        .group(LIGHT_GROUP)
        .help("The camera sees the light itself; it lights, and reflects, either way");
    if let Some(seen) = seen {
        camera = camera.requires(seen);
    }
    let normalize = normalize.then(|| {
        Parameter::toggle("normalize", "Normalize", false)
            .group(LIGHT_GROUP)
            .help("Its power does not change with its size")
    });
    (normalize.into_iter())
        .chain([
            multiplier("diffuse", "Diffuse"),
            multiplier("specular", "Specular"),
            camera,
        ])
        .collect()
}

/// ShapingAPI's section, opened by its toggle: Cone Angle and Softness,
/// Focus and Focus Tint, an IES profile with its Angle Scale and Normalize.
fn shaping() -> Schema {
    let on = || Condition::when("shaping", true);
    let profiled = || on().except("iesFile", Value::Image(None));
    let section = Section::new(SHAPING_GROUP, "Shaping", false).open_when(on());
    let slider = |name: &str, label: &str, range, soft, default| {
        Parameter::slider(name, label, range, soft, 2, default).group(SHAPING_GROUP)
    };
    [
        Parameter::toggle("shaping", "Shaping", false)
            .group(SHAPING_GROUP)
            .help("Narrows the light: a cone, focus toward its axis, an IES profile"),
        angle("coneAngle", "Cone Angle", (0.0, 90.0), 90.0)
            .group(SHAPING_GROUP)
            .requires(on()),
        slider("coneSoftness", "Cone Softness", (0.0, 1.0), (0.0, 1.0), 0.0).requires(on()),
        slider("focus", "Focus", (0.0, 1e6), (0.0, 20.0), 0.0).requires(on()),
        Parameter::new("focusTint", "Focus Tint", ValueKind::Color, [0.0; 3])
            .group(SHAPING_GROUP)
            .requires(on().except("focus", Value::Number(0.0))),
        file("iesFile", "IES Profile", SHAPING_GROUP).requires(on()),
        slider(
            "iesAngleScale",
            "IES Angle Scale",
            (-1e6, 1e6),
            (-2.0, 2.0),
            0.0,
        )
        .requires(profiled()),
        Parameter::toggle("iesNormalize", "IES Normalize", false)
            .group(SHAPING_GROUP)
            .requires(profiled()),
    ]
    .into_iter()
    .fold(Schema::default().section(section), Schema::parameter)
}

/// ShadowAPI's section: Shadows, their Color, Distance and Falloff (−1:
/// none) and Falloff Gamma.
fn shadow() -> Schema {
    let on = || Condition::when("shadowEnable", true);
    let section = Section::new(SHADOW_GROUP, "Shadows", false);
    let reach = |name: &str, label: &str| {
        Parameter::length(name, label, (-1.0, 1e12), None, 3, -1.0)
            .group(SHADOW_GROUP)
            .requires(on())
    };
    [
        Parameter::toggle("shadowEnable", "Shadows", true).group(SHADOW_GROUP),
        Parameter::new("shadowColor", "Shadow Color", ValueKind::Color, [0.0; 3])
            .group(SHADOW_GROUP)
            .requires(on()),
        reach("shadowDistance", "Shadow Distance")
            .help("How far an occluder may be and still shadow; −1: any distance"),
        reach("shadowFalloff", "Shadow Falloff")
            .help("Within the distance, how far shadows fade out; −1: none"),
        Parameter::slider(
            "shadowFalloffGamma",
            "Falloff Gamma",
            (0.0, 1e6),
            (0.1, 4.0),
            2,
            1.0,
        )
        .group(SHADOW_GROUP)
        .requires(on().except("shadowFalloff", Value::Number(-1.0))),
    ]
    .into_iter()
    .fold(Schema::default().section(section), Schema::parameter)
}

/// A standard light type: its `shape` rows, the emission's (`intensity` by
/// default), LightAPI's (Normalize when `normalize`, Visible to Camera
/// while `seen` holds), then `after`; Shaping when `shaped`, then Shadows.
struct Kind {
    id: PrimTypeId,
    label: &'static str,
    shape: Vec<Parameter>,
    intensity: f64,
    normalize: bool,
    seen: Option<Condition>,
    after: Vec<Parameter>,
    shaped: bool,
}

impl Kind {
    fn new(id: PrimTypeId, label: &'static str, shape: Vec<Parameter>) -> Self {
        Kind {
            id,
            label,
            shape,
            intensity: 1.0,
            normalize: true,
            seen: None,
            after: Vec::new(),
            shaped: true,
        }
    }

    fn build(self) -> PrimType {
        let params = (self.shape.into_iter())
            .chain(emission_params(self.intensity))
            .chain(light_api(self.normalize, self.seen))
            .chain(self.after);
        let schema = Schema::default().section(Section::new(LIGHT_GROUP, "Light", true));
        let mut schema = params.fold(schema, Schema::parameter);
        if self.shaped {
            schema = schema.merged(shaping());
        }
        PrimType::new(self.id, self.label, Role::Light, schema.merged(shadow()))
    }
}

/// UsdLux's RectLight: `width` × `height` in its XY plane shining down its
/// -Z, a 1 × 1 rectangle by default; its Color Map (`texture`, an image)
/// multiplies its emission, its first texel at +X +Y.
pub(crate) fn rect_light() -> PrimType {
    let shape = vec![
        size("width", "Width", 1.0),
        size("height", "Height", 1.0),
        file("texture", "Color Map", LIGHT_GROUP),
    ];
    Kind::new(RECT_LIGHT, "Rect Light", shape).build()
}

/// UsdLux's DiskLight: a disk of `radius` (0.5) in its XY plane, shining
/// down its -Z.
pub(crate) fn disk_light() -> PrimType {
    Kind::new(
        DISK_LIGHT,
        "Disk Light",
        vec![size("radius", "Radius", 0.5)],
    )
    .build()
}

/// UsdLux's SphereLight: a sphere of `radius` (0.5) at its origin; a point
/// of its power when it is treated as one (`treatAsPoint`).
pub(crate) fn sphere_light() -> PrimType {
    let shape = vec![
        size("radius", "Radius", 0.5),
        Parameter::toggle("treatAsPoint", "Treat As Point", false).group(LIGHT_GROUP),
    ];
    Kind {
        seen: Some(Condition::when("treatAsPoint", false)),
        ..Kind::new(SPHERE_LIGHT, "Sphere Light", shape)
    }
    .build()
}

/// UsdLux's CylinderLight: a tube `length` (1) along its X axis of
/// `radius` (0.5), shining from its side, not its caps; a line of its power
/// when it is treated as one (`treatAsLine`).
pub(crate) fn cylinder_light() -> PrimType {
    let shape = vec![
        size("length", "Length", 1.0),
        size("radius", "Radius", 0.5),
        Parameter::toggle("treatAsLine", "Treat As Line", false).group(LIGHT_GROUP),
    ];
    Kind {
        seen: Some(Condition::when("treatAsLine", false)),
        ..Kind::new(CYLINDER_LIGHT, "Cylinder Light", shape)
    }
    .build()
}

/// UsdLux's DistantLight: parallel light down its -Z from infinitely far,
/// the sun's size across (`angle`, 0.53°) and intensity 50,000 (an
/// illuminance of about 3.4) by default.
pub(crate) fn distant_light() -> PrimType {
    let shape = vec![angle("angle", "Angle", (0.0, 10.0), 0.53)];
    Kind {
        intensity: 50_000.0,
        shaped: false,
        ..Kind::new(DISTANT_LIGHT, "Distant Light", shape)
    }
    .build()
}

/// UsdLux's DomeLight (and DomeLight_1): light from every direction,
/// optionally an image (`texture`: an [`ImageRef`](kinogaki_value::ImageRef),
/// a file or an Image node, which the host sends decoded) laid out as
/// `textureFormat` says, its top along its pole (`poleAxis`: the scene's up
/// axis, DomeLight_1's fallback, or the dome's own +Y or +Z); without an
/// image, below the horizon it may shine `groundColor` instead (`ground`:
/// a hemisphere gradient, the image's rows hidden while it is on).
pub(crate) fn dome_light() -> PrimType {
    let ground = |on: bool| Condition::when("ground", on);
    let imaged = || ground(false).except("texture", Value::Image(None));
    let formats = [
        ("automatic", "Automatic"),
        ("latlong", "Lat-Long"),
        ("mirroredBall", "Mirrored Ball"),
        ("angular", "Angular"),
        ("cubeMapVerticalCross", "Vertical Cross"),
    ];
    let poles = [("scene", "Scene"), ("Y", "Y"), ("Z", "Z")];
    let after = vec![
        Parameter::toggle("ground", "Ground", false).group(LIGHT_GROUP),
        Parameter::new(
            "groundColor",
            "Ground Color",
            ValueKind::Color,
            [0.1, 0.09, 0.07],
        )
        .group(LIGHT_GROUP)
        .requires(ground(true)),
        file("texture", "Image", LIGHT_GROUP).requires(ground(false)),
        Parameter::choice("textureFormat", "Format", &formats, "automatic")
            .group(LIGHT_GROUP)
            .requires(imaged()),
        Parameter::choice("poleAxis", "Pole Axis", &poles, "scene")
            .group(LIGHT_GROUP)
            .help("Where the dome's top points: the scene's up axis, or its own Y or Z"),
    ];
    Kind {
        normalize: false,
        after,
        shaped: false,
        ..Kind::new(DOME_LIGHT, "Dome Light", Vec::new())
    }
    .build()
}

/// UsdLux's lights, in the order a UI lists them.
pub fn lights() -> Vec<PrimType> {
    vec![
        rect_light(),
        disk_light(),
        sphere_light(),
        cylinder_light(),
        distant_light(),
        dome_light(),
    ]
}
