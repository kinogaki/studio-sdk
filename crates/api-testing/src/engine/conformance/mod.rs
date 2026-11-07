//! An engine checked against what a host sends ([`conformance`]): scenes of
//! prims built with the SDK's builders, one for each prim kind and standard
//! type the engine declares (`scenes`), each rendered as a final frame and
//! read back beside the same scene without its subject; scenes that render
//! as their twins (`twins`): a glTF cut-out through its factor and vertex
//! colors, a float image's first channel, instances' own colors; then what a
//! host does over time (`lifecycle`): an empty scene, a camera not in the
//! scene, an edit after the first frame, a view id given again, a scene of
//! 6.4 million triangles and a material's nodes adoption cut.
//!
//! A scene passes when the engine renders it without failing, its final
//! frame is an Image of the view's size whose main layer is the beauty (RGBA
//! colors), its beauty is finite, and its subject shows (the two beauties
//! differ). Twins render alike. A view that can't render fails its read at
//! once and says why, never leaving it unanswered; an edit and a view given
//! again render as a fresh engine does; what an engine doesn't render it says
//! ([`RenderEngine::said`]).
use super::Waiter;
use crate::{Outcome, Report};
use kinogaki_render_api::aov::standard as aovs;
use kinogaki_render_api::display::{Display, Made};
use kinogaki_render_api::geometry::image::Image;
use kinogaki_render_api::geometry::{Diagnostic, Error};
use kinogaki_render_api::settings::defaults;
use kinogaki_render_api::{
    AovDesc, AovFormat, EngineDescription, Prim, PrimId, RenderEngine, SceneSettings, SceneUpdate,
    ViewCamera, ViewDesc, ViewId, ViewMode, Wake,
};
use std::sync::Arc;
use std::time::Duration;

mod lifecycle;
mod scenes;
mod twins;

/// The film's side, in pixels.
const SIDE: u32 = 48;

/// The view rendered.
const VIEW: ViewId = ViewId(1);

/// How long a final frame may take.
const LIMIT: Duration = Duration::from_secs(120);

/// What makes the engine under test, waking the given wake once per result.
pub type Make<'a> = &'a dyn Fn(Wake) -> Result<Made, Error>;

/// Engine `name`, made by `make`, rendering every scene its description
/// declares the prims of, then what a host does over time (see the module
/// docs): one outcome a check.
pub fn conformance(name: &str, make: Make<'_>) -> Report {
    let outcome = |what: &str, result| Outcome {
        producer: "prims".into(),
        fixture: what.into(),
        consumer: name.into(),
        result,
    };
    let description = match Session::new(make) {
        Err(why) => {
            let outcomes = vec![outcome("making the engine", Err(why))];
            return Report { outcomes };
        }
        Ok(session) => session.description,
    };
    let scenes = (scenes::scenes(&description).into_iter())
        .map(|scene| outcome(&scene.name, scenes::check(make, &scene)));
    let checks = [twins::checks, lifecycle::checks];
    let more = (checks.iter())
        .flat_map(|checks| checks(make, &description))
        .map(|(what, result)| outcome(what, result));
    Report {
        outcomes: scenes.chain(more).collect(),
    }
}

/// An engine made for a check, its display kept, and its wakes.
struct Session {
    engine: Box<dyn RenderEngine>,
    _display: Box<dyn Display>,
    description: Arc<EngineDescription>,
    waiter: Waiter,
}

impl Session {
    /// The engine `make` makes.
    fn new(make: Make<'_>) -> Result<Session, String> {
        let waiter = Waiter::default();
        let (engine, display) = make(waiter.wake()).map_err(|e| e.to_string())?;
        Ok(Session {
            description: engine.description().clone(),
            engine,
            _display: display,
            waiter,
        })
    }

    /// `prims` added, their ids in order from 0 (so a prim names those
    /// before it), with the scene's settings.
    fn send(&mut self, prims: Vec<Prim>) {
        let added = (prims.into_iter().enumerate())
            .map(|(i, prim)| (id(i), prim))
            .collect();
        self.engine.update(SceneUpdate {
            added,
            settings: Some(SceneSettings::default()),
            ..SceneUpdate::default()
        });
    }

    /// The beauty of a final of view [`VIEW`] at `size` through `camera`, at
    /// the engine's defaults, as one Image; or why there is none.
    fn render(&mut self, camera: ViewCamera, [width, height]: [u32; 2]) -> Result<Image, Error> {
        let desc = ViewDesc {
            camera,
            width,
            height,
            mode: ViewMode::Final,
            values: Arc::new(defaults(self.description.settings())),
        };
        self.engine.view(VIEW, desc);
        let asked = [AovDesc {
            aov: aovs::beauty(),
            format: AovFormat::F32,
        }];
        let pending = self.engine.finish(VIEW, &asked);
        let mut outcome = None;
        self.waiter.until(LIMIT, || {
            outcome = pending.take();
            outcome.is_some()
        });
        outcome.expect("taken above")
    }
}

/// The prim id of the `i`th prim of a scene.
fn id(i: usize) -> PrimId {
    PrimId {
        slot: i as u32,
        generation: 0,
    }
}

/// The beauty of `prims` through the conformance camera on an engine made
/// for them: an error when the engine fails or it isn't finite.
fn beauty(make: Make<'_>, prims: Vec<Prim>) -> Result<Arc<[f32]>, String> {
    rendered(make, prims).map(|(beauty, _)| beauty)
}

/// [`beauty`], and what the engine said of the scene.
fn rendered(make: Make<'_>, prims: Vec<Prim>) -> Result<(Arc<[f32]>, Vec<Diagnostic>), String> {
    let mut session = Session::new(make)?;
    session.send(prims);
    let camera = ViewCamera::Free(scenes::camera());
    let outputs = session
        .render(camera, [SIDE; 2])
        .map_err(|e| e.to_string())?;
    if let Some(why) = session.engine.problem(VIEW) {
        return Err(why.message);
    }
    let beauty = finite_beauty(&outputs, [SIDE; 2])?;
    Ok((beauty, session.engine.said()))
}

/// Whether beauty `a` agrees with `b`: at most one pixel in a hundred
/// differs by more than a hundredth (sampling noise apart).
fn alike(a: &[f32], b: &[f32]) -> Result<(), String> {
    let differs = |(x, y): (&f32, &f32)| (x - y).abs() > 1e-2 * x.abs().max(y.abs()).max(1.0);
    let pixels = a.len() / 4;
    let off = (a.chunks(4).zip(b.chunks(4)))
        .filter(|(p, q)| p.iter().zip(q.iter()).any(differs))
        .count();
    match off * 100 <= pixels {
        true => Ok(()),
        false => Err(format!("{off} of {pixels} pixels differ from its twin's")),
    }
}

/// The beauty of `outputs`, when they are an Image of `size` whose main
/// layer is the beauty (RGBA colors) and it is finite.
fn finite_beauty(outputs: &Image, size: [u32; 2]) -> Result<Arc<[f32]>, String> {
    if outputs.data().size() != size {
        return Err(format!(
            "a frame of {:?}, not the view's {size:?}",
            outputs.data().size()
        ));
    }
    let main = outputs.layers().first().map(|l| l.format());
    if !main
        .is_some_and(|f| f.name.is_empty() && f.channels.len() == 4 && f.encoding.space().is_some())
    {
        return Err("its main layer isn't the beauty".into());
    }
    let [width, height] = size.map(|s| s as i32);
    let beauty: Arc<[f32]> = (0..width * height)
        .flat_map(|p| outputs.read_pixel(0, p % width, p / width))
        .collect();
    match beauty.iter().all(|v| v.is_finite()) {
        true => Ok(beauty),
        false => Err("the beauty isn't finite".into()),
    }
}
