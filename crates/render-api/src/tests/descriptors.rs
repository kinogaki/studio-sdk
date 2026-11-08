//! Settings and the standard prims.
use super::*;
use crate::geometry::EmbeddedFile;
use crate::geometry::image::{ImageColor, ImageFile, ImageSource};
use kinogaki_value::ramp::{Interpolation, Key, Ramp};
use kinogaki_value::{Condition, ImageRef, Parameters, Schema, Value, ValueKind};

/// Values by name, from pairs.
fn values<const N: usize>(pairs: [(&str, Value); N]) -> Parameters {
    pairs.into_iter().map(|(n, v)| (n.to_owned(), v)).collect()
}

#[test]
fn settings_fit_their_descriptors() {
    let d = Parameter::new("samples", "Samples", ValueKind::integer(1, 64), 8_i64).group("main");
    let ds = Schema::default()
        .section(Section::new("main", "Main", true))
        .parameter(d.clone());
    assert_eq!(ds.values(), values([("samples", Value::Integer(8))]));
    ds.validate(&values([("samples", Value::Integer(64))]))
        .unwrap();
    for bad in [
        values([("samples", Value::Integer(65))]),
        values([("samples", Value::Number(8.0))]),
        values([("seed", Value::Integer(1))]),
    ] {
        assert!(ds.validate(&bad).is_err());
    }
    // Animatable by its kind's default, unless the engine says otherwise;
    // an image never.
    assert!(d.animatable && !d.clone().animatable(false).animatable);
    assert!(
        !Parameter::new("t", "T", ValueKind::File, "")
            .group("main")
            .animatable
    );
}

#[test]
fn colors_and_color_ramps_fit_their_descriptors() {
    let key = |p, rgb, i| Key::color(p, rgb, i);
    let stops = vec![
        key(0.0, [0.0, 0.0, 1.0], Interpolation::Linear),
        key(0.5, [1.0, 1.0, 1.0], Interpolation::Constant),
        key(1.0, [1.0, 0.0, 0.0], Interpolation::Linear),
    ];
    let ramp = |keys: Vec<Key>| Value::Ramp(Ramp { color: true, keys });
    let kind = ValueKind::Ramp {
        color: true,
        most: Some(4),
    };
    let ds = Schema::default()
        .section(Section::new("main", "Main", true))
        .parameter(Parameter::new("ramp", "Ramp", kind, ramp(stops.clone())).group("main"))
        .parameter(Parameter::new("sky", "Sky", ValueKind::Color, [0.1, 0.2, 0.3]).group("main"));
    assert_eq!(ds.values()["sky"], Value::Color([0.1, 0.2, 0.3]));
    ds.validate(&ds.values()).unwrap();
    let infinite = vec![key(0.0, [f64::INFINITY, 0.0, 0.0], Interpolation::Linear)];
    for bad in [
        ("ramp", ramp(vec![stops[2], stops[0]])),
        ("ramp", ramp(vec![stops[0]; 5])),
        ("ramp", ramp(Vec::new())),
        ("ramp", ramp(infinite)),
        ("ramp", Value::Ramp(Ramp::default_of(false))),
        ("sky", Value::Color([f64::NAN, 0.0, 0.0])),
    ] {
        assert!(ds.validate(&values([bad])).is_err());
    }
}

/// A dome of `image`, laid out as its file says, its top the scene's up.
fn dome_of(image: Option<ImageRef>) -> standard::LightShape {
    standard::LightShape::Dome {
        image,
        format: standard::DomeFormat::Automatic,
        pole: standard::Pole::Scene,
        ground: None,
    }
}

#[test]
fn standard_lights_and_fog_round_trip_through_their_prims() {
    let shaped = standard::Shaping {
        cone_angle: 25.0,
        cone_softness: 0.4,
        focus: 2.0,
        focus_tint: [0.1, 0.2, 0.3],
        ies: Some(ImageRef::File(ImageSource::path("lamp.ies"))),
        ies_angle_scale: -0.5,
        ies_normalize: true,
    };
    let lights = [
        standard::Light::default(),
        standard::Light {
            shape: standard::LightShape::Rect {
                width: 2.0,
                height: 0.5,
                texture: Some(ImageRef::File(ImageSource::path("panel.png"))),
            },
            diffuse: 0.5,
            specular: 2.0,
            camera: true,
            ..standard::Light::default()
        },
        standard::Light {
            shape: standard::LightShape::Disk { radius: 0.25 },
            emission: standard::Emission::Temperature,
            color_temperature: 3200.0,
            shaping: Some(shaped.clone()),
            ..standard::Light::default()
        },
        standard::Light {
            shape: standard::LightShape::Sphere {
                radius: 0.1,
                point: true,
            },
            normalize: true,
            shadow: standard::Shadow {
                enable: true,
                color: [1.0, 0.0, 0.0],
                distance: 2.0,
                falloff: 0.5,
                falloff_gamma: 2.0,
            },
            ..standard::Light::default()
        },
        standard::Light {
            shape: standard::LightShape::Cylinder {
                length: 0.8,
                radius: 0.015,
                line: false,
            },
            specular: 0.5,
            shaping: Some(standard::Shaping::default()),
            ..standard::Light::default()
        },
        standard::Light {
            shape: standard::LightShape::Dome {
                image: None,
                format: standard::DomeFormat::Automatic,
                pole: standard::Pole::Y,
                ground: Some([0.2, 0.1, 0.05]),
            },
            emission: standard::Emission::of_key("D65").unwrap(),
            ..standard::Light::default()
        },
        // A dome's image: a file, a file held embedded, or an Image node.
        standard::Light {
            shape: standard::LightShape::Dome {
                image: Some(ImageRef::File(ImageSource::path("sky/noon.exr"))),
                format: standard::DomeFormat::Angular,
                pole: standard::Pole::Z,
                ground: None,
            },
            ..standard::Light::default()
        },
        standard::Light {
            shape: dome_of(Some(ImageRef::File(ImageSource::new(
                ImageFile::Embedded(EmbeddedFile::new("noon.exr", vec![1, 2, 3])),
                ImageColor::Data,
            )))),
            ..standard::Light::default()
        },
        standard::Light {
            shape: dome_of(Some(ImageRef::Node("/Library/Sky".into()))),
            shadow: standard::Shadow {
                enable: false,
                ..standard::Shadow::default()
            },
            ..standard::Light::default()
        },
        standard::Light {
            shape: standard::LightShape::Distant { angle: 0.53 },
            intensity: 50_000.0,
            ..standard::Light::default()
        },
    ]
    .into_iter()
    .chain(standard::presets().into_iter().map(|p| p.light));
    for light in lights {
        let prim = light.declared();
        let ty = light.prim_type();
        assert_eq!(prim.type_id, ty.id);
        // Every value fits the type's schema, and it decodes as sent.
        assert_eq!(ty.schema().validate(&prim.values), Ok(()));
        assert_eq!(standard::Light::decode(&prim), Some(light));
    }
    // Each type's defaults fit it and name each parameter once; each is
    // unseen by the camera, and shaping is for shapes that leave a surface.
    for ty in standard::lights().into_iter().chain([standard::fog()]) {
        let defaults = ty.schema().values();
        assert_eq!(ty.schema().check(), Ok(()), "{}", ty.id);
        assert_eq!(ty.schema().validate(&defaults), Ok(()), "{}", ty.id);
        assert_eq!(defaults.len(), ty.schema().rows().len(), "{}", ty.id);
        let prim = Declared::new(ty.id, defaults.clone());
        if let Some(light) = standard::Light::decode(&prim) {
            assert!(!light.camera, "{}", ty.id);
            assert_eq!(
                defaults.contains_key("coneAngle"),
                light.shapes(),
                "{}",
                ty.id
            );
            assert_eq!(light.shadow, standard::Shadow::default());
        }
    }
    // Emission keys are the files' keys: color, temperature, the CIE names.
    assert_eq!(
        standard::Emission::of_key("color"),
        Some(standard::Emission::Color)
    );
    assert_eq!(
        standard::Emission::of_key("LED-B3").unwrap().key(),
        "LED-B3"
    );
    assert_eq!(standard::Emission::of_key("D66"), None);
}

#[test]
fn rows_apply_by_the_values_beside_them() {
    let sphere = standard::sphere_light();
    let mut values = sphere.schema().values();
    let applies = |values: &Parameters, name: &str| param(&sphere, name).unwrap().applies(values);
    // A point has nothing the camera could see.
    assert!(applies(&values, "cameraVisibility"));
    values.insert("treatAsPoint".into(), Value::Bool(true));
    assert!(!applies(&values, "cameraVisibility"));
    // A row that doesn't apply is hidden.
    assert_eq!(
        param(&sphere, "cameraVisibility")
            .unwrap()
            .clone()
            .resolved(&values),
        None
    );
    // Shaping's rows show while it is on; the tint while it focuses, the
    // profile's settings while it has one.
    assert!(!applies(&values, "coneAngle"));
    values.insert("shaping".into(), Value::Bool(true));
    assert!(applies(&values, "coneAngle") && !applies(&values, "focusTint"));
    values.insert("focus".into(), Value::Number(3.0));
    assert!(applies(&values, "focusTint") && !applies(&values, "iesAngleScale"));
    let ies = Value::Image(Some(ImageRef::File(ImageSource::path("lamp.ies"))));
    values.insert("iesFile".into(), ies);
    assert!(applies(&values, "iesAngleScale"));
    // Color applies to the color emission, Temperature to its own.
    assert!(applies(&values, "color") && !applies(&values, "colorTemperature"));
    values.insert("emission".into(), standard::choice("temperature"));
    assert!(!applies(&values, "color") && applies(&values, "colorTemperature"));
    // A dome's image applies while it has no ground; it starts at none, and
    // takes a file or an Image node from the root, never a blank one; its
    // format applies while it has one.
    let dome = standard::dome_light();
    let mut sky = dome.schema().values();
    let image = param(&dome, "texture").unwrap();
    assert!(image.applies(&sky) && !param(&dome, "textureFormat").unwrap().applies(&sky));
    assert_eq!(sky.get("texture"), Some(&Value::Image(None)));
    let take = |r: ImageRef| image.check(&Value::Image(Some(r))).is_ok();
    assert!(take(ImageRef::File(ImageSource::path("sky.exr"))));
    assert!(take(ImageRef::Node("/Library/Sky".into())));
    assert!(!take(ImageRef::File(ImageSource::path(" "))));
    assert!(!take(ImageRef::Node("Library/Sky".into())));
    assert!(!image.accepts(&Value::from("sky.exr")));
    let prim = Declared::new(standard::DOME_LIGHT, sky.clone());
    assert_eq!(
        standard::Light::decode(&prim).map(|l| l.shape),
        Some(dome_of(None))
    );
    assert!(
        !sky.contains_key("normalize"),
        "UsdLux ignores it on a dome"
    );
    sky.insert("ground".into(), Value::Bool(true));
    assert!(!image.applies(&sky));
    // Shadows' rows show while it casts them; the gamma while it fades.
    let shadows = param(&dome, "shadowColor").unwrap();
    assert!(shadows.applies(&sky) && !param(&dome, "shadowFalloffGamma").unwrap().applies(&sky));
    sky.insert("shadowEnable".into(), Value::Bool(false));
    assert!(!shadows.applies(&sky));
    // Fitting keeps what fits by name, defaults the rest, drops strangers.
    let disk = standard::disk_light();
    let from_sphere = disk.schema().fit(&values);
    assert_eq!(
        from_sphere.get("emission"),
        Some(&standard::choice("temperature"))
    );
    assert_eq!(from_sphere.get("shaping"), Some(&Value::Bool(true)));
    assert_eq!(from_sphere.get("treatAsPoint"), None);
    assert_eq!(from_sphere.len(), disk.schema().rows().len());
    let zero = self::values([("radius", Value::Number(0.0))]);
    assert_eq!(
        disk.schema().fit(&zero).get("radius"),
        Some(&Value::Number(0.5)),
        "a disk keeps a size to shine"
    );
    // A group may open with a toggle.
    let group = Section::new("gobo", "Gobo", false).open_when(Condition::when("gobo", true));
    assert!(!group.is_open(&Parameters::new()));
    assert!(group.is_open(&self::values([("gobo", Value::Bool(true))])));
}

/// The parameter of `ty` named `name`.
fn param<'a>(ty: &'a PrimType, name: &str) -> Option<&'a Parameter> {
    ty.schema().parameters().find(|p| &*p.name == name)
}
