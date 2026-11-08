//! Declared prims, materials, outputs and motion.
use super::*;

#[test]
fn engines_declare_types() {
    let (wake, _) = counter();
    let d = RecordingEngine::new(wake).description().as_ref().clone();
    assert!(d.prim_type(standard::RECT_LIGHT).is_some());
    assert!(
        d.prim_type(PrimTypeId::declared("kinogaki.other.rim_light"))
            .is_none()
    );
    let sphere = d.prim_type(standard::SPHERE_LIGHT).unwrap();
    assert_eq!(sphere.schema(), standard::sphere_light().schema());
    assert_eq!(sphere.id.as_str(), "kinogaki.core.sphere_light");
    assert_eq!(
        Prim::Declared(standard::Light::default().declared()).kind(),
        PrimKind::Declared
    );
    assert!(!PrimKind::STRUCTURAL.contains(&PrimKind::Declared));
}

#[test]
fn a_later_declared_prim_replaces_the_one_a_batch_sends() {
    let id = PrimId {
        slot: 3,
        generation: 0,
    };
    let mut first = SceneUpdate::new();
    first
        .added
        .push((id, Prim::Declared(standard::Light::default().declared())));
    let thick = standard::Light {
        intensity: 2.0,
        ..standard::Light::default()
    }
    .declared();
    let mut later = SceneUpdate::new();
    later.changed.push(PrimChange::new(
        id,
        Dirty::PARAMS,
        Prim::Declared(thick.clone()),
    ));
    first.merge(later);
    assert_eq!(first.added, vec![(id, Prim::Declared(thick))]);
}

/// Rules approximating every material as one color, whose standard form is
/// an OpenPBR Surface of that color, and which translate the standard form to
/// a node of their own.
struct Rules(NodeDef);

impl MaterialRules for Rules {
    fn display(&self, _: &MaterialNetwork) -> MaterialDisplay {
        MaterialDisplay::color([0.1, 0.2, 0.3])
    }
    fn standard(&self, _: &MaterialNetwork) -> Option<MaterialNetwork> {
        crate::material::standard::open_pbr(
            &[(
                "base_color".to_owned(),
                value::Value::Color([0.1, 0.2, 0.3]),
            )]
            .into(),
        )
        .ok()
    }
    fn translate(&self, _: &MaterialNetwork) -> Option<MaterialNetwork> {
        let mut built = MaterialNetwork::builder();
        let own = built
            .node(&Arc::new(self.0.clone()), &Default::default())
            .ok()?;
        built.build(own).ok()
    }
}

/// `engine`'s one node def.
fn def(engine: &str) -> NodeDef {
    let id = kinogaki_value::NodeTypeId::declared(if engine == "here" {
        "toy.here.paint"
    } else {
        "toy.there.paint"
    });
    NodeDef::new(id, "Paint", "Surface").output(NodeOutput::new(
        "out",
        "Out",
        kinogaki_mtlx::Type::of("surfaceshader"),
    ))
}

/// The network of `def` alone.
fn single(def: NodeDef) -> MaterialNetwork {
    let mut built = MaterialNetwork::builder();
    let own = built.node(&Arc::new(def), &Default::default()).unwrap();
    built.build(own).unwrap()
}

#[test]
fn an_engine_keeps_its_own_material_and_translates_anothers_standard_form() {
    let base = RecordingEngine::new(Arc::new(|| {})).description().clone();
    let here = (base.rebuilt().id(EngineId::declared("toy.here.engine")))
        .materials(vec![def("here")], Arc::new(Rules(def("here"))))
        .build();
    let there = (base.rebuilt().id(EngineId::declared("toy.there.engine")))
        .materials(vec![def("there")], Arc::new(Rules(def("there"))))
        .build();
    let all = Engines::new(vec![here.clone(), there.clone(), (*base).clone()]);
    let own = single(def("here"));
    assert_eq!(all.adopt(&own, here.id()), own, "its own: as it is");
    assert_eq!(
        all.adopt(&single(def("there")), here.id()),
        own,
        "another's: its standard form, translated"
    );
    assert_eq!(here.display(&own), MaterialDisplay::color([0.1, 0.2, 0.3]));
    // An engine with no rules for it renders its default: an empty network,
    // naming what it couldn't render.
    let unrendered = all.adopt(&own, base.id());
    assert!(unrendered.output().is_none() && !unrendered.cut().is_empty());
    let engines = Engines::new(vec![here.clone(), there]);
    assert_eq!(
        engines.owner(&own).map(|e| e.id().as_str()),
        Some("toy.here.engine")
    );
    let theirs = single(def("there"));
    assert_eq!(
        engines.owner(&theirs).map(|e| e.id().as_str()),
        Some("toy.there.engine")
    );
    assert!(engines.owner(&MaterialNetwork::default()).is_none());
    assert_eq!(
        engines.default_engine().unwrap().id().as_str(),
        "toy.here.engine"
    );
    assert!(
        engines
            .get(EngineId::declared("toy.there.engine"))
            .is_some()
    );
    assert!(engines.get(EngineId::declared("toy.nope.engine")).is_none());
    // A host with no engine installed has none to default to.
    let none = Engines::new(Vec::new());
    assert!(
        none.is_empty()
            && none.default_engine().is_none()
            && none.get(EngineId::declared("toy.here.engine")).is_none()
    );
}

#[test]
fn outputs_have_one_id_each_and_engines_list_theirs() {
    use aov::standard;
    let all = standard::all();
    let mut ids: Vec<_> = all.iter().map(|a| a.id.clone()).collect();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), all.len());
    let get = |id| standard::get(id).unwrap();
    assert_eq!(get(standard::BEAUTY).channels(), 4);
    assert_eq!(get(standard::DIRECT).channels(), 3);
    assert_eq!(get(standard::OCCLUSION).channels(), 1);
    assert!(get(standard::INDIRECT).is_color() && get(standard::INDIRECT).channels() == 3);
    assert!(get(standard::VOLUME).is_color() && get(standard::VOLUME).channels() == 4);
    assert!(!get(standard::OCCLUSION).is_color());
    assert!(get(standard::OBJECT_ID).is_id() && get(standard::BEAUTY).is_beauty());
    let (wake, _) = counter();
    let engine = RecordingEngine::new(wake);
    let d = engine.description();
    assert_eq!(d.aov("beauty"), Some(&standard::beauty()));
    assert_eq!(d.aov("direct"), None);
}

#[test]
fn a_camera_blurs_while_its_shutter_is_open() {
    let open = Camera {
        shutter: [-0.25, 0.25],
        ..Camera::default()
    };
    assert!(open.blurs() && !Camera::default().blurs());
    assert_eq!(format!("{:?}", Dirty::TRANSFORM), "Dirty(TRANSFORM)");
}
