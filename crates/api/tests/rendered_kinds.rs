//! Object kinds that are render prims through their facet, as an engine's
//! lights are: a lamp renders as a spot of a type its engine declares (and
//! as nothing for an engine that declares none), and a kind without the
//! facet answers none.
use kinogaki_render_api::{EngineDescription, RecordingEngine, RenderEngine, standard};
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

/// Another kind's body.
#[derive(Debug)]
struct Other;
impl ObjectBody for Other {
    fn kind(&self) -> ObjectKindId {
        ObjectKindId::declared("toy.lights.other")
    }
}

/// Bodies save nowhere (not what this tests).
struct Unsaved;
impl ObjectCodec for Unsaved {
    fn write(&self, _: &dyn ObjectBody, _: &mut dyn ElementWriter) -> Result<(), Error> {
        Err(Error::invalid("not saved"))
    }
    fn read(&self, _: &dyn ElementReader, _: &ReadCx<'_>) -> Result<Arc<dyn ObjectBody>, Error> {
        Err(Error::invalid("not saved"))
    }
}

/// A kind of `id`, its facet `rendered`.
struct Kind {
    id: &'static str,
    rendered: Option<LampPrims>,
}

struct LampPrims;
impl RenderedKind for LampPrims {
    fn declared(&self, body: &dyn ObjectBody) -> Vec<kinogaki_render_api::Declared> {
        let Some(lamp) = body.downcast_ref::<Lamp>() else {
            return Vec::new();
        };
        let mut values = area_light().defaults();
        values.insert("intensity".into(), Value::Number(lamp.intensity));
        vec![kinogaki_render_api::Declared::new(
            standard::AREA_LIGHT,
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
        let intensity = spot.value("intensity")?.number()?;
        Some(Arc::new(Lamp { intensity }))
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
    fn renders(&self) -> Option<&dyn RenderedKind> {
        self.rendered.as_ref().map(|r| r as &dyn RenderedKind)
    }
}

/// An engine declaring the standard spot, or nothing.
fn engine(spots: bool) -> EngineDescription {
    let e = RecordingEngine::new(Arc::new(|| {})).description().clone();
    let mut capabilities = e.capabilities().clone();
    capabilities.types = if spots {
        vec![area_light()]
    } else {
        Vec::new()
    };
    e.rebuilt().capabilities(capabilities).build()
}

#[test]
fn a_rendered_kind_says_what_its_objects_are_and_takes_edits() {
    let lamps = Kind {
        id: "toy.lights.lamp",
        rendered: Some(LampPrims),
    };
    let kind: &dyn ObjectKindSpec = &lamps;
    let rendered = kind.renders().unwrap();
    let lamp = Lamp { intensity: 4.0 };
    let mut declared = rendered.declared(&lamp);
    assert_eq!(declared.len(), 1);
    assert_eq!(&*declared[0].type_id, standard::AREA_LIGHT);
    assert_eq!(declared[0].value("intensity"), Some(&Value::Number(4.0)));
    // An edit comes back as a new body.
    declared[0]
        .values
        .insert("intensity".into(), Value::Number(9.0));
    let edited = rendered.with_declared(&lamp, &declared).unwrap();
    assert_eq!(edited.downcast_ref::<Lamp>().unwrap().intensity, 9.0);
    // Two prims don't fit a lamp; another kind's body is none of its.
    let two = [declared[0].clone(), declared[0].clone()];
    assert!(rendered.with_declared(&lamp, &two).is_none());
    assert!(rendered.with_declared(&Other, &declared).is_none());
    assert!(!kind.at_root());
}

#[test]
fn a_kind_without_the_facet_answers_none() {
    let plain = Kind {
        id: "toy.lights.plain",
        rendered: None,
    };
    let kind: &dyn ObjectKindSpec = &plain;
    assert!(kind.renders().is_none());
}

/// A lamp's codec: its type and intensity; a lamp of a type no installed
/// engine declares isn't installed.
struct Lamps;
impl ObjectCodec for Lamps {
    fn write(&self, body: &dyn ObjectBody, out: &mut dyn ElementWriter) -> Result<(), Error> {
        let lamp = body
            .downcast_ref::<Lamp>()
            .ok_or_else(|| Error::invalid("not a lamp"))?;
        out.set("type", element::Field::Text(standard::AREA_LIGHT.into()))?;
        out.set("intensity", element::Field::Float(lamp.intensity))
    }
    fn read(
        &self,
        element: &dyn ElementReader,
        cx: &ReadCx<'_>,
    ) -> Result<Arc<dyn ObjectBody>, Error> {
        let ty = element.text("type")?;
        if cx.engines().iter().all(|e| e.prim_type(ty).is_none()) {
            return Err(Error::missing(format!("light type {ty} isn't installed")));
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
            Error::missing(format!(
                "light type {} isn't installed",
                standard::AREA_LIGHT
            ))
        );
    }
}

/// The standard area light's prim type.
fn area_light() -> kinogaki_render_api::PrimType {
    let mut lights = standard::lights().into_iter();
    lights.find(|t| &*t.id == standard::AREA_LIGHT).unwrap()
}
