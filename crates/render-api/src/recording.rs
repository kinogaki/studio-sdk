//! An engine that renders nothing: it applies updates to a CPU-side store and
//! counts what it was sent, for hosts' tests (a drag must send ranges only,
//! an object move no triangles).

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::aov::{AovDesc, AovSpec, standard as aov};
use crate::description::{Capabilities, EngineDescription, Motion};
use crate::dirty::Dirty;
use crate::engine::{Pending, RenderEngine, ViewDesc, ViewId, Wake, pending};
use crate::id::PrimId;
use crate::material::preview::{PreviewImage, PreviewRequest};
use crate::prim::{Prim, PrimKind, Topology};
use crate::settings::SceneSettings;
use crate::update::SceneUpdate;
use kinogaki_geometry::Diagnostic;
use kinogaki_geometry::image::Image;
use kinogaki_value::EngineId;

/// What a [`RecordingEngine`] was sent.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Counts {
    /// Batches.
    pub updates: u64,
    /// Prims added.
    pub added: u64,
    /// Prims removed.
    pub removed: u64,
    /// Changes, and how many had each dirty bit (by bit index).
    pub changed: u64,
    /// Per dirty bit, the changes that carried it.
    pub dirty: [u64; Dirty::COUNT],
    /// Triangles that arrived in added shapes or TOPOLOGY changes.
    pub triangles: u64,
    /// Points that arrived: whole shapes on add or TOPOLOGY, else the
    /// POINTS ranges' lengths (a whole-ranges change counts every point).
    pub points: u64,
    /// Settled batches.
    pub settled: u64,
}

/// A store of the scene as an engine would hold it, and counts of what came.
pub struct RecordingEngine {
    description: Arc<EngineDescription>,
    /// The prims, by id.
    pub prims: BTreeMap<PrimId, Prim>,
    /// The scene's settings.
    pub settings: SceneSettings,
    /// The views.
    pub views: BTreeMap<ViewId, (ViewDesc, bool)>,
    /// The view last named the one the user works with.
    pub focused: Option<ViewId>,
    /// What it was sent.
    pub counts: Counts,
    /// The previews it was asked for.
    pub previews: Vec<PreviewRequest>,
    wake: Wake,
}

impl RecordingEngine {
    /// An engine that wakes `wake` once per finished read.
    pub fn new(wake: Wake) -> RecordingEngine {
        RecordingEngine {
            description: Arc::new(description()),
            prims: BTreeMap::new(),
            settings: SceneSettings::default(),
            focused: None,
            views: BTreeMap::new(),
            counts: Counts::default(),
            previews: Vec::new(),
            wake,
        }
    }

    fn count_shape(&mut self, prim: &Prim) {
        if let Prim::Shape(shape) = prim {
            if let Topology::Triangles(triangles) = shape.topology() {
                self.counts.triangles += triangles.len() as u64;
            }
            self.counts.points += shape.points().len() as u64;
        }
    }
}

/// The recording engine's id.
const ID: EngineId = EngineId::declared("kinogaki.render.recording");

/// The recording engine's description: every structural prim, the
/// standard types, the beauty, no settings and no vocabulary of its own.
fn description() -> EngineDescription {
    let mut types = crate::standard::lights();
    types.push(crate::standard::fog());
    EngineDescription::builder(ID, "Recording")
        .capabilities(Capabilities {
            prims: PrimKind::STRUCTURAL.to_vec(),
            types,
            aovs: vec![AovSpec::new(aov::beauty(), true)],
            motion: Motion {
                transforms: true,
                velocities: true,
            },
            // It keeps no film.
            film_bytes: 0,
            ..Capabilities::default()
        })
        .build()
}

impl RenderEngine for RecordingEngine {
    fn description(&self) -> &Arc<EngineDescription> {
        &self.description
    }

    fn update(&mut self, update: SceneUpdate) {
        self.counts.updates += 1;
        self.counts.settled += u64::from(update.settled);
        for id in update.removed {
            self.counts.removed += 1;
            self.prims.remove(&id);
        }
        for (id, prim) in update.added {
            self.counts.added += 1;
            self.count_shape(&prim);
            self.prims.insert(id, prim);
        }
        for change in update.changed {
            self.counts.changed += 1;
            for bit in 0..Dirty::COUNT {
                if change.dirty.bits() & (1u16 << bit) != 0 {
                    self.counts.dirty[bit] += 1;
                }
            }
            if change.dirty.contains(Dirty::TOPOLOGY) {
                self.count_shape(&change.prim);
            } else if change.dirty.contains(Dirty::POINTS) {
                self.counts.points += match &change.prim {
                    Prim::Shape(shape) if change.points.is_whole() => shape.points().len() as u64,
                    _ => change.points.len(),
                };
            }
            self.prims.insert(change.id, change.prim);
        }
        if let Some(settings) = update.settings {
            self.settings = settings;
        }
    }

    fn view(&mut self, view: ViewId, desc: ViewDesc) {
        let paused = self.views.get(&view).is_some_and(|v| v.1);
        self.views.insert(view, (desc, paused));
    }

    fn drop_view(&mut self, view: ViewId) {
        self.views.remove(&view);
    }

    fn latest(&self, _view: ViewId) -> Option<Arc<crate::engine::Frame>> {
        None
    }

    fn pause(&mut self, view: ViewId, paused: bool) {
        if let Some(entry) = self.views.get_mut(&view) {
            entry.1 = paused;
        }
    }

    fn focus(&mut self, view: ViewId) {
        self.focused = Some(view);
    }

    fn problem(&self, _view: ViewId) -> Option<Diagnostic> {
        None
    }

    /// Delivers black outputs of the view's size at once (a view it wasn't
    /// given fails).
    fn finish(&mut self, view: ViewId, outputs: &[AovDesc]) -> Pending<Image> {
        let (deliver, result) = pending(self.wake.clone());
        let size = self.views.get(&view).map(|(d, _)| [d.width, d.height]);
        let pixels = size.map_or(0, |[w, h]| w as usize * h as usize);
        let planes: Vec<_> = (outputs.iter())
            .map(|desc| {
                (
                    desc.clone(),
                    vec![0.0f32; pixels * desc.aov.channels()].into(),
                )
            })
            .collect();
        let image = size.ok_or_else(|| kinogaki_geometry::Error::missing("no such view"));
        match image.and_then(|size| crate::aov::image(size, &planes, &[])) {
            Ok(image) => deliver.deliver(image),
            Err(why) => deliver.fail(why),
        }
        result
    }

    /// Delivers a black picture at once, keeping what it was asked.
    fn preview(&mut self, request: PreviewRequest) -> Pending<PreviewImage> {
        let (deliver, result) = pending(self.wake.clone());
        let side = request.side;
        self.previews.push(request);
        let pixels = side as usize * side as usize * 4;
        deliver.deliver(PreviewImage {
            side,
            pixels: Arc::from(vec![0.0f32; pixels]),
        });
        result
    }
}
