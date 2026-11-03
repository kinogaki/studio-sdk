//! Declared prims, materials, outputs and motion.
use super::*;
use kinogaki_geometry::{Sampled, Xform};

#[test]
fn engines_declare_types_and_apply_schemas_to_them() {
    let (wake, _) = counter();
    let mut d = RecordingEngine::new(wake).description().as_ref().clone();
    assert!(d.prim_type(standard::AREA_LIGHT).is_some());
    assert!(d.prim_type("kinogaki.other.rim_light").is_none());
    let gobo = AppliedSchema {
        applies_to: vec![Arc::from(standard::SPOT_LIGHT)],
        groups: vec![SettingGroup::new("gobo", "Gobo", false)],
        params: vec![SettingDescriptor::new(
            "gobo",
            "Gobo",
            "gobo",
            SettingKind::Bool { default: false },
        )],
    };
    d.applied.push(gobo);
    let (groups, params) = d.schema(standard::SPOT_LIGHT).unwrap();
    assert_eq!(params.last().unwrap().name.as_ref(), "gobo");
    assert_eq!(groups.len(), 2);
    let (_, area) = d.schema(standard::AREA_LIGHT).unwrap();
    assert!(
        area.iter().all(|p| &*p.name != "gobo"),
        "applies to spots only"
    );
    assert!(d.schema("kinogaki.other.rim_light").is_none());
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
    later.changed.push(Change::new(
        id,
        Dirty::PARAMS,
        Prim::Declared(thick.clone()),
    ));
    first.merge(later);
    assert_eq!(first.added, vec![(id, Prim::Declared(thick))]);
}

/// Material rules giving every material `tint`, flat materials of one
/// `flat` node in `engine`.
struct Rules {
    engine: EngineId,
    tint: Option<Tint>,
}

impl MaterialRules for Rules {
    fn tint(&self, _: &MaterialNetwork) -> Option<Tint> {
        self.tint.clone()
    }
    fn volume(&self, _: &MaterialNetwork) -> bool {
        false
    }
    fn flat(&self, tint: &Tint) -> MaterialNetwork {
        let Tint::Color(c) = tint else { unreachable!() };
        MaterialNetwork::single(
            self.engine.clone(),
            ShaderNode::new("flat").with("color", Constant::Color(*c)),
        )
    }
    fn default_material(&self) -> MaterialNetwork {
        MaterialNetwork::new(self.engine.clone())
    }
    fn preview_surface(&self, _: &MaterialNetwork) -> PreviewSurface {
        PreviewSurface::default()
    }
    fn import_preview_surface(&self, _: &PreviewSurface) -> MaterialNetwork {
        self.default_material()
    }
}

#[test]
fn another_engines_material_is_adopted_flat_in_its_tint() {
    let flat_of = |engine: &str| MaterialNetwork::new(EngineId::new(engine));
    let mut here: EngineDescription = RecordingEngine::new(Arc::new(|| {}))
        .description()
        .as_ref()
        .clone();
    here.id = EngineId::new("here");
    here.materials = Arc::new(Rules {
        engine: EngineId::new("here"),
        tint: None,
    });
    let mut there = here.clone();
    there.id = EngineId::new("there");
    there.materials = Arc::new(Rules {
        engine: EngineId::new("there"),
        tint: Some(Tint::Color([0.1, 0.2, 0.3])),
    });
    let own = flat_of("here");
    assert_eq!(here.adopt(own.clone(), &there), own, "its own: as it is");
    let adopted = here.adopt(flat_of("there"), &there);
    assert_eq!(adopted.engine.as_str(), "here");
    assert_eq!(
        adopted.nodes[0].value("color"),
        Some(Constant::Color([0.1, 0.2, 0.3]))
    );
    // No one tint: the default material.
    there.materials = Arc::new(Rules {
        engine: EngineId::new("there"),
        tint: None,
    });
    assert_eq!(
        here.adopt(flat_of("there"), &there),
        here.default_material()
    );
    let engines = Engines::new(vec![here.clone(), there]);
    assert_eq!(engines.default_engine().unwrap().id.as_str(), "here");
    assert!(engines.get(&EngineId::new("there")).is_some());
    assert!(engines.get(&EngineId::new("nope")).is_none());
    // A host with no engine installed has none to default to.
    let none = Engines::new(Vec::new());
    assert!(
        none.is_empty()
            && none.default_engine().is_none()
            && none.get(&EngineId::new("here")).is_none()
    );
}

#[test]
fn tints_say_their_color_and_attribute() {
    let cd: Arc<str> = Arc::from("Cd");
    assert_eq!(Tint::Attribute(cd.clone()).color(), None);
    assert_eq!(Tint::Attribute(cd.clone()).attribute(), Some(&cd));
    assert_eq!(Tint::Color([1.0; 3]).color(), Some([1.0; 3]));
    assert_eq!(Tint::Color([1.0; 3]).attribute(), None);
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
fn an_instancer_samples_all_its_copies_a_time_and_a_camera_blurs_while_open() {
    let copies: Arc<[Xform]> = Arc::from([Xform::IDENTITY; 3]);
    let moving = Sampled::new([0.0, 0.5], [copies.clone(), copies]).unwrap();
    assert!(!moving.is_still() && moving.values()[1].len() == 3);
    // A camera blurs while its shutter is open.
    let open = Camera {
        shutter: [-0.25, 0.25],
        ..Camera::default()
    };
    assert!(open.blurs() && !Camera::default().blurs());
    assert_eq!(format!("{:?}", Dirty::TRANSFORM), "Dirty(TRANSFORM)");
}
