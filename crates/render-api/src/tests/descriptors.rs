//! Vocabularies, networks, settings and the standard prims.
use super::*;

/// A tiny vocabulary: one surface whose `mode` choice decides whether it
/// has a second input.
fn tiny_layout(_: &str, params: &Params) -> NodeLayout {
    let two = vocabulary::param(params, "mode").and_then(ParamValue::choice) == Some("two");
    let mut inputs = vec![SocketSpec::new(
        "color",
        "Color",
        SocketKind::Color,
        [0.5; 3],
    )];
    if two {
        inputs.push(SocketSpec::new(
            "amount",
            "Amount",
            SocketKind::Float,
            [1.0; 3],
        ));
    }
    NodeLayout {
        params: vec![ParamSpec {
            name: Arc::from("mode"),
            label: Arc::from("Mode"),
            kind: ParamKind::Choice {
                choices: vec![Choice::new("one", "One"), Choice::new("two", "Two")],
                menu: false,
            },
            default: ParamValue::Choice(Arc::from("one")),
            tooltip: None,
            shown: true,
        }],
        inputs,
        outputs: Vec::new(),
    }
}

fn tiny() -> NodeType {
    NodeType {
        engine: EngineId::new("tiny"),
        name: Arc::from("tiny_surface"),
        label: Arc::from("Tiny Surface"),
        category: Arc::from("Surface"),
        aliases: Vec::new(),
        surface: Some(Arc::from("Tiny")),
        defaults: vec![(Arc::from("mode"), ParamValue::Choice(Arc::from("one")))],
        layout: Arc::new(tiny_layout),
    }
}

#[test]
fn node_types_fit_their_settings_and_lay_out_by_them() {
    let t = tiny();
    assert_eq!(t.layout(&t.defaults).inputs.len(), 1);
    // A choice it does not offer takes the default; a missing one too.
    let bad = vec![(Arc::from("mode"), ParamValue::Choice(Arc::from("three")))];
    assert_eq!(t.fit(&bad), t.defaults);
    assert_eq!(t.fit(&Vec::new()), t.defaults);
    let two = vec![(Arc::from("mode"), ParamValue::Choice(Arc::from("two")))];
    assert_eq!(t.fit(&two), two);
    let layout = t.layout(&two);
    assert_eq!(layout.input("amount"), Some(1));
    assert!(layout.params[0].accepts(&two[0].1));
}

#[test]
fn settings_fit_their_descriptors() {
    let d = SettingDescriptor::new(
        "samples",
        "Samples",
        "main",
        SettingKind::Int {
            default: 8,
            min: 1,
            max: 64,
        },
    );
    let ds = vec![d.clone()];
    assert_eq!(
        settings::defaults(&ds),
        [(Arc::from("samples"), SettingValue::Int(8))]
    );
    settings::validate(&ds, &vec![(Arc::from("samples"), SettingValue::Int(64))]).unwrap();
    for bad in [
        vec![(Arc::from("samples"), SettingValue::Int(65))],
        vec![(Arc::from("samples"), SettingValue::Float(8.0))],
        vec![(Arc::from("seed"), SettingValue::Int(1))],
    ] {
        assert!(settings::validate(&ds, &bad).is_err());
    }
    // Animatable by its kind's default, unless the engine says otherwise;
    // an image never.
    assert!(d.animatable && !d.clone().animatable(false).animatable);
    assert!(!SettingDescriptor::new("t", "T", "main", SettingKind::Image).animatable);
    let mut r = RenderSettings::default();
    r.set("samples", SettingValue::Int(3));
    r.set("samples", SettingValue::Int(4));
    assert_eq!(r.value("samples"), Some(&SettingValue::Int(4)));
}

#[test]
fn colors_and_color_ramps_fit_their_descriptors_and_ramps_blend() {
    let key = |p, rgb, i| ramp::Key::color(p, rgb, i);
    let stops: Arc<[ramp::Key]> = Arc::from(vec![
        key(0.0, [0.0, 0.0, 1.0], ramp::Interpolation::Linear),
        key(0.5, [1.0, 1.0, 1.0], ramp::Interpolation::Constant),
        key(1.0, [1.0, 0.0, 0.0], ramp::Interpolation::Linear),
    ]);
    let ramp = SettingKind::ColorRamp {
        default: stops.clone(),
        most: 4,
    };
    let row = |name: &str, kind| SettingDescriptor::new(name, name, "main", kind);
    let ds = vec![
        row("ramp", ramp),
        row(
            "sky",
            SettingKind::Color {
                default: [0.1, 0.2, 0.3],
            },
        ),
    ];
    assert_eq!(
        settings::defaults(&ds)[1].1,
        SettingValue::Color([0.1, 0.2, 0.3])
    );
    settings::validate(&ds, &settings::defaults(&ds)).unwrap();
    let unordered = Arc::from(vec![stops[2], stops[0]]);
    let too_many = Arc::from(vec![stops[0]; 5]);
    for bad in [
        (Arc::from("ramp"), SettingValue::ColorRamp(unordered)),
        (Arc::from("ramp"), SettingValue::ColorRamp(too_many)),
        (
            Arc::from("ramp"),
            SettingValue::ColorRamp(Arc::from(vec![])),
        ),
        (Arc::from("sky"), SettingValue::Color([-1.0, 0.0, 0.0])),
        (Arc::from("sky"), SettingValue::Color([f64::NAN, 0.0, 0.0])),
    ] {
        assert!(settings::validate(&ds, &vec![bad]).is_err());
    }
    // A negative color is no color ramp.
    let negative = Arc::from(vec![key(
        0.0,
        [-0.1, 0.0, 0.0],
        ramp::Interpolation::Linear,
    )]);
    assert!(
        settings::validate(
            &ds,
            &vec![(Arc::from("ramp"), SettingValue::ColorRamp(negative))]
        )
        .is_err()
    );
}

#[test]
fn standard_lights_and_fog_round_trip_through_their_prims() {
    let lights = [
        standard::Light::default(),
        standard::Light {
            shape: standard::Shape::Disk { radius: 0.25 },
            emission: standard::Emission::Temperature,
            color_temperature: 3200.0,
            ..standard::Light::default()
        },
        standard::Light {
            shape: standard::Shape::Dome {
                image: None,
                ground: Some([0.2, 0.1, 0.05]),
            },
            emission: standard::Emission::of_key("D65").unwrap(),
            ..standard::Light::default()
        },
        // A dome's image: its file in a value, the decoded prim as sent.
        standard::Light {
            shape: standard::Shape::Dome {
                image: Some(standard::ImageRef::File(Arc::from("sky/noon.exr"))),
                ground: None,
            },
            ..standard::Light::default()
        },
        standard::Light {
            shape: standard::Shape::Dome {
                image: Some(standard::ImageRef::Prim(TextureId {
                    slot: 3,
                    generation: 1,
                })),
                ground: None,
            },
            ..standard::Light::default()
        },
        standard::Light {
            shape: standard::Shape::Distant { angle: 0.53 },
            intensity: 50_000.0,
            ..standard::Light::default()
        },
        standard::Light {
            shape: standard::Shape::Spot {
                radius: 0.1,
                cone_angle: 30.0,
                cone_softness: 0.2,
            },
            normalize: true,
            ..standard::Light::default()
        },
    ];
    for light in lights {
        let prim = light.declared();
        let ty = light.prim_type();
        assert_eq!(prim.type_id, ty.id);
        // Every value fits the type's schema, and it decodes as sent.
        assert_eq!(settings::validate(&ty.params, &prim.values), Ok(()));
        assert_eq!(standard::Light::decode(&prim), Some(light));
    }
    // Each type's defaults fit it and name each parameter once.
    for ty in standard::lights().into_iter().chain([standard::fog()]) {
        let defaults = ty.defaults();
        assert_eq!(
            settings::validate(&ty.params, &defaults),
            Ok(()),
            "{}",
            ty.id
        );
        let mut names: Vec<_> = ty.params.iter().map(|d| d.name.clone()).collect();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), ty.params.len(), "{}", ty.id);
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
fn rows_show_and_apply_by_the_values_beside_them() {
    let area = standard::area_light();
    let mut values = area.defaults();
    let shown = |values: &SettingValues, name: &str| param(&area, name).unwrap().is_shown(values);
    assert!(shown(&values, "width") && !shown(&values, "radius"));
    settings::set_value(&mut values, "shape", standard::choice("disk"));
    assert!(!shown(&values, "width") && shown(&values, "radius"));
    // Color shows for the color emission; Temperature applies to its own.
    let temperature = param(&area, "colorTemperature").unwrap();
    assert!(shown(&values, "color") && !temperature.applies(&values));
    settings::set_value(&mut values, "emission", standard::choice("temperature"));
    assert!(!shown(&values, "color") && temperature.applies(&values));
    assert_eq!(
        param(&area, "emission").unwrap().trailing,
        Some(Trailing::EmissionSwatch)
    );
    // A dome's image shows while it has no ground; an empty file is none.
    let dome = standard::dome_light();
    let mut sky = dome.defaults();
    let image = param(&dome, "texture").unwrap();
    assert!(image.is_shown(&sky));
    assert_eq!(
        settings::value(&sky, "texture"),
        Some(&SettingValue::File(Arc::from("")))
    );
    let imageless = standard::Shape::Dome {
        image: None,
        ground: None,
    };
    let prim = Declared::new(standard::DOME_LIGHT, sky.clone());
    assert_eq!(
        standard::Light::decode(&prim).map(|l| l.shape),
        Some(imageless)
    );
    settings::set_value(&mut sky, "ground", SettingValue::Bool(true));
    assert!(!image.is_shown(&sky));
    // Fitting keeps what fits by name, defaults the rest, drops strangers.
    let spot = standard::spot_light();
    let from_area = settings::fit(&spot.params, &values);
    assert_eq!(
        settings::value(&from_area, "emission"),
        Some(&standard::choice("temperature"))
    );
    assert_eq!(
        settings::value(&from_area, "radius"),
        Some(&SettingValue::Float(0.5)),
        "the disk's radius fits the spot"
    );
    assert_eq!(settings::value(&from_area, "width"), None);
    assert_eq!(from_area.len(), spot.params.len());
    let zero = vec![(Arc::from("radius"), SettingValue::Float(0.0))];
    assert_eq!(
        settings::value(&settings::fit(&spot.params, &zero), "radius"),
        Some(&SettingValue::Float(0.1)),
        "a spot keeps a size to shine"
    );
    // A group may open with a toggle.
    let mut group = SettingGroup::new("gobo", "Gobo", false);
    group.open_with = Some(Arc::from("gobo"));
    assert!(!group.is_open(&Vec::new()));
    assert!(group.is_open(&vec![(Arc::from("gobo"), SettingValue::Bool(true))]));
}

/// The parameter of `ty` named `name`.
fn param<'a>(ty: &'a PrimType, name: &str) -> Option<&'a SettingDescriptor> {
    ty.params.iter().find(|p| &*p.name == name)
}
