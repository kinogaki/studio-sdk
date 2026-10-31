//! A package's UI: its contexts and settings sections, and how the host
//! keeps its state ([`UiPackage`], added to a package with
//! [`WithUi::with_ui`]).
//!
//! **One package.** The UI is part of the package it belongs to: the
//! package's id, label and session slots are its
//! [`Package`]'s, and the plugin exports that one package
//! ([`export_package!`](kinogaki_studio_api::export_package)):
//!
//! ```
//! use kinogaki_studio_api::{ContextId, Package, PackageId};
//! use studio_ui_api::host::Waker;
//! use studio_ui_api::package::{ContextPart, UiPackage, WithUi};
//!
//! struct TreesUi;
//! impl UiPackage for TreesUi {
//!     type State = ();
//!     fn state(&self, _wake: Waker) {}
//!     fn contexts(&self) -> Vec<ContextPart<()>> {
//!         vec![ContextPart::new(ContextId::declared("acme.trees.forest"))]
//!     }
//! }
//!
//! fn package() -> Package {
//!     Package::new(PackageId::declared("acme.trees"), "Trees").with_ui(TreesUi)
//! }
//! # let package = package();
//! # assert!(studio_ui_api::package::Registered::of(&package).is_some());
//! // kinogaki_studio_api::export_package!(package);
//! ```
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
//! **A context** ([`ContextPart`]) is an id, its [`Library`] (what it
//! offers the host's menus, keys, Tab and viewport chrome, as data) and a
//! set of optional parts: the selection [`Levels`], the [`ViewportLayer`],
//! the [`Inspector`], the [`Vocabulary`] that runs its items, the
//! [`ObjectRows`] and [`OutlineRows`] for the object
//! kinds it owns, the [`Creations`] it adds to an object level's Create
//! and [`Results`], how it takes the host's cooks of its objects. A part a context leaves out is nothing there, never another
//! context's.
//!
//! **Adding the UI** ([`WithUi::with_ui`]) erases the state's type: the
//! host reads every package's UI as a [`Registered`]
//! ([`Registered::of`]), whose parts take the state as `dyn Any` and
//! downcast it back.
mod data;
mod edited;
mod library;
mod parts;
mod results;
mod settings;
mod view;

pub use data::{
    BarField, BarValue, Candidate, ContextObject, ContextView, Creation, LevelPreset, LevelSpec,
    NumericBar, OutlineRow, Start, Status, TimeDomain, TimeMark, TimeUnits, ToolbarItem, Tools,
};
pub use edited::{Edited, Hover, SurfaceLook, TemplateMark};
pub use library::{
    AnalysisMode, BarMenu, BarSection, ContextMenuSpec, Held, KeyBinding, KeyStore, KeysActive,
    LevelItems, Library, MarkingDefault, MenuDefaults, MenuEntry, Places, REPEAT, Shared,
    SharedRows, Shelf, ShelfEntry, Suggested, TabSpec, ViewMenus, ViewportSpec, VocabularyItem,
    When,
};
pub use parts::{
    Creations, Erased, Inspector, Invoked, Levels, ObjectRows, OutlineRows, ViewportLayer,
    Vocabulary,
};
pub use results::{Results, Shown, ShownKind};
pub use settings::{Section, SettingsSection};
pub use view::{Holds, KeyPress, Popover, PopoverRow, RowEdit};

use crate::host::{Cx, Waker};
use kinogaki_studio_api::{ContextId, Package, PackageUi};
use std::any::Any;
use std::fmt;
use std::sync::Arc;

/// A package's UI, as the package writes it. Its id, label and session
/// slots are its [`Package`]'s.
///
/// The host calls every method on its UI thread. `state` once per host
/// (one editor window), when the package is first needed; `poll` after
/// each wake of the host (a worker's result may be ready: adopt it);
/// `reset` when a new document replaces the open one (live tools,
/// workers' results and caches belong to the old one).
pub trait UiPackage: Send + Sync + 'static {
    /// The package's UI state.
    type State: Any;

    /// A new host's state; `wake` is for its workers.
    fn state(&self, wake: Waker) -> Self::State;

    /// Its contexts.
    fn contexts(&self) -> Vec<ContextPart<Self::State>>;

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
    /// What it offers the host's menus, keys, Tab and viewport chrome.
    pub library: Library,
    /// Its selection levels; `None`: whole objects.
    pub levels: Option<Box<dyn Levels<S>>>,
    /// What it adds to the viewport.
    pub viewport: Option<Box<dyn ViewportLayer<S>>>,
    /// Its inspector.
    pub inspector: Option<Box<dyn Inspector<S>>>,
    /// Its tools and commands.
    pub vocabulary: Option<Box<dyn Vocabulary<S>>>,
    /// Inspector rows of the object kinds it owns.
    pub objects: Option<Box<dyn ObjectRows<S>>>,
    /// Outliner rows inside the object kinds it owns.
    pub outline: Option<Box<dyn OutlineRows<S>>>,
    /// What it adds to an object level's Create.
    pub creations: Option<Box<dyn Creations<S>>>,
    /// How it takes the host's cooks of its objects.
    pub results: Option<Box<dyn Results<S>>>,
}

impl<S: ?Sized> ContextPart<S> {
    /// Context `id`, with no parts yet.
    pub fn new(id: ContextId) -> Self {
        ContextPart {
            id,
            mode: false,
            library: Library::default(),
            levels: None,
            viewport: None,
            inspector: None,
            vocabulary: None,
            objects: None,
            outline: None,
            creations: None,
            results: None,
        }
    }

    /// As its context's mode on a node.
    pub fn as_mode(mut self) -> Self {
        self.mode = true;
        self
    }

    /// With its library.
    pub fn with_library(mut self, library: Library) -> Self {
        self.library = library;
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

    /// With how it takes the host's cooks.
    pub fn with_results(mut self, part: impl Results<S> + 'static) -> Self {
        self.results = Some(Box::new(part));
        self
    }
}

impl<S: 'static> ContextPart<S> {
    /// The same context over the host's erased state.
    fn erase(self) -> ContextPart<dyn Any> {
        ContextPart {
            id: self.id,
            mode: self.mode,
            library: self.library,
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
            objects: self
                .objects
                .map(|p| Box::new(Erased::<S, _>::new(p)) as Box<dyn ObjectRows<dyn Any>>),
            outline: self
                .outline
                .map(|p| Box::new(Erased::<S, _>::new(p)) as Box<dyn OutlineRows<dyn Any>>),
            creations: self
                .creations
                .map(|p| Box::new(Erased::<S, _>::new(p)) as Box<dyn Creations<dyn Any>>),
            results: self
                .results
                .map(|p| Box::new(Erased::<S, _>::new(p)) as Box<dyn Results<dyn Any>>),
        }
    }
}

impl<S: ?Sized> fmt::Debug for ContextPart<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let has = |b: bool| if b { "yes" } else { "no" };
        f.debug_struct("ContextPart")
            .field("id", &self.id)
            .field("mode", &self.mode)
            .field("items", &self.library.items.len())
            .field("levels", &has(self.levels.is_some()))
            .field("viewport", &has(self.viewport.is_some()))
            .field("inspector", &has(self.inspector.is_some()))
            .field("vocabulary", &has(self.vocabulary.is_some()))
            .field("objects", &has(self.objects.is_some()))
            .field("outline", &has(self.outline.is_some()))
            .field("creations", &has(self.creations.is_some()))
            .field("results", &has(self.results.is_some()))
            .finish()
    }
}

/// The host's state of one package (`UiPackage::State`, erased).
pub type State = Box<dyn Any>;

/// A package's UI, its state's type erased: what the host reads from the
/// package ([`Registered::of`]).
pub struct Registered {
    /// Its contexts, over the erased state.
    pub contexts: Vec<ContextPart<dyn Any>>,
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
    /// `package`'s UI; `None`: a headless package.
    pub fn of(package: &Package) -> Option<&Registered> {
        package.ui.as_ref()?.parts().downcast_ref()
    }

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
            .field("contexts", &self.contexts)
            .field("settings", &self.settings)
            .finish()
    }
}

/// A [`Package`] with its UI.
pub trait WithUi {
    /// With UI `ui`: its contexts read once and its state's type erased.
    /// The registry checks its contexts' ids like the package's others
    /// (under the package's vendor, each given parts by one package).
    fn with_ui(self, ui: impl UiPackage) -> Self;
}

impl WithUi for Package {
    fn with_ui(mut self, ui: impl UiPackage) -> Self {
        let registered = Registered {
            contexts: ui.contexts().into_iter().map(ContextPart::erase).collect(),
            settings: ui.settings(),
            package: Arc::new(ui),
        };
        let contexts = registered.contexts.iter().map(|c| (c.id, c.mode)).collect();
        self.ui = Some(PackageUi::new(contexts, registered));
        self
    }
}
