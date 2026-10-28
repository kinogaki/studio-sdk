//! An engine that renders nothing: it applies updates to a CPU-side store and
//! counts what it was sent, for hosts' tests (a drag must send ranges only,
//! an object move no triangles).

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::aov::{AovDesc, AovSpec, standard as aov};
use crate::description::{EngineDescription, MaterialRules, PreviewSetup, Tint};
use crate::dirty::Dirty;
use crate::engine::{Outputs, Pending, RenderEngine, ViewDesc, ViewId, Wake, pending};
use crate::exchange::PreviewSurface;
use crate::id::PrimId;
use crate::material::MaterialNetwork;
use crate::prim::{Prim, PrimKind};
use crate::settings::RenderSettings;
use crate::update::SceneUpdate;

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
    /// Triangles that arrived in added meshes or TOPOLOGY changes.
    pub triangles: u64,
    /// Points that arrived: whole meshes on add or TOPOLOGY, else the
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
    pub settings: RenderSettings,
    /// The views.
    pub views: BTreeMap<ViewId, (ViewDesc, bool)>,
    /// What it was sent.
    pub counts: Counts,
    wake: Wake,
}

impl RecordingEngine {
    /// An engine that wakes `wake` once per finished read.
    pub fn new(wake: Wake) -> RecordingEngine {
        RecordingEngine {
            description: Arc::new(description()),
            prims: BTreeMap::new(),
            settings: RenderSettings::default(),
            views: BTreeMap::new(),
            counts: Counts::default(),
            wake,
        }
    }

    fn count_mesh(&mut self, prim: &Prim) {
        if let Prim::Mesh(mesh) = prim {
            self.counts.triangles += mesh.triangles.len() as u64;
            self.counts.points += mesh.points.len() as u64;
        }
    }
}

/// The recording engine's materials: none of its own, so everything is
/// its empty material.
struct Rules;

fn empty() -> MaterialNetwork {
    MaterialNetwork {
        engine: Arc::from("recording"),
        ..MaterialNetwork::default()
    }
}

impl MaterialRules for Rules {
    fn tint(&self, _: &MaterialNetwork) -> Option<Tint> {
        None
    }
    fn volume(&self, _: &MaterialNetwork) -> bool {
        false
    }
    fn flat(&self, _: &Tint) -> MaterialNetwork {
        empty()
    }
    fn default_material(&self) -> MaterialNetwork {
        empty()
    }
    fn preview_surface(&self, _: &MaterialNetwork) -> PreviewSurface {
        PreviewSurface::default()
    }
    fn import_preview_surface(&self, _: &PreviewSurface) -> MaterialNetwork {
        empty()
    }
}

/// The recording engine's description: every structural prim, the
/// standard types, the beauty, no settings and no vocabulary of its own.
fn description() -> EngineDescription {
    let mut types = crate::standard::lights();
    types.push(crate::standard::fog());
    EngineDescription {
        id: Arc::from("recording"),
        label: Arc::from("Recording"),
        prims: PrimKind::STRUCTURAL.to_vec(),
        types,
        applied: Vec::new(),
        setting_groups: Vec::new(),
        settings: Vec::new(),
        material_nodes: Vec::new(),
        material_presets: Vec::new(),
        aovs: vec![AovSpec::new(aov::beauty(), true)],
        realtime: false,
        preview: PreviewSetup {
            settings: Vec::new(),
            floor: empty(),
        },
        materials: Arc::new(Rules),
    }
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
            self.count_mesh(&prim);
            self.prims.insert(id, prim);
        }
        for change in update.changed {
            self.counts.changed += 1;
            for bit in 0..Dirty::COUNT {
                if change.dirty.bits() & (1 << bit) != 0 {
                    self.counts.dirty[bit] += 1;
                }
            }
            if change.dirty.contains(Dirty::TOPOLOGY) {
                self.count_mesh(&change.prim);
            } else if change.dirty.contains(Dirty::POINTS) {
                self.counts.points += match &change.prim {
                    Prim::Mesh(mesh) if change.points.is_whole() => mesh.points.len() as u64,
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

    /// Delivers black outputs of the view's size at once.
    fn finish(&mut self, view: ViewId, outputs: &[AovDesc]) -> Pending<Outputs> {
        let (deliver, result) = pending(self.wake.clone());
        let (width, height) = self
            .views
            .get(&view)
            .map_or((0, 0), |(d, _)| (d.width, d.height));
        let pixels = width as usize * height as usize;
        let planes = outputs
            .iter()
            .map(|desc| {
                (
                    desc.clone(),
                    Arc::from(vec![0.0f32; pixels * desc.aov.channels()]),
                )
            })
            .collect();
        deliver.deliver(Outputs {
            width,
            height,
            block: 1,
            planes,
            names: Vec::new(),
        });
        result
    }
}
