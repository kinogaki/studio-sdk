//! The interface's contracts: coalescing, ids, one-shot results, networks.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use kinogaki_render_api::*;

fn mesh(points: usize) -> Prim {
    Prim::Mesh(Mesh {
        points: Arc::from(vec![[0.0f32; 3]; points]),
        triangles: Arc::from(vec![[0u32, 1, 2]]),
        normals: None,
        uvs: None,
        material_slots: Arc::from(vec![0u16]),
        attributes: Arc::from([]),
        materials: Arc::from(Vec::new()),
        transform: IDENTITY,
        visible: true,
        id: 1,
        path: Arc::from("/mesh"),
    })
}

fn id(slot: u32) -> PrimId {
    PrimId {
        slot,
        generation: 0,
    }
}

fn counter() -> (Wake, Arc<AtomicUsize>) {
    let count = Arc::new(AtomicUsize::new(0));
    let held = count.clone();
    (
        Arc::new(move || {
            held.fetch_add(1, Ordering::SeqCst);
        }),
        count,
    )
}

#[test]
fn an_add_then_a_remove_cancel() {
    let mut first = SceneUpdate::new();
    first.added.push((id(0), mesh(3)));
    first.added.push((id(1), mesh(3)));
    let mut second = SceneUpdate::new();
    second
        .changed
        .push(Change::new(id(0), Dirty::TRANSFORM, mesh(3)));
    second.removed.push(id(0));
    first.merge(second);
    assert_eq!(first.added.len(), 1);
    assert_eq!(first.added[0].0, id(1));
    assert!(first.removed.is_empty() && first.changed.is_empty());
}

#[test]
fn a_change_to_an_added_prim_replaces_its_payload() {
    let mut first = SceneUpdate::new();
    first.added.push((id(0), mesh(3)));
    let mut second = SceneUpdate::new();
    second
        .changed
        .push(Change::new(id(0), Dirty::POINTS, mesh(5)));
    first.merge(second);
    assert_eq!(first.added, vec![(id(0), mesh(5))]);
    assert!(first.changed.is_empty());
}

#[test]
fn dirty_bits_or_and_the_later_payload_wins() {
    let mut first = SceneUpdate::new();
    first
        .changed
        .push(Change::new(id(2), Dirty::TRANSFORM, mesh(3)));
    let mut second = SceneUpdate::new();
    second
        .changed
        .push(Change::new(id(2), Dirty::VISIBILITY, mesh(4)));
    first.merge(second);
    assert_eq!(first.changed.len(), 1);
    assert_eq!(first.changed[0].dirty, Dirty::TRANSFORM | Dirty::VISIBILITY);
    assert_eq!(first.changed[0].prim, mesh(4));
    // A removal after a change drops the change.
    let mut third = SceneUpdate::new();
    third.removed.push(id(2));
    first.merge(third);
    assert!(first.changed.is_empty());
    assert_eq!(first.removed, vec![id(2)]);
}

#[test]
fn point_ranges_merge_and_fall_back_to_whole_past_the_cap() {
    let mut first = SceneUpdate::new();
    first
        .changed
        .push(Change::points(id(0), mesh(100_000), Ranges::of(0..10)));
    let mut second = SceneUpdate::new();
    second
        .changed
        .push(Change::points(id(0), mesh(100_000), Ranges::of(20..30)));
    let mut third = SceneUpdate::new();
    third
        .changed
        .push(Change::points(id(0), mesh(100_000), Ranges::of(500..510)));
    first.merge(second);
    first.merge(third);
    let points = &first.changed[0].points;
    // 10..20 is within the gap: one span; 500 is far: a second.
    assert_eq!(points.spans(), &[0..30, 500..510]);
    assert_eq!(points.len(), 40);
    // Many scattered spans: the whole stream.
    let mut ranges = Ranges::new();
    for at in 0..=Ranges::MOST as u32 {
        ranges.add(at * 100..at * 100 + 1);
    }
    assert!(ranges.is_whole());
    // A whole change absorbs a patch, and a patch after TRANSFORM keeps its ranges.
    let mut whole = SceneUpdate::new();
    whole
        .changed
        .push(Change::new(id(0), Dirty::POINTS, mesh(10)));
    let mut patch = SceneUpdate::new();
    patch
        .changed
        .push(Change::points(id(0), mesh(10), Ranges::of(1..2)));
    whole.merge(patch);
    assert!(whole.changed[0].points.is_whole());
    let mut moved = SceneUpdate::new();
    moved
        .changed
        .push(Change::new(id(0), Dirty::TRANSFORM, mesh(10)));
    let mut patch = SceneUpdate::new();
    patch
        .changed
        .push(Change::points(id(0), mesh(10), Ranges::of(1..2)));
    moved.merge(patch);
    assert_eq!(moved.changed[0].points.spans().to_vec(), vec![1..2]);
    assert_eq!(moved.changed[0].dirty, Dirty::TRANSFORM | Dirty::POINTS);
}

#[test]
fn settling_survives_later_non_drag_batches() {
    let mut first = SceneUpdate::new();
    first.settled = true;
    let mut camera = SceneUpdate::new();
    camera.changed.push(Change::new(
        id(9),
        Dirty::TRANSFORM,
        Prim::Camera(Camera::default()),
    ));
    first.merge(camera);
    assert!(first.settled);
    let mut drag = SceneUpdate::new();
    drag.changed
        .push(Change::points(id(0), mesh(10), Ranges::of(0..1)));
    first.merge(drag);
    assert!(!first.settled);
}

#[test]
fn ids_are_reused_only_with_a_new_generation() {
    let mut ids = IdAllocator::new();
    let a = ids.allocate();
    let b = ids.allocate();
    assert_eq!((a.slot, b.slot), (0, 1));
    assert!(ids.release(a));
    assert!(!ids.release(a), "a released id cannot be released again");
    assert!(!ids.is_live(a));
    let c = ids.allocate();
    assert_eq!(c.slot, a.slot);
    assert_ne!(c.generation, a.generation);
    assert!(ids.is_live(c) && !ids.is_live(a));
    assert!(!ids.release(a), "the old id does not name the new prim");
    assert_eq!(ids.slots(), 2);
}

#[test]
fn pending_delivers_once_and_wakes_once() {
    let (wake, wakes) = counter();
    let (deliver, result) = pending::<u32>(wake);
    assert!(!result.is_delivered());
    assert_eq!(result.take(), None);
    assert_eq!(wakes.load(Ordering::SeqCst), 0);
    let worker = std::thread::spawn(move || deliver.deliver(7));
    worker.join().expect("the producer runs");
    assert!(result.is_delivered());
    assert_eq!(wakes.load(Ordering::SeqCst), 1);
    assert_eq!(result.take(), Some(7));
    assert_eq!(result.take(), None);
    assert_eq!(wakes.load(Ordering::SeqCst), 1);
}

#[test]
fn the_recording_engine_counts_what_it_was_sent() {
    let (wake, wakes) = counter();
    let mut engine = RecordingEngine::new(wake);
    let mut first = SceneUpdate::new();
    first.added.push((id(0), mesh(3)));
    engine.update(first);
    let mut moved = SceneUpdate::new();
    moved
        .changed
        .push(Change::new(id(0), Dirty::TRANSFORM, mesh(3)));
    engine.update(moved);
    let mut drag = SceneUpdate::new();
    drag.changed
        .push(Change::points(id(0), mesh(3), Ranges::of(1..2)));
    engine.update(drag);
    let counts = &engine.counts;
    assert_eq!((counts.updates, counts.added, counts.changed), (3, 1, 2));
    assert_eq!(counts.triangles, 1, "moves and drags send no triangles");
    assert_eq!(counts.points, 4);
    assert_eq!(counts.dirty[5], 1);
    engine.view(
        ViewId(1),
        ViewDesc {
            camera: ViewCamera::Free(Camera::default()),
            width: 4,
            height: 2,
            mode: ViewMode::Final,
        },
    );
    let result = engine.finish(
        ViewId(1),
        &[AovDesc {
            aov: aov::standard::beauty(),
            format: AovFormat::F32,
        }],
    );
    let outputs = result.take().expect("delivered at once");
    assert_eq!(outputs.planes[0].1.len(), 4 * 2 * 4);
    assert_eq!(wakes.load(Ordering::SeqCst), 1);
}

#[test]
fn mesh_attributes_hold_one_value_an_element_of_their_domain() {
    let attribute = |domain, components, values: usize| MeshAttribute {
        name: Arc::from("Cd"),
        domain,
        components,
        color: true,
        integer: false,
        values: Arc::from(vec![0.5f32; values]),
    };
    // Four points, two triangles.
    assert!(attribute(AttributeDomain::Point, 3, 12).is_consistent(4, 2));
    assert!(attribute(AttributeDomain::Corner, 1, 6).is_consistent(4, 2));
    assert!(attribute(AttributeDomain::Triangle, 4, 8).is_consistent(4, 2));
    assert!(attribute(AttributeDomain::Constant, 2, 2).is_consistent(4, 2));
    assert!(!attribute(AttributeDomain::Point, 3, 9).is_consistent(4, 2));
    assert!(!attribute(AttributeDomain::Constant, 5, 5).is_consistent(4, 2));
    assert!(!attribute(AttributeDomain::Constant, 0, 0).is_consistent(4, 2));
    assert_eq!(format!("{:?}", Dirty::ATTRIBUTES), "Dirty(ATTRIBUTES)");
}

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
        engine: Arc::from("tiny"),
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
fn networks_check_their_links_and_name_their_attributes() {
    let node = |kind: &str| ShaderNode::new(kind);
    let mut n = MaterialNetwork {
        engine: Arc::from("tiny"),
        nodes: vec![
            node("tiny_surface").with("color", Constant::Color([0.2, 0.4, 0.6])),
            node("reader").with_param("attribute", ParamValue::Attribute(Arc::from("Cd"))),
        ],
        links: vec![Link {
            from: 1,
            output: Arc::from("color"),
            to: 0,
            input: Arc::from("color"),
        }],
    };
    n.validate_links().unwrap();
    assert_eq!(n.attribute_names(), [Arc::from("Cd")]);
    n.map_colors(|c| c.map(|v| v * 2.0));
    assert_eq!(
        n.nodes[0].value("color"),
        Some(Constant::Color([0.4, 0.8, 1.2]))
    );
    let mut twice = n.clone();
    twice.links.push(twice.links[0].clone());
    assert!(matches!(
        twice.validate_links(),
        Err(NetworkError::LinkedTwice(0, _))
    ));
    let mut cycle = n.clone();
    cycle.links.push(Link {
        from: 0,
        output: Arc::from("out"),
        to: 1,
        input: Arc::from("in"),
    });
    assert!(matches!(
        cycle.validate_links(),
        Err(NetworkError::Cycle(_))
    ));
    let mut out = n.clone();
    out.links[0].from = 7;
    assert_eq!(out.validate_links(), Err(NetworkError::NodeOutOfRange(7)));
    assert!(n.same_wiring(&n.clone()));
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
    let mut r = RenderSettings::default();
    r.set("samples", SettingValue::Int(3));
    r.set("samples", SettingValue::Int(4));
    assert_eq!(r.value("samples"), Some(&SettingValue::Int(4)));
}

#[test]
fn colors_and_color_ramps_fit_their_descriptors_and_ramps_blend() {
    let stops: Arc<[ColorStop]> = Arc::from(vec![
        ColorStop::new(0.0, [0.0, 0.0, 1.0]),
        ColorStop::new(0.5, [1.0, 1.0, 1.0]),
        ColorStop::new(1.0, [1.0, 0.0, 0.0]),
    ]);
    let ramp = SettingKind::ColorRamp {
        default: stops.clone(),
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
    let too_many = Arc::from(vec![stops[0]; MAX_RAMP_STOPS + 1]);
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
    // Linear between stops, the ends' colors beyond them.
    assert_eq!(ramp_color(&stops, 0.25), [0.5, 0.5, 1.0]);
    assert_eq!(ramp_color(&stops, -1.0), [0.0, 0.0, 1.0]);
    assert_eq!(ramp_color(&stops, 2.0), [1.0, 0.0, 0.0]);
    assert_eq!(ramp_color(&stops[..1], 0.7), [0.0, 0.0, 1.0]);
}

#[test]
fn spot_cones_fade_over_their_softness() {
    let cos = |deg: f32| deg.to_radians().cos();
    // A hard 30° cone.
    assert_eq!(standard::cone_falloff(cos(29.0), 30.0, 0.0), 1.0);
    assert_eq!(standard::cone_falloff(cos(31.0), 30.0, 0.0), 0.0);
    // Softness 0.5: full inside 15°, half way at 22.5° (by cosine), none past 30°.
    assert_eq!(standard::cone_falloff(cos(14.0), 30.0, 0.5), 1.0);
    assert_eq!(standard::cone_falloff(cos(30.5), 30.0, 0.5), 0.0);
    let mid = standard::cone_falloff((cos(15.0) + cos(30.0)) / 2.0, 30.0, 0.5);
    assert!((mid - 0.5).abs() < 1e-4, "{mid}");
    // 180° lights every direction.
    assert_eq!(standard::cone_falloff(-1.0, 180.0, 0.0), 1.0);
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
                texture: None,
                ground: Some([0.2, 0.1, 0.05]),
            },
            emission: standard::Emission::of_key("D65").unwrap(),
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
    let fog = standard::Fog {
        density: 2.0,
        inscatter: true,
        ..standard::Fog::default()
    };
    assert_eq!(standard::Fog::decode(&fog.declared()), Some(fog));
    assert_eq!(
        standard::Fog::decode(&standard::Light::default().declared()),
        None
    );
    assert_eq!(standard::Light::decode(&fog.declared()), None);
    assert_eq!(
        standard::Fog::default().density,
        0.1,
        "the schema's defaults"
    );
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
    let shown = |values: &SettingValues, name: &str| area.param(name).unwrap().is_shown(values);
    assert!(shown(&values, "width") && !shown(&values, "radius"));
    settings::set_value(&mut values, "shape", standard::choice("disk"));
    assert!(!shown(&values, "width") && shown(&values, "radius"));
    // Color shows for the color emission; Temperature applies to its own.
    let temperature = area.param("colorTemperature").unwrap();
    assert!(shown(&values, "color") && !temperature.applies(&values));
    settings::set_value(&mut values, "emission", standard::choice("temperature"));
    assert!(!shown(&values, "color") && temperature.applies(&values));
    assert_eq!(
        area.param("emission").unwrap().trailing,
        Some(Trailing::EmissionSwatch)
    );
    // A dome's image is the host's: never shown.
    let dome = standard::dome_light();
    assert!(!dome.param("texture").unwrap().is_shown(&dome.defaults()));
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

#[test]
fn engines_declare_types_and_apply_schemas_to_them() {
    let (wake, _) = counter();
    let mut d = RecordingEngine::new(wake).description().as_ref().clone();
    assert!(d.prim_type(standard::AREA_LIGHT).is_some());
    assert!(d.prim_type("kinogaki.other.rim_light").is_none());
    let gobo = AppliedSchema {
        id: Arc::from("kinogaki.test.gobo"),
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
        Prim::Declared(standard::Fog::default().declared()).kind(),
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
        .push((id, Prim::Declared(standard::Fog::default().declared())));
    let thick = standard::Fog {
        density: 2.0,
        ..standard::Fog::default()
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
    engine: &'static str,
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
            self.engine,
            ShaderNode::new("flat").with("color", Constant::Color(*c)),
        )
    }
    fn default_material(&self) -> MaterialNetwork {
        MaterialNetwork {
            engine: Arc::from(self.engine),
            ..MaterialNetwork::default()
        }
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
    let flat_of = |engine: &'static str| MaterialNetwork {
        engine: Arc::from(engine),
        ..MaterialNetwork::default()
    };
    let mut here: EngineDescription = RecordingEngine::new(Arc::new(|| {}))
        .description()
        .as_ref()
        .clone();
    here.id = Arc::from("here");
    here.materials = Arc::new(Rules {
        engine: "here",
        tint: None,
    });
    let mut there = here.clone();
    there.id = Arc::from("there");
    there.materials = Arc::new(Rules {
        engine: "there",
        tint: Some(Tint::Color([0.1, 0.2, 0.3])),
    });
    let own = flat_of("here");
    assert_eq!(here.adopt(own.clone(), &there), own, "its own: as it is");
    let adopted = here.adopt(flat_of("there"), &there);
    assert_eq!(&*adopted.engine, "here");
    assert_eq!(
        adopted.nodes[0].value("color"),
        Some(Constant::Color([0.1, 0.2, 0.3]))
    );
    // No one tint: the default material.
    there.materials = Arc::new(Rules {
        engine: "there",
        tint: None,
    });
    assert_eq!(
        here.adopt(flat_of("there"), &there),
        here.default_material()
    );
    let engines = Engines::new(vec![here.clone(), there]);
    assert_eq!(&*engines.default_engine().id, "here");
    assert!(engines.get("there").is_some() && engines.get("nope").is_none());
}

#[test]
fn tints_say_their_color_and_attribute() {
    let cd: Arc<str> = Arc::from("Cd");
    let modulated = Tint::Modulated {
        color: [0.2, 0.4, 0.1],
        attribute: cd.clone(),
    };
    assert_eq!(modulated.color(), Some([0.2, 0.4, 0.1]));
    assert_eq!(modulated.attribute(), Some(&cd));
    assert_eq!(Tint::Attribute(cd.clone()).color(), None);
    assert_eq!(Tint::Attribute(cd.clone()).attribute(), Some(&cd));
    assert_eq!(Tint::Color([1.0; 3]).color(), Some([1.0; 3]));
    assert_eq!(Tint::Color([1.0; 3]).attribute(), None);
}

#[test]
fn outputs_have_one_id_each_and_engines_list_theirs() {
    use aov::standard;
    let mut all = standard::all();
    all.push(standard::cryptomatte(CryptomatteOf::Object, 6));
    all.push(standard::cryptomatte(CryptomatteOf::Material, 6));
    let mut ids: Vec<_> = all.iter().map(|a| a.id.clone()).collect();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), all.len());
    let get = |id| standard::get(id).unwrap();
    assert_eq!(get(standard::BEAUTY).channels(), 4);
    assert_eq!(get(standard::DIRECT).channels(), 3);
    assert_eq!(get(standard::OCCLUSION).channels(), 1);
    assert_eq!(
        standard::cryptomatte(CryptomatteOf::Object, 6).channels(),
        12
    );
    assert!(get(standard::INDIRECT).is_color() && get(standard::INDIRECT).channels() == 3);
    assert!(get(standard::VOLUME).is_color() && get(standard::VOLUME).channels() == 4);
    assert!(get(standard::AMBIENT).is_color() && !get(standard::OCCLUSION).is_color());
    assert!(get(standard::OBJECT_ID).is_id() && get(standard::BEAUTY).is_beauty());
    assert_eq!(
        standard::get(standard::CRYPTO_OBJECT),
        None,
        "levels are an engine's"
    );
    let (wake, _) = counter();
    let engine = RecordingEngine::new(wake);
    let d = engine.description();
    assert_eq!(d.aov("beauty"), Some(&standard::beauty()));
    assert_eq!(d.aov("direct"), None);
    assert_eq!(d.default_aovs(), vec![standard::beauty()]);
}
