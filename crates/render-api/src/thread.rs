//! An engine's render thread, as every engine runs one: [`EngineThread`] is the
//! [`RenderEngine`] a host holds, a handle that never blocks. Every call sends
//! a [`Message`] to the thread and returns; the thread owns the engine's
//! renderer and scene and answers through what it shares with the handle
//! ([`Published`]: each view's newest frame and problem). Dropping the handle
//! stops the thread and joins it.
//!
//! - **Views.** A dropped view's frame and problem go on the host's thread at
//!   once, and the thread publishes for a view only while the host hasn't
//!   dropped it since it gave it ([`Published::publish`]): a view id given
//!   again never shows the old view's frame.
//! - **Reads.** A final render and a preview are both a [`Reading`] of a
//!   finished view: its outputs, or a preview's beauty as its picture.
//! - **Previews** render as a view of their own ([`PREVIEW`]): a request's
//!   scene replaces the last request's prims ([`PreviewScene::replacing`]),
//!   seen as the engine's `preview_view` says, read once its frame is in.
//! - An engine's own messages travel as [`Message::Engine`], and what its
//!   handle offers besides (statistics, a helper) is its `extra`.
use std::collections::BTreeMap;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::JoinHandle;

use crate::aov::standard as aovs;
use crate::geometry::{Diagnostic, Error};
use crate::{
    AovDesc, AovFormat, Deliver, EngineDescription, FinalOutputs, Frame, Pending, PreviewImage,
    PreviewRequest, PreviewScene, PrimId, RenderEngine, SceneUpdate, ViewDesc, ViewId, Wake,
    pending,
};

/// The preview's view, apart from a host's.
pub const PREVIEW: ViewId = ViewId(u32::MAX);

/// What a finished view is read for: a final render's outputs, or a
/// preview's picture.
pub struct Reading(Read);

/// What a [`Reading`] is.
enum Read {
    /// A final render's outputs.
    Outputs(Vec<AovDesc>, Deliver<FinalOutputs>),
    /// A preview's beauty, as its picture.
    Preview(Deliver<PreviewImage>),
}

impl Reading {
    /// The outputs to read: a preview's beauty as floats.
    pub fn outputs(&self) -> Vec<AovDesc> {
        match &self.0 {
            Read::Outputs(outputs, _) => outputs.clone(),
            Read::Preview(_) => vec![AovDesc {
                aov: aovs::beauty(),
                format: AovFormat::F32,
            }],
        }
    }

    /// Whether someone still waits for it.
    pub fn wanted(&self) -> bool {
        match &self.0 {
            Read::Outputs(_, deliver) => deliver.wanted(),
            Read::Preview(deliver) => deliver.wanted(),
        }
    }

    /// Hands `outputs` over (a preview its first plane, the beauty).
    pub fn deliver(self, outputs: FinalOutputs) {
        match self.0 {
            Read::Outputs(_, deliver) => deliver.deliver(outputs),
            Read::Preview(deliver) => match outputs.planes.into_iter().next() {
                Some((_, pixels)) => deliver.deliver(PreviewImage {
                    side: outputs.width,
                    pixels,
                }),
                None => deliver.fail(Error::invalid("the preview has no beauty")),
            },
        }
    }

    /// Says why it can't be read.
    pub fn fail(self, why: Error) {
        match self.0 {
            Read::Outputs(_, deliver) => deliver.fail(why),
            Read::Preview(deliver) => deliver.fail(why),
        }
    }
}

impl PreviewScene {
    /// It replacing the prims `held` (the last request's): its update
    /// removes them, and `held` becomes what it adds.
    pub fn replacing(mut self, held: &mut Vec<PrimId>) -> PreviewScene {
        self.update.removed = std::mem::take(held);
        held.extend(self.update.added.iter().map(|(id, _)| *id));
        self
    }
}

/// What a host's call asks of the render thread.
pub enum Message<E> {
    /// [`RenderEngine::update`].
    Update(SceneUpdate),
    /// [`RenderEngine::view`].
    View(ViewId, ViewDesc),
    /// [`RenderEngine::drop_view`]: its frame and problem already went.
    Drop(ViewId),
    /// [`RenderEngine::pause`].
    Pause(ViewId, bool),
    /// [`RenderEngine::focus`].
    Focus(ViewId),
    /// Read the view once its frame of its newest desc is in.
    Read(ViewId, Reading),
    /// A host let go of a display image: a view waiting for one may take it.
    Freed,
    /// The engine's own.
    Engine(E),
    /// The handle is dropped: the thread ends.
    Stop,
}

/// What the handle, the thread and an engine's display share: each view's
/// newest frame and problem, and how often the host dropped each view.
#[derive(Default)]
pub struct Published {
    frames: BTreeMap<ViewId, Arc<Frame>>,
    problems: BTreeMap<ViewId, Diagnostic>,
    drops: BTreeMap<ViewId, u64>,
}

impl Published {
    /// How often the host dropped view `view` so far: what the thread notes
    /// when the host gives a view, and publishes under.
    pub fn drops(&self, view: ViewId) -> u64 {
        self.drops.get(&view).copied().unwrap_or(0)
    }

    /// View `view`'s newest frame.
    pub fn latest(&self, view: ViewId) -> Option<&Arc<Frame>> {
        self.frames.get(&view)
    }

    /// Why view `view` can't render.
    pub fn problem(&self, view: ViewId) -> Option<&Diagnostic> {
        self.problems.get(&view)
    }

    /// `frame` as its view's newest, its problem gone, when the host hasn't
    /// dropped the view since it gave it (`drops`); whether it was.
    pub fn publish(&mut self, frame: Arc<Frame>, drops: u64) -> bool {
        let view = frame.view;
        let current = self.drops(view) == drops;
        if current {
            self.problems.remove(&view);
            self.frames.insert(view, frame);
        }
        current
    }

    /// Why view `view` can't render, when the host hasn't dropped it since it
    /// gave it (`drops`); whether it was kept.
    pub fn fail(&mut self, view: ViewId, why: Diagnostic, drops: u64) -> bool {
        let current = self.drops(view) == drops;
        if current {
            self.problems.insert(view, why);
        }
        current
    }

    /// View `view`'s problem cleared: it renders again.
    pub fn clear_problem(&mut self, view: ViewId) {
        self.problems.remove(&view);
    }

    fn drop_view(&mut self, view: ViewId) {
        self.frames.remove(&view);
        self.problems.remove(&view);
        *self.drops.entry(view).or_default() += 1;
    }
}

/// `mutex` locked, whatever a panic left in it.
pub fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

/// The render thread's end of its mailbox.
pub struct Mailbox<E> {
    /// What the host asks, in order.
    pub receiver: Receiver<Message<E>>,
    /// The thread's own way back to itself (its completion callbacks').
    pub sender: Sender<Message<E>>,
    /// What it publishes.
    pub published: Arc<Mutex<Published>>,
}

/// An engine as a host holds it: a handle on its render thread (see the
/// module docs), with what the engine offers besides (`X`).
pub struct EngineThread<E: Send + 'static, X = ()> {
    description: Arc<EngineDescription>,
    sender: Sender<Message<E>>,
    published: Arc<Mutex<Published>>,
    wake: Wake,
    /// How the engine sees a preview's scene.
    preview_view: fn(&PreviewScene, u32) -> ViewDesc,
    /// The prims the last preview added.
    previewed: Vec<PrimId>,
    thread: Option<JoinHandle<()>>,
    extra: X,
}

impl<E: Send + 'static, X> EngineThread<E, X> {
    /// `description`'s engine running `run` on a thread named `name` until
    /// its mailbox says stop, its handle offering what `extra` makes of a
    /// sender to the thread; results wake `wake`, previews are seen as
    /// `preview_view` says.
    ///
    /// # Errors
    /// The thread can't start.
    pub fn start(
        name: &str,
        (description, extra): (EngineDescription, impl FnOnce(Sender<Message<E>>) -> X),
        wake: Wake,
        preview_view: fn(&PreviewScene, u32) -> ViewDesc,
        run: impl FnOnce(Mailbox<E>) + Send + 'static,
    ) -> std::io::Result<Self> {
        let (sender, receiver) = channel();
        let extra = extra(sender.clone());
        let published = Arc::new(Mutex::new(Published::default()));
        let mailbox = Mailbox {
            receiver,
            sender: sender.clone(),
            published: published.clone(),
        };
        let thread = std::thread::Builder::new()
            .name(name.into())
            .spawn(move || run(mailbox))?;
        Ok(Self {
            description: Arc::new(description),
            sender,
            published,
            wake,
            preview_view,
            previewed: Vec::new(),
            thread: Some(thread),
            extra,
        })
    }

    /// What the engine offers besides.
    pub fn extra(&self) -> &X {
        &self.extra
    }

    /// Sends `message` to the thread.
    pub fn send(&self, message: Message<E>) {
        // The thread outlives every sender but the handle's drop.
        let _ = self.sender.send(message);
    }

    /// A sender to the thread, for what the engine's display or a host's
    /// helper sends.
    pub fn sender(&self) -> Sender<Message<E>> {
        self.sender.clone()
    }

    /// What the thread publishes.
    pub fn published(&self) -> &Arc<Mutex<Published>> {
        &self.published
    }
}

impl<E: Send + 'static, X> Drop for EngineThread<E, X> {
    fn drop(&mut self) {
        self.send(Message::Stop);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl<E: Send + 'static, X: Send> RenderEngine for EngineThread<E, X> {
    fn description(&self) -> &Arc<EngineDescription> {
        &self.description
    }

    fn update(&mut self, update: SceneUpdate) {
        self.send(Message::Update(update));
    }

    fn view(&mut self, view: ViewId, desc: ViewDesc) {
        self.send(Message::View(view, desc));
    }

    fn drop_view(&mut self, view: ViewId) {
        lock(&self.published).drop_view(view);
        self.send(Message::Drop(view));
    }

    fn latest(&self, view: ViewId) -> Option<Arc<Frame>> {
        lock(&self.published).latest(view).cloned()
    }

    fn pause(&mut self, view: ViewId, paused: bool) {
        self.send(Message::Pause(view, paused));
    }

    fn focus(&mut self, view: ViewId) {
        self.send(Message::Focus(view));
    }

    fn problem(&self, view: ViewId) -> Option<Diagnostic> {
        lock(&self.published).problem(view).cloned()
    }

    fn finish(&mut self, view: ViewId, outputs: &[AovDesc]) -> Pending<FinalOutputs> {
        let (deliver, result) = pending(self.wake.clone());
        let reading = Reading(Read::Outputs(outputs.to_vec(), deliver));
        self.send(Message::Read(view, reading));
        result
    }

    fn preview(&mut self, request: PreviewRequest) -> Pending<PreviewImage> {
        let (deliver, result) = pending(self.wake.clone());
        let scene = request.scene().replacing(&mut self.previewed);
        let view = (self.preview_view)(&scene, request.side);
        self.send(Message::Update(scene.update));
        self.send(Message::View(PREVIEW, view));
        self.send(Message::Read(PREVIEW, Reading(Read::Preview(deliver))));
        result
    }
}
