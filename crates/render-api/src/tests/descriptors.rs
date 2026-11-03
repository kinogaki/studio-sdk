//! Vocabularies, networks, settings and the standard prims.
use super::*;
use kinogaki_value::ramp::{Interpolation, Key, Ramp};
use kinogaki_value::{Choice, Condition, Parameters, Value, ValueKind};

/// Values by name, from pairs.
fn values<const N: usize>(pairs: [(&str, Value); N]) -> Parameters {
    pairs.into_iter().map(|(n, v)| (n.to_owned(), v)).collect()
}

/// A tiny vocabulary: one surface whose `mode` choice decides whether it
/// has a second input.
fn tiny_layout(_: &str, params: &Parameters) -> NodeLayout {
    let two = params.get("mode").and_then(Value::choice) == Some("two");
    let mut inputs = vec![SocketSpec::new(
        "color",
        "Color",
        SocketKind::Color,
        [0.5; 3],
    )];
    if two {
        inputs.push(SocketSpec::new("amount", "Amount", SocketKind::Float, 1.0));
    }
    NodeLayout {
        params: vec![ParamSpec {
            name: Arc::from("mode"),
            label: Arc::from("Mode"),
            kind: ValueKind::choice(vec![Choice::new("one", "One"), Choice::new("two", "Two")]),
            default: standard::choice("one"),
            usage: None,
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
        defaults: values([("mode", standard::choice("one"))]),
        layout: Arc::new(tiny_layout),
    }
}

#[test]
fn node_types_fit_their_settings_and_lay_out_by_them() {
    let t = tiny();
    assert_eq!(t.layout(&t.defaults).inputs.len(), 1);
    // A choice it does not offer takes the default; a missing one too.
    let bad = values([("mode", standard::choice("three"))]);
    assert_eq!(t.fit(&bad), Some(t.defaults.clone()));
    assert_eq!(t.fit(&Parameters::new()), Some(t.defaults.clone()));
    let two = values([("mode", standard::choice("two"))]);
    assert_eq!(t.fit(&two), Some(two.clone()));
    let layout = t.layout(&two);
    assert_eq!(layout.input("amount"), Some(1));
    assert!(layout.params[0].accepts(&two["mode"]));
    // A layout whose settings never settle is reported, not cut short.
    let flip = NodeType {
        layout: Arc::new(|_, params: &Parameters| {
            let one = params.get("mode").and_then(Value::choice) == Some("one");
            let mut layout = tiny_layout("", params);
            layout.params[0].default = standard::choice(if one { "two" } else { "one" });
            layout.params[0].kind =
                ValueKind::choice(vec![Choice::new(if one { "two" } else { "one" }, "Next")]);
            layout
        }),
        ..tiny()
    };
    assert_eq!(flip.fit(&flip.defaults), None);
}

#[test]
fn settings_fit_their_descriptors() {
    let d = SettingDescriptor::new(
        "samples",
        "Samples",
        "main",
        ValueKind::integer(1, 64),
        8_i64,
    );
    let ds = vec![d.clone()];
    assert_eq!(
        settings::defaults(&ds),
        values([("samples", Value::Integer(8))])
    );
    settings::validate(&ds, &values([("samples", Value::Integer(64))])).unwrap();
    for bad in [
        values([("samples", Value::Integer(65))]),
        values([("samples", Value::Number(8.0))]),
        values([("seed", Value::Integer(1))]),
    ] {
        assert!(settings::validate(&ds, &bad).is_err());
    }
    // Animatable by its kind's default, unless the engine says otherwise;
    // an image never.
    assert!(d.animatable && !d.clone().animatable(false).animatable);
    assert!(!SettingDescriptor::new("t", "T", "main", ValueKind::File, "").animatable);
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
    let ds = vec![
        SettingDescriptor::new("ramp", "Ramp", "main", kind, ramp(stops.clone())),
        SettingDescriptor::new("sky", "Sky", "main", ValueKind::Color, [0.1, 0.2, 0.3]),
    ];
    assert_eq!(
        settings::defaults(&ds)["sky"],
        Value::Color([0.1, 0.2, 0.3])
    );
    settings::validate(&ds, &settings::defaults(&ds)).unwrap();
    let infinite = vec![key(0.0, [f64::INFINITY, 0.0, 0.0], Interpolation::Linear)];
    for bad in [
        ("ramp", ramp(vec![stops[2], stops[0]])),
        ("ramp", ramp(vec![stops[0]; 5])),
        ("ramp", ramp(Vec::new())),
        ("ramp", ramp(infinite)),
        ("ramp", Value::Ramp(Ramp::default_of(false))),
        ("sky", Value::Color([f64::NAN, 0.0, 0.0])),
    ] {
        assert!(settings::validate(&ds, &values([bad])).is_err());
    }
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
        // A dome's image: its file in a value.
        standard::Light {
            shape: standard::Shape::Dome {
                image: Some(Arc::from("sky/noon.exr")),
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
        assert_eq!(defaults.len(), ty.params.len(), "{}", ty.id);
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
    let shown = |values: &Parameters, name: &str| param(&area, name).unwrap().is_shown(values);
    assert!(shown(&values, "width") && !shown(&values, "radius"));
    values.insert("shape".into(), standard::choice("disk"));
    assert!(!shown(&values, "width") && shown(&values, "radius"));
    // Color shows for the color emission; Temperature applies to its own.
    let temperature = param(&area, "colorTemperature").unwrap();
    assert!(shown(&values, "color") && !temperature.applies(&values));
    values.insert("emission".into(), standard::choice("temperature"));
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
    assert_eq!(sky.get("texture"), Some(&Value::from("")));
    let imageless = standard::Shape::Dome {
        image: None,
        ground: None,
    };
    let prim = Declared::new(standard::DOME_LIGHT, sky.clone());
    assert_eq!(
        standard::Light::decode(&prim).map(|l| l.shape),
        Some(imageless)
    );
    sky.insert("ground".into(), Value::Bool(true));
    assert!(!image.is_shown(&sky));
    // Fitting keeps what fits by name, defaults the rest, drops strangers.
    let spot = standard::spot_light();
    let from_area = settings::fit(&spot.params, &values);
    assert_eq!(
        from_area.get("emission"),
        Some(&standard::choice("temperature"))
    );
    assert_eq!(
        from_area.get("radius"),
        Some(&Value::Number(0.5)),
        "the disk's radius fits the spot"
    );
    assert_eq!(from_area.get("width"), None);
    assert_eq!(from_area.len(), spot.params.len());
    let zero = self::values([("radius", Value::Number(0.0))]);
    assert_eq!(
        settings::fit(&spot.params, &zero).get("radius"),
        Some(&Value::Number(0.1)),
        "a spot keeps a size to shine"
    );
    // A group may open with a toggle.
    let mut group = SettingGroup::new("gobo", "Gobo", false);
    group.open_when = Some(Condition::when("gobo", true));
    assert!(!group.is_open(&Parameters::new()));
    assert!(group.is_open(&self::values([("gobo", Value::Bool(true))])));
}

/// The parameter of `ty` named `name`.
fn param<'a>(ty: &'a PrimType, name: &str) -> Option<&'a SettingDescriptor> {
    ty.params.iter().find(|p| &*p.name == name)
}
