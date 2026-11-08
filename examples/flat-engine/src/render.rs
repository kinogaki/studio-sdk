//! Flat's render thread, on the SDK's [`EngineThread`]: it keeps the prims
//! it was sent and each view's desc, and after the messages that arrived it
//! says the MaterialX nodes it doesn't render (once each), renders every
//! view they changed, publishes the frame (or why a view can't render),
//! wakes the host once, and answers the reads waiting for it.
use super::paint::Paint;
use kinogaki_render_api::geometry::{Diagnostic, Error, Vec3, Xform};
use kinogaki_render_api::thread::{EngineThread, Mailbox, Message, PREVIEW, Published, Reading};
use kinogaki_render_api::{
    Camera, PreviewScene, Prim, PrimId, Projection, RenderStats, SceneSettings, SceneUpdate,
    Topology, ViewCamera, ViewDesc, ViewId, ViewMode, ViewPass, Wake, aov, f32_to_half,
};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

/// Flat as a host holds it.
pub(crate) type Handle = EngineThread<()>;

/// Starts Flat's thread, waking `wake` once a published frame.
pub(crate) fn start(wake: Wake) -> std::io::Result<Handle> {
    let woken = wake.clone();
    let description = (super::description(), |_| ());
    EngineThread::start("flat", description, wake, preview_view, move |mailbox| {
        Scene::default().run(mailbox, &woken);
    })
}

/// A preview is seen through its request's camera.
fn preview_view(scene: &PreviewScene, side: u32) -> ViewDesc {
    ViewDesc {
        camera: ViewCamera::Prim(scene.camera),
        width: side,
        height: side,
        mode: ViewMode::Final,
        values: Arc::default(),
    }
}

/// One view: its desc, how often the host had dropped it when it gave it,
/// and its last render (or why it can't render).
struct View {
    desc: ViewDesc,
    drops: u64,
    generation: u64,
    pixels: Option<Result<Arc<[f32]>, String>>,
}

/// What the thread holds.
#[derive(Default)]
struct Scene {
    prims: BTreeMap<PrimId, Prim>,
    settings: SceneSettings,
    views: BTreeMap<ViewId, View>,
    /// The views to render again.
    stale: BTreeSet<ViewId>,
    /// Reads waiting for their view's render.
    reads: Vec<(ViewId, Reading)>,
    /// The (material, node) pairs said so far: each is said once.
    said: BTreeSet<(Arc<str>, Arc<str>)>,
}

impl Scene {
    fn run(mut self, mailbox: Mailbox<()>, wake: &Wake) {
        while let Ok(first) = mailbox.receiver.recv() {
            let rest = std::iter::from_fn(|| mailbox.receiver.try_recv().ok());
            for message in std::iter::once(first).chain(rest) {
                if !self.take(message) {
                    return;
                }
            }
            let mut woke = self.say_cuts(&mailbox.published);
            for id in std::mem::take(&mut self.stale) {
                let Some(view) = self.views.get_mut(&id) else {
                    continue;
                };
                let rendered = render(&self.prims, &self.settings, &view.desc);
                view.generation += 1;
                view.pixels = Some(rendered.clone());
                let mut published = lock(&mailbox.published);
                woke |= match rendered {
                    Ok(pixels) => {
                        let size = [view.desc.width, view.desc.height];
                        let halves: Vec<u16> = pixels.iter().map(|&v| f32_to_half(v)).collect();
                        match ViewPass::pixels(size, &halves) {
                            Ok(image) => {
                                let frame = ViewPass {
                                    view: id,
                                    generation: view.generation,
                                    desc: view.desc.clone(),
                                    image,
                                    stats: RenderStats {
                                        samples: 1,
                                        target: 1,
                                        ..RenderStats::default()
                                    },
                                };
                                published.publish(Arc::new(frame), view.drops)
                            }
                            Err(why) => {
                                published.fail(id, Diagnostic::error(why.to_string()), view.drops)
                            }
                        }
                    }
                    Err(why) => published.fail(id, Diagnostic::error(why), view.drops),
                };
            }
            if woke {
                wake();
            }
            self.answer();
        }
    }

    /// Says each MaterialX node adoption cut from a material (what Flat
    /// doesn't render) not said before; whether it said one.
    fn say_cuts(&mut self, published: &Mutex<Published>) -> bool {
        let materials = (self.prims.values()).filter_map(|prim| match prim {
            Prim::Material(m) => Some(m),
            _ => None,
        });
        let cut =
            materials.flat_map(|m| (m.network.cut().iter()).map(|n| (m.path.clone(), n.clone())));
        let new: Vec<_> = cut.filter(|pair| !self.said.contains(pair)).collect();
        for (material, node) in &new {
            let said = format!("{material}: Flat doesn't render `{node}`");
            lock(published).say(Diagnostic::warning(said));
        }
        self.said.extend(new.iter().cloned());
        !new.is_empty()
    }

    /// Takes `message`; `false` once the handle went.
    fn take(&mut self, message: Message<()>) -> bool {
        match message {
            Message::Update(update) => self.update(update),
            Message::View(id, desc, drops) => self.view(id, desc, drops),
            Message::Drop(id) => {
                self.views.remove(&id);
            }
            Message::Read(id, reading) => self.reads.push((id, reading)),
            Message::Preview(update, desc, reading) => {
                self.update(update);
                self.view(PREVIEW, desc, 0);
                self.reads.push((PREVIEW, reading));
            }
            Message::Stop => return false,
            Message::Pause(..) | Message::Focus(_) | Message::Freed | Message::Engine(()) => {}
        }
        true
    }

    /// Takes `update`: every view renders again.
    fn update(&mut self, update: SceneUpdate) {
        self.settings = update.settings.unwrap_or_else(|| self.settings.clone());
        for id in &update.removed {
            self.prims.remove(id);
        }
        self.prims.extend(update.added);
        self.prims
            .extend(update.changed.into_iter().map(|c| (c.id, c.prim)));
        self.stale.extend(self.views.keys());
    }

    /// View `id` as `desc`, given while the host had dropped it `drops` times.
    fn view(&mut self, id: ViewId, desc: ViewDesc, drops: u64) {
        let view = View {
            desc,
            drops,
            generation: 0,
            pixels: None,
        };
        self.views.insert(id, view);
        self.stale.insert(id);
    }

    /// Answers the reads whose view rendered; a read of a view that went,
    /// or can't render, fails saying why.
    fn answer(&mut self) {
        for (id, reading) in std::mem::take(&mut self.reads) {
            match self.views.get(&id).map(|v| (&v.desc, v.pixels.clone())) {
                None => reading.fail(Error::invalid("the view went")),
                Some((_, None)) => self.reads.push((id, reading)),
                Some((_, Some(Err(why)))) => reading.fail(Error::invalid(why)),
                Some((desc, Some(Ok(pixels)))) => {
                    let planes: Vec<_> = (reading.outputs().into_iter())
                        .filter(|o| o.aov.is_beauty())
                        .map(|o| (o, aov::Values::Floats(pixels.clone())))
                        .collect();
                    match aov::image([desc.width, desc.height], &planes, &[]) {
                        Ok(image) => reading.deliver(image),
                        Err(why) => reading.fail(why),
                    }
                }
            }
        }
    }
}

/// A triangle in the world: its corners, their texture coordinates and its
/// paint (an index into the world's paints).
struct Triangle {
    corners: [Vec3; 3],
    uv: [[f32; 2]; 3],
    paint: usize,
}

/// The world's triangles and the paints they name: every top-level
/// shape's, and each Instancer's prototypes' through its placements.
fn world(prims: &BTreeMap<PrimId, Prim>) -> (Vec<Triangle>, Vec<Paint>) {
    let mut paints = vec![Paint::of(prims, None)];
    let mut painted: BTreeMap<PrimId, usize> = BTreeMap::new();
    let named: BTreeSet<PrimId> = (prims.values())
        .filter_map(|p| match p {
            Prim::Instancer(i) => Some(i.prototypes().iter().copied()),
            _ => None,
        })
        .flatten()
        .collect();
    let mut triangles = Vec::new();
    let mut stack: Vec<(PrimId, Xform)> = (prims.keys())
        .filter(|id| !named.contains(id))
        .map(|&id| (id, Xform::IDENTITY))
        .collect();
    while let Some((id, above)) = stack.pop() {
        match prims.get(&id) {
            Some(Prim::Shape(shape)) if shape.visibility().visible => {
                let Topology::Triangles(corners) = shape.topology() else {
                    continue;
                };
                let at = above.compose(&shape.xform().at(0.0));
                let point =
                    |i: u32| at.apply_point(Vec3::from(shape.points()[i as usize].map(f64::from)));
                let uv =
                    shape.corners::<2>(&corners.to_vec(), kinogaki_render_api::geometry::names::UV);
                for (p, t) in corners.iter().enumerate() {
                    let material = shape.subsets().of(p).map(|m| m.prim());
                    let paint = match material {
                        None => 0,
                        Some(m) => *painted.entry(m).or_insert_with(|| {
                            paints.push(Paint::of(prims, Some(m)));
                            paints.len() - 1
                        }),
                    };
                    let uv = uv
                        .as_ref()
                        .map_or([[0.0; 2]; 3], |uv| std::array::from_fn(|k| uv[p * 3 + k]));
                    triangles.push(Triangle {
                        corners: t.map(point),
                        uv,
                        paint,
                    });
                }
            }
            Some(Prim::Instancer(inst)) if inst.visibility().visible => {
                let at = above.compose(&inst.xform().at(0.0));
                for (i, x) in inst.xforms().iter().enumerate() {
                    stack.push((inst.prototype_of(i), at.compose(x)));
                }
            }
            _ => {}
        }
    }
    (triangles, paints)
}

/// The view's image: RGBA floats, rows from the top, each pixel the
/// facing ratio of the nearest triangle its ray hits times its paint there
/// (black where none); why not, when its camera isn't in the scene.
fn render(
    prims: &BTreeMap<PrimId, Prim>,
    settings: &SceneSettings,
    desc: &ViewDesc,
) -> Result<Arc<[f32]>, String> {
    let camera = match &desc.camera {
        ViewCamera::Free(camera) => camera,
        ViewCamera::Prim(id) => match prims.get(&id.prim()) {
            Some(Prim::Camera(camera)) => camera,
            _ => return Err("the view's camera isn't in the scene".into()),
        },
    };
    let (triangles, paints) = world(prims);
    let (width, height) = (desc.width.max(1), desc.height.max(1));
    let mut pixels = Vec::with_capacity((width * height * 4) as usize);
    for y in 0..height {
        for x in 0..width {
            let film = [
                (f64::from(x) + 0.5) / f64::from(width) * 2.0 - 1.0,
                1.0 - (f64::from(y) + 0.5) / f64::from(height) * 2.0,
            ];
            let (from, toward) = ray(camera, settings, film);
            let hit = nearest(&triangles, from, toward);
            let color = hit.map_or([0.0; 3], |(shade, triangle, [u, v])| {
                let t = &triangles[triangle];
                let uv: [f32; 2] = std::array::from_fn(|k| {
                    (1.0 - u - v) * t.uv[0][k] + u * t.uv[1][k] + v * t.uv[2][k]
                });
                paints[t.paint].at(uv).map(|c| c * shade as f32)
            });
            pixels.extend([color[0], color[1], color[2], 1.0]);
        }
    }
    Ok(pixels.into())
}

/// The ray through film point `film` (-1 to 1 across each side) of
/// `camera`, which looks down its -Z, its film back in millimetres.
fn ray(camera: &Camera, settings: &SceneSettings, [u, v]: [f64; 2]) -> (Vec3, Vec3) {
    let at = camera.xform.at(0.0);
    let half = [camera.horizontal_aperture, camera.vertical_aperture].map(|a| f64::from(a) / 2.0);
    let (origin, direction) = match camera.projection {
        Projection::Orthographic => {
            let unit = settings.units.millimeters_per_unit();
            let offset = Vec3::new(u * half[0] / unit, v * half[1] / unit, 0.0);
            (offset, Vec3::new(0.0, 0.0, -1.0))
        }
        _ => {
            let focal = f64::from(camera.focal_length);
            (Vec3::ZERO, Vec3::new(u * half[0], v * half[1], -focal))
        }
    };
    let direction = at
        .apply_vector(direction)
        .normalized()
        .unwrap_or(Vec3::Z * -1.0);
    (at.apply_point(origin), direction)
}

/// The facing ratio at the nearest hit of the ray from `from` along
/// `toward` (Möller–Trumbore), the triangle hit and the hit's barycentric
/// coordinates, if it hits.
fn nearest(triangles: &[Triangle], from: Vec3, toward: Vec3) -> Option<(f64, usize, [f32; 2])> {
    let mut best: Option<(f64, f64, usize, [f32; 2])> = None;
    for (
        at,
        Triangle {
            corners: [a, b, c], ..
        },
    ) in triangles.iter().enumerate()
    {
        let (ab, ac) = (*b - *a, *c - *a);
        let p = toward.cross(ac);
        let det = ab.dot(p);
        if det.abs() < 1e-12 {
            continue;
        }
        let s = from - *a;
        let u = s.dot(p) / det;
        let q = s.cross(ab);
        let v = toward.dot(q) / det;
        let t = ac.dot(q) / det;
        if u < 0.0 || v < 0.0 || u + v > 1.0 || t <= 1e-9 {
            continue;
        }
        if best.is_none_or(|(near, ..)| t < near) {
            let facing = ab
                .cross(ac)
                .normalized()
                .map_or(0.0, |n| n.dot(toward).abs());
            best = Some((t, 0.1 + 0.8 * facing, at, [u as f32, v as f32]));
        }
    }
    best.map(|(_, shade, at, uv)| (shade, at, uv))
}

/// `mutex` locked, whatever a panic left in it.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}
