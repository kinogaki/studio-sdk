//! A package's UI: its contexts, session slots and settings sections, and
//! how the host keeps its state ([`UiPackage`], [`register`]).
//!
//! **State lives with the host.** A package's UI state (`UiPackage::State`:
//! live tools, caches, its workers' handles) is made once
//! ([`UiPackage::state`]) and kept by the host. For each call the host
//! takes it out, lends it to the part it calls with the call's context,
//! and puts it back; host methods never call back into a package (they
//! queue what would), so a package never meets its state missing. Undo
//! snapshots never hold it: what undo restores is the document and the
//! session's [slots](kinogaki_studio_api::Slot).
//!
//! **A context** ([`ContextPart`]) is an id and a set of optional parts:
//! the selection [`Levels`], the [`ViewportLayer`], the [`Inspector`], the
//! [`Vocabulary`], the [`NetworkView`], [`ObjectRows`] and [`OutlineRows`]
//! for the object kinds it owns, and the [`Creations`] it adds to an
//! object level's Create. A part a context leaves out is nothing there,
//! never another context's.
//!
//! **Registering** ([`register`]) erases the state's type: the host holds
//! every package as a [`Registered`], whose parts take the state as
//! `dyn Any` and downcast it back.
mod data;
mod parts;
mod settings;

pub use data::{
    BarField, BarValue, ContextObject, ContextView, Creation, FlagSide, FlagSpec, LevelSpec,
    NetworkNode, NetworkWire, NumericBar, OutlineRow, Start, Status, TimeDomain, TimeMark,
    TimeUnits, ToolbarItem, Tools, VocabularyItem,
};
pub use parts::{
    Creations, Erased, Inspector, Levels, NetworkView, ObjectRows, OutlineRows, ViewportLayer,
    Vocabulary,
};
pub use settings::{Section, SettingsSection};

use crate::host::{Cx, Waker};
use kinogaki_studio_api::{ContextId, PackageId, SessionSlot};
use std::any::Any;
use std::fmt;
use std::sync::Arc;

/// A package's UI, as the package writes it.
///
/// The host calls every method on its UI thread. `state` once per host
/// (one editor window), when the package is first needed; `poll` after
/// each wake of the host (a worker's result may be ready: adopt it);
/// `reset` when a new document replaces the open one (live tools,
/// workers' results and caches belong to the old one).
pub trait UiPackage: Send + Sync + 'static {
    /// The package's UI state.
    type State: Any;

    /// Its id (`acme.trees`).
    fn id(&self) -> PackageId;

    /// What the UI calls it ("Trees").
    fn label(&self) -> &str;

    /// A new host's state; `wake` is for its workers.
    fn state(&self, wake: Waker) -> Self::State;

    /// Its contexts.
    fn contexts(&self) -> Vec<ContextPart<Self::State>>;

    /// Its session slots.
    fn slots(&self) -> Vec<SessionSlot> {
        Vec::new()
    }

    /// Its settings sections (one per context that has user settings).
    fn settings(&self) -> Vec<SettingsSection> {
        Vec::new()
    }

    /// The host woke: adopt what the package's workers finished.
    fn poll(&self, s: &mut Self::State, cx: &mut dyn Cx) {
        let _ = (s, cx);
    }

    /// A new document: forget what belonged to the old one.
    fn reset(&self, s: &mut Self::State, cx: &mut dyn Cx) {
        let _ = (s, cx);
    }
}

/// One context of a package: its id and its parts.
pub struct ContextPart<S: ?Sized> {
    /// Its id (`acme.trees.forest`).
    pub id: ContextId,
    /// Whether it is its context's mode on one of the network's nodes (CAD's
    /// sketch mode on a Sketch node) rather than the network itself.
    pub mode: bool,
    /// Its selection levels; `None`: whole objects.
    pub levels: Option<Box<dyn Levels<S>>>,
    /// What it adds to the viewport.
    pub viewport: Option<Box<dyn ViewportLayer<S>>>,
    /// Its inspector.
    pub inspector: Option<Box<dyn Inspector<S>>>,
    /// Its tools and commands.
    pub vocabulary: Option<Box<dyn Vocabulary<S>>>,
    /// Its network, as the network view draws it.
    pub network: Option<Box<dyn NetworkView<S>>>,
    /// Inspector rows of the object kinds it owns.
    pub objects: Option<Box<dyn ObjectRows<S>>>,
    /// Outliner rows inside the object kinds it owns.
    pub outline: Option<Box<dyn OutlineRows<S>>>,
    /// What it adds to an object level's Create.
    pub creations: Option<Box<dyn Creations<S>>>,
}

impl<S: ?Sized> ContextPart<S> {
    /// Context `id`, with no parts yet.
    pub fn new(id: ContextId) -> Self {
        ContextPart {
            id,
            mode: false,
            levels: None,
            viewport: None,
            inspector: None,
            vocabulary: None,
            network: None,
            objects: None,
            outline: None,
            creations: None,
        }
    }

    /// As its context's mode on a node.
    pub fn as_mode(mut self) -> Self {
        self.mode = true;
        self
    }

    /// With selection levels.
    pub fn with_levels(mut self, part: impl Levels<S> + 'static) -> Self {
        self.levels = Some(Box::new(part));
        self
    }

    /// With a viewport layer.
    pub fn with_viewport(mut self, part: impl ViewportLayer<S> + 'static) -> Self {
        self.viewport = Some(Box::new(part));
        self
    }

    /// With an inspector.
    pub fn with_inspector(mut self, part: impl Inspector<S> + 'static) -> Self {
        self.inspector = Some(Box::new(part));
        self
    }

    /// With a vocabulary.
    pub fn with_vocabulary(mut self, part: impl Vocabulary<S> + 'static) -> Self {
        self.vocabulary = Some(Box::new(part));
        self
    }

    /// With a network view.
    pub fn with_network(mut self, part: impl NetworkView<S> + 'static) -> Self {
        self.network = Some(Box::new(part));
        self
    }

    /// With object rows.
    pub fn with_objects(mut self, part: impl ObjectRows<S> + 'static) -> Self {
        self.objects = Some(Box::new(part));
        self
    }

    /// With outliner rows.
    pub fn with_outline(mut self, part: impl OutlineRows<S> + 'static) -> Self {
        self.outline = Some(Box::new(part));
        self
    }

    /// With creations.
    pub fn with_creations(mut self, part: impl Creations<S> + 'static) -> Self {
        self.creations = Some(Box::new(part));
        self
    }
}

impl<S: 'static> ContextPart<S> {
    /// The same context over the host's erased state.
    fn erase(self) -> ContextPart<dyn Any> {
        ContextPart {
            id: self.id,
            mode: self.mode,
            levels: self
                .levels
                .map(|p| Box::new(Erased::<S, _>::new(p)) as Box<dyn Levels<dyn Any>>),
            viewport: self
                .viewport
                .map(|p| Box::new(Erased::<S, _>::new(p)) as Box<dyn ViewportLayer<dyn Any>>),
            inspector: self
                .inspector
                .map(|p| Box::new(Erased::<S, _>::new(p)) as Box<dyn Inspector<dyn Any>>),
            vocabulary: self
                .vocabulary
                .map(|p| Box::new(Erased::<S, _>::new(p)) as Box<dyn Vocabulary<dyn Any>>),
            network: self
                .network
                .map(|p| Box::new(Erased::<S, _>::new(p)) as Box<dyn NetworkView<dyn Any>>),
            objects: self
                .objects
                .map(|p| Box::new(Erased::<S, _>::new(p)) as Box<dyn ObjectRows<dyn Any>>),
            outline: self
                .outline
                .map(|p| Box::new(Erased::<S, _>::new(p)) as Box<dyn OutlineRows<dyn Any>>),
            creations: self
                .creations
                .map(|p| Box::new(Erased::<S, _>::new(p)) as Box<dyn Creations<dyn Any>>),
        }
    }
}

impl<S: ?Sized> fmt::Debug for ContextPart<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let has = |b: bool| if b { "yes" } else { "no" };
        f.debug_struct("ContextPart")
            .field("id", &self.id)
            .field("mode", &self.mode)
            .field("levels", &has(self.levels.is_some()))
            .field("viewport", &has(self.viewport.is_some()))
            .field("inspector", &has(self.inspector.is_some()))
            .field("vocabulary", &has(self.vocabulary.is_some()))
            .field("network", &has(self.network.is_some()))
            .field("objects", &has(self.objects.is_some()))
            .field("outline", &has(self.outline.is_some()))
            .field("creations", &has(self.creations.is_some()))
            .finish()
    }
}

/// The host's state of one package (`UiPackage::State`, erased).
pub type State = Box<dyn Any>;

/// A registered package, its state's type erased: what the host keeps.
pub struct Registered {
    /// Its id.
    pub id: PackageId,
    /// What the UI calls it.
    pub label: String,
    /// Its contexts, over the erased state.
    pub contexts: Vec<ContextPart<dyn Any>>,
    /// Its session slots.
    pub slots: Vec<SessionSlot>,
    /// Its settings sections.
    pub settings: Vec<SettingsSection>,
    package: Arc<dyn Calls>,
}

/// The package-wide calls of a registered package, over the erased state.
trait Calls: Send + Sync {
    fn state(&self, wake: Waker) -> State;
    fn poll(&self, s: &mut dyn Any, cx: &mut dyn Cx);
    fn reset(&self, s: &mut dyn Any, cx: &mut dyn Cx);
}

impl<P: UiPackage> Calls for P {
    fn state(&self, wake: Waker) -> State {
        Box::new(UiPackage::state(self, wake))
    }
    fn poll(&self, s: &mut dyn Any, cx: &mut dyn Cx) {
        UiPackage::poll(self, own(s), cx)
    }
    fn reset(&self, s: &mut dyn Any, cx: &mut dyn Cx) {
        UiPackage::reset(self, own(s), cx)
    }
}

fn own<S: 'static>(s: &mut dyn Any) -> &mut S {
    s.downcast_mut()
        .expect("the host lends each package its own state")
}

impl Registered {
    /// A new host's state of the package.
    pub fn state(&self, wake: Waker) -> State {
        self.package.state(wake)
    }

    /// [`UiPackage::poll`] with the package's `state`.
    pub fn poll(&self, state: &mut dyn Any, cx: &mut dyn Cx) {
        self.package.poll(state, cx)
    }

    /// [`UiPackage::reset`] with the package's `state`.
    pub fn reset(&self, state: &mut dyn Any, cx: &mut dyn Cx) {
        self.package.reset(state, cx)
    }

    /// Context `id`'s part (its mode when `mode`).
    pub fn context(&self, id: ContextId, mode: bool) -> Option<&ContextPart<dyn Any>> {
        self.contexts.iter().find(|c| c.id == id && c.mode == mode)
    }
}

impl fmt::Debug for Registered {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Registered")
            .field("id", &self.id)
            .field("label", &self.label)
            .field("contexts", &self.contexts)
            .field("slots", &self.slots)
            .field("settings", &self.settings)
            .finish()
    }
}

/// `package` as the host keeps it: its contexts read once and its state's
/// type erased.
pub fn register<P: UiPackage>(package: P) -> Registered {
    Registered {
        id: package.id(),
        label: package.label().to_owned(),
        contexts: package
            .contexts()
            .into_iter()
            .map(ContextPart::erase)
            .collect(),
        slots: package.slots(),
        settings: package.settings(),
        package: Arc::new(package),
    }
}
