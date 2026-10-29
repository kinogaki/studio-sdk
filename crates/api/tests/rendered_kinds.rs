//! Object kinds that take part in a render through their facets, as a
//! render-domain package's do: a lamp renders as a spot of a type its
//! engine declares (and as nothing for an engine that declares none), a
//! render setup names its engine, size, camera and settings, and a kind
//! without either answers neither.
use kinogaki_render_api::{
    EngineDescription, RecordingEngine, RenderEngine, SettingValue, SettingValues, settings,
    standard,
};
use kinogaki_studio_api::element::{ElementReader, ElementWriter};
use kinogaki_studio_api::*;
use std::sync::Arc;

/// A lamp: a standard spot of `intensity`.
#[derive(Debug)]
struct Lamp {
    intensity: f64,
}
impl ObjectBody for Lamp {
    fn kind(&self) -> ObjectKindId {
        ObjectKindId::declared("toy.lights.lamp")
    }
}

/// A render setup for engine `engine`.
#[derive(Debug)]
struct Setup {
    engine: &'static str,
    camera: Option<ObjectRef>,
}
impl ObjectBody for Setup {
    fn kind(&self) -> ObjectKindId {
        ObjectKindId::declared("toy.lights.setup")
    }
}

/// Bodies of both kinds save nowhere (not what this tests).
struct Unsaved;
impl ObjectCodec for Unsaved {
    fn write(&self, _: &dyn ObjectBody, _: &mut dyn ElementWriter) -> Result<(), Error> {
        Err(Error::invalid("not saved"))
    }
    fn read(&self, _: &dyn ElementReader, _: &ReadCx<'_>) -> Result<Arc<dyn ObjectBody>, Error> {
        Err(Error::invalid("not saved"))
    }
}

/// A kind of `id`, its facets `rendered` and `setup`.
struct Kind {
    id: &'static str,
    rendered: Option<LampPrims>,
    setup: Option<Setups>,
}

struct LampPrims;
impl RenderedKind for LampPrims {
    fn declared(&self, body: &dyn ObjectBody) -> Vec<kinogaki_render_api::Declared> {
        let Some(lamp) = body.downcast_ref::<Lamp>() else {
            return Vec::new();
        };
        let mut values = standard::spot_light().defaults();
        settings::set_value(
            &mut values,
            "intensity",
            SettingValue::Float(lamp.intensity),
        );
        vec![kinogaki_render_api::Declared::new(
            standard::SPOT_LIGHT,
            values,
        )]
    }
    fn with_declared(
        &self,
        body: &dyn ObjectBody,
        declared: &[kinogaki_render_api::Declared],
    ) -> Option<Arc<dyn ObjectBody>> {
        body.downcast_ref::<Lamp>()?;
        let [spot] = declared else {
            return None;
        };
        let intensity = spot.value("intensity")?.as_float()?;
        Some(Arc::new(Lamp { intensity }))
    }
}

struct Setups;
impl RenderSetupKind for Setups {
    fn setup(&self, body: &dyn ObjectBody) -> Option<RenderSetup> {
        let setup = body.downcast_ref::<Setup>()?;
        Some(RenderSetup {
            engine: setup.engine.into(),
            resolution: [640, 360],
            camera: setup.camera,
            output: "render.exr".into(),
            outputs: Vec::new(),
            values: SettingValues::default(),
            processing: Processing::Network { source: None },
        })
    }
}

impl ObjectKindSpec for Kind {
    fn id(&self) -> ObjectKindId {
        ObjectKindId::declared(self.id)
    }
    fn label(&self) -> &str {
        self.id
    }
    fn icon(&self) -> &str {
        "light"
    }
    fn placed(&self) -> bool {
        true
    }
    fn context(&self) -> Option<ContextId> {
        None
    }
    fn result(&self) -> Option<DataTypeId> {
        None
    }
    fn new_body(&self) -> Arc<dyn ObjectBody> {
        Arc::new(Lamp { intensity: 1.0 })
    }
    fn codec(&self) -> &dyn ObjectCodec {
        &Unsaved
    }
    fn adopts(&self, _: &Data) -> Option<Arc<dyn ObjectBody>> {
        None
    }
    fn renders(&self) -> Option<&dyn RenderedKind> {
        self.rendered.as_ref().map(|r| r as &dyn RenderedKind)
    }
    fn render_setup(&self) -> Option<&dyn RenderSetupKind> {
        self.setup.as_ref().map(|s| s as &dyn RenderSetupKind)
    }
}

/// An engine declaring the standard spot, or nothing.
fn engine(spots: bool) -> EngineDescription {
    let mut e = RecordingEngine::new(Arc::new(|| {}))
        .description()
        .as_ref()
        .clone();
    e.types = if spots {
        vec![standard::spot_light()]
    } else {
        Vec::new()
    };
    e
}

#[test]
fn a_rendered_kind_says_what_its_objects_are_and_takes_edits() {
    let lamps = Kind {
        id: "toy.lights.lamp",
        rendered: Some(LampPrims),
        setup: None,
    };
    let kind: &dyn ObjectKindSpec = &lamps;
    let rendered = kind.renders().unwrap();
    let lamp = Lamp { intensity: 4.0 };
    let mut declared = rendered.declared(&lamp);
    assert_eq!(declared.len(), 1);
    assert_eq!(&*declared[0].type_id, standard::SPOT_LIGHT);
    assert_eq!(
        declared[0].value("intensity"),
        Some(&SettingValue::Float(4.0))
    );
    // An edit comes back as a new body.
    settings::set_value(
        &mut declared[0].values,
        "intensity",
        SettingValue::Float(9.0),
    );
    let edited = rendered.with_declared(&lamp, &declared).unwrap();
    assert_eq!(edited.downcast_ref::<Lamp>().unwrap().intensity, 9.0);
    // Two prims don't fit a lamp; another kind's body is none of its.
    let two = [declared[0].clone(), declared[0].clone()];
    assert!(rendered.with_declared(&lamp, &two).is_none());
    let setup = Setup {
        engine: "toy",
        camera: None,
    };
    assert!(rendered.with_declared(&setup, &declared).is_none());
    assert!(kind.render_setup().is_none() && !kind.at_root());
}

#[test]
fn retyping_a_prim_keeps_its_values_by_name() {
    let mut spot =
        kinogaki_render_api::Declared::new(standard::SPOT_LIGHT, standard::spot_light().defaults());
    settings::set_value(&mut spot.values, "intensity", SettingValue::Float(3.0));
    spot.retype(&standard::area_light());
    assert_eq!(&*spot.type_id, standard::AREA_LIGHT);
    assert_eq!(spot.value("intensity"), Some(&SettingValue::Float(3.0)));
    // The area light's own parameters take their defaults; the spot's stay.
    assert!(spot.value("width").is_some() && spot.value("coneAngle").is_some());
    spot.retype(&standard::spot_light());
    assert_eq!(spot.value("intensity"), Some(&SettingValue::Float(3.0)));
}

#[test]
fn a_setup_kind_says_which_engine_renders_and_how() {
    let setups = Kind {
        id: "toy.lights.setup",
        rendered: None,
        setup: Some(Setups),
    };
    let kind: &dyn ObjectKindSpec = &setups;
    let body = Setup {
        engine: "toy",
        camera: Some(ObjectRef(7)),
    };
    let setup = kind.render_setup().unwrap().setup(&body).unwrap();
    assert_eq!((&*setup.engine, setup.camera), ("toy", Some(ObjectRef(7))));
    // Another kind's body is none of its.
    assert_eq!(
        kind.render_setup().unwrap().setup(&Lamp { intensity: 1.0 }),
        None
    );
    assert!(kind.renders().is_none());
}

#[test]
fn a_kind_without_facets_answers_neither() {
    let plain = Kind {
        id: "toy.lights.plain",
        rendered: None,
        setup: None,
    };
    let kind: &dyn ObjectKindSpec = &plain;
    assert!(kind.renders().is_none() && kind.render_setup().is_none());
}

/// A lamp's codec: its type and intensity; a lamp of a type no installed
/// engine declares isn't installed.
struct Lamps;
impl ObjectCodec for Lamps {
    fn write(&self, body: &dyn ObjectBody, out: &mut dyn ElementWriter) -> Result<(), Error> {
        let lamp = body
            .downcast_ref::<Lamp>()
            .ok_or_else(|| Error::invalid("not a lamp"))?;
        out.set("type", element::Field::Text(standard::SPOT_LIGHT.into()))?;
        out.set("intensity", element::Field::Float(lamp.intensity))
    }
    fn read(
        &self,
        element: &dyn ElementReader,
        cx: &ReadCx<'_>,
    ) -> Result<Arc<dyn ObjectBody>, Error> {
        let ty = element.text("type")?;
        if cx.engines().iter().all(|e| e.prim_type(ty).is_none()) {
            return Err(Error::NotInstalled(format!(
                "light type {ty} isn't installed"
            )));
        }
        Ok(Arc::new(Lamp {
            intensity: element.float("intensity")?,
        }))
    }
}

#[test]
fn a_codec_reads_what_is_installed_and_says_what_isnt() {
    let mut element = element::MemoryElement::new("toy.lights.lamp");
    element
        .write(|out| Lamps.write(&Lamp { intensity: 2.5 }, out))
        .unwrap();
    let with = kinogaki_render_api::Engines::new(vec![engine(true)]);
    let back = element
        .read(|r| Lamps.read(r, &ReadCx::new(&with)))
        .unwrap();
    assert_eq!(back.downcast_ref::<Lamp>().unwrap().intensity, 2.5);
    for engines in [vec![engine(false)], Vec::new()] {
        let without = kinogaki_render_api::Engines::new(engines);
        let error = element
            .read(|r| Lamps.read(r, &ReadCx::new(&without)))
            .unwrap_err();
        assert_eq!(
            error,
            Error::NotInstalled(format!(
                "light type {} isn't installed",
                standard::SPOT_LIGHT
            ))
        );
    }
}
