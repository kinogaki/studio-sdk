use super::*;
use crate::element::MemoryElement;
use crate::params::{Schema, ValueKind};
use kinogaki_render_api::{Engines, Parameter, PrimTypeId, Section};

const LIGHTS: ObjectKindId = ObjectKindId::declared("acme.ray.light");
const FOG: ObjectKindId = ObjectKindId::declared("acme.ray.fog");

/// The standard lights, and one of the engine's own (a glow, no shape).
fn lights() -> DeclaredKind {
    let strength = Parameter::new(
        "strength",
        "Strength",
        ValueKind::Number(crate::params::Number::plain()),
        1.0,
    );
    let schema = (Schema::default())
        .section(Section::new("glow", "Glow", true))
        .parameter(strength.group("glow"));
    let glow = PrimType::new(
        PrimTypeId::declared("acme.ray.glow"),
        "Glow",
        Role::Light,
        schema,
    );
    let types = [standard::lights(), vec![standard::fog(), glow]].concat();
    DeclaredKind::lights(LIGHTS, Home::family().group("Lights"), types)
}

/// An engine declaring the kind's types (`lights()`'s and the fog).
fn rendering() -> Engines {
    let lights = lights().types;
    let types = [lights, vec![standard::fog()]].concat();
    let capabilities = kinogaki_render_api::Capabilities {
        types,
        ..Default::default()
    };
    let id = kinogaki_render_api::EngineId::declared("acme.ray.engine");
    let ray = kinogaki_render_api::EngineDescription::builder(id, "Ray");
    Engines::new(vec![ray.capabilities(capabilities).build()])
}

fn rect() -> Declared {
    standard::Light::default().declared()
}

fn cx(engines: &Engines) -> CreateCx<'_> {
    CreateCx::new(engines)
}

#[test]
fn a_new_light_is_its_first_type_and_edits_take_one_prim_of_its_types() {
    let kind = lights();
    let body = kind.new_body();
    assert_eq!(kind.declared(&*body), [rect()]);
    let mut prim = rect();
    prim.values.insert("intensity".into(), Value::Number(4.0));
    let edited = kind.with_declared(&*body, &[prim.clone()]).unwrap();
    assert_eq!(kind.declared(&*edited), [prim.clone()]);
    // Not two prims, none, the fog (an atmosphere), another engine's type,
    // a value that isn't finite or out of its limits.
    assert!(
        kind.with_declared(&*body, &[prim.clone(), prim.clone()])
            .is_none()
    );
    assert!(kind.with_declared(&*body, &[]).is_none());
    let fog = Declared::new(standard::FOG, standard::fog().schema().values());
    assert!(kind.with_declared(&*body, &[fog]).is_none());
    let rim = Declared::new(
        PrimTypeId::declared("acme.other.rim_light"),
        prim.values.clone(),
    );
    assert!(kind.with_declared(&*body, &[rim]).is_none());
    for bad in [f64::NAN, -1.0] {
        let mut wrong = prim.clone();
        wrong.values.insert("intensity".into(), Value::Number(bad));
        assert!(kind.with_declared(&*body, &[wrong]).is_none(), "{bad}");
    }
}

#[test]
fn creations_are_the_kinds_types_an_engine_renders_under_its_prefix() {
    let kind = lights();
    let none = Engines::new(Vec::new());
    assert!(
        kind.creations(&cx(&none)).is_empty(),
        "no engine renders them"
    );
    let engines = rendering();
    let made = kind.creations(&cx(&engines));
    let labels: Vec<&str> = made.iter().map(|c| c.label.as_str()).collect();
    assert_eq!(
        labels,
        [
            "Rect Light",
            "Disk Light",
            "Sphere Light",
            "Spot Light",
            "Cylinder Light",
            "Distant Light",
            "Dome Light",
            "Glow"
        ]
    );
    assert_eq!(made[0].id, CreationId::declared("acme.ray.rect_light"));
    assert_eq!(made[0].home, Home::family().group("Lights"));
    assert!(made[0].aliases.contains(&"area".to_owned()));
    let glow = kind.create(&made[7], &cx(&engines)).unwrap();
    assert_eq!(kind.declared(&*glow)[0].type_id.as_str(), "acme.ray.glow");
    // The Spot Light is a sphere light, shaped.
    assert_eq!(made[3].id, CreationId::declared("acme.ray.spot_light"));
    let spot = kind.create(&made[3], &cx(&engines)).unwrap();
    let spot = standard::Light::decode(&kind.declared(&*spot)[0]).unwrap();
    assert_eq!(spot, standard::presets()[0].light);
    let fog = KindCreation::new(CreationId::declared("acme.ray.fog"), "Fog", Home::family());
    assert!(kind.create(&fog, &cx(&engines)).is_err());
    assert!(kind.placed() && !kind.at_root());
}

#[test]
fn an_atmosphere_is_one_type_at_the_root() {
    let render = crate::menu::topics::objects::RENDER;
    let (fog, engines) = (
        DeclaredKind::atmosphere(FOG, Home::topic(render), standard::fog()),
        rendering(),
    );
    assert!(fog.at_root() && !fog.placed() && fog.result().is_none());
    let made = fog.creations(&cx(&engines));
    assert_eq!(
        (made[0].label.as_str(), made[0].id),
        ("Fog", CreationId::declared("acme.ray.fog"))
    );
    assert_eq!(made[0].home, Home::topic(render));
    let body = fog.create(&made[0], &cx(&engines)).unwrap();
    let mut denser = fog.declared(&*body).remove(0);
    denser.values.insert("density".into(), Value::Number(0.4));
    assert_eq!(
        fog.declared(&*fog.with_declared(&*body, &[denser.clone()]).unwrap()),
        [denser]
    );
    assert!(fog.glyphs().is_none());
    // A light kind's body is not the fog's.
    assert!(
        fog.with_declared(&*lights().new_body(), &[rect()])
            .is_none()
    );
}

#[test]
fn an_object_round_trips_keeping_what_its_type_doesnt_declare() {
    let kind = lights();
    let mut prim = rect();
    prim.values.insert("intensity".into(), Value::Number(0.25));
    prim.values
        .insert("acmeOwn".into(), Value::Text("kept".into()));
    let body = kind.body(&prim).unwrap();
    let mut element = MemoryElement::new(LIGHTS.as_str());
    element.write(|out| kind.write(&*body, out)).unwrap();
    let engines = Engines::new(Vec::new());
    let back = element
        .read(|r| kind.read(r, &ReadCx::new(&engines)))
        .unwrap();
    assert_eq!(kind.declared(&*back), [prim]);
    // A type the kind doesn't have is refused, naming it.
    let mut other = MemoryElement::new(LIGHTS.as_str());
    other
        .write(|out| {
            out.set("type", Field::Text("acme.other.rim_light".into()))?;
            out.child(VALUES, VALUES).map(|_| ())
        })
        .unwrap();
    let why = other
        .read(|r| kind.read(r, &ReadCx::new(&engines)))
        .unwrap_err();
    assert!(
        why.to_string().contains("has no type acme.other.rim_light"),
        "{why}"
    );
}

#[test]
fn a_standard_light_is_a_light_value_and_one_is_adopted() {
    let kind = lights();
    let spot = standard::Light {
        shape: LightShape::Sphere {
            radius: 0.2,
            point: false,
        },
        shaping: Some(standard::Shaping {
            cone_angle: 40.0,
            cone_softness: 0.3,
            ..standard::Shaping::default()
        }),
        intensity: 5.0,
        ..standard::Light::default()
    };
    let engines = Engines::new(Vec::new());
    assert_eq!(kind.adopted(), [data::light().id()]);
    let body = kind
        .adopts(&data::light().wrap(spot.clone()).unwrap(), &cx(&engines))
        .unwrap()
        .body;
    let valued = kind.value(&*body, "/Spot").unwrap();
    assert_eq!(data::light().get(&valued.value), Some(&spot));
    assert!(valued.warnings.is_empty());
    // A parameter of the engine's own on it is left out, said.
    let mut prim = kind.declared(&*body).remove(0);
    prim.values.insert("gobo".into(), Value::Bool(true));
    let gobo = kind.body(&prim).unwrap();
    let valued = kind.value(&*gobo, "/Spot").unwrap();
    assert_eq!(data::light().get(&valued.value), Some(&spot));
    assert_eq!(valued.warnings.len(), 1);
}

/// What a standard light's rows take is a light value its codec reads back:
/// a size of 0 is refused, the least size is taken and saves, and so does
/// any finite color.
#[test]
fn a_standard_lights_rows_take_only_what_its_codec_reads() {
    use crate::codec::DataCodec;
    let kind = lights();
    let body = kind.new_body();
    let sized = |width: f64| {
        let mut prim = rect();
        prim.values.insert("width".into(), Value::Number(width));
        prim
    };
    assert!(kind.with_declared(&*body, &[sized(0.0)]).is_none());
    let least = kind
        .with_declared(&*body, &[sized(f64::from(1e-6_f32))])
        .unwrap();
    let valued = kind.value(&*least, "/Area").unwrap().value;
    let mut e = MemoryElement::new("light");
    e.write(|w| crate::render::lights::LightCodec.write(&valued, w))
        .unwrap();
    let back = e
        .read(|r| crate::render::lights::LightCodec.read(r))
        .unwrap();
    assert_eq!(data::light().get(&back), data::light().get(&valued));
    let mut bright = rect();
    bright
        .values
        .insert("color".into(), Value::Color([1e20, 1.0, 1.0]));
    let bright = kind.with_declared(&*body, &[bright]).unwrap();
    let valued = kind.value(&*bright, "/Area").unwrap().value;
    let mut e = MemoryElement::new("light");
    e.write(|w| crate::render::lights::LightCodec.write(&valued, w))
        .unwrap();
    let back = e
        .read(|r| crate::render::lights::LightCodec.read(r))
        .unwrap();
    assert_eq!(data::light().get(&back), data::light().get(&valued));
}
