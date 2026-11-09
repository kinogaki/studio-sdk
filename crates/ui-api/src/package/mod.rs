//! A package's UI: its contexts and settings pages, and how the host
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
//! # assert!(studio_ui_api::package::hosting::Registered::of(&package).is_some());
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
//! set of optional parts: the selection [`Levels`]; in the viewport its
//! [`LiveTools`], [`Panels`], [`Transforms`] and [`Display`]; the [`Inspector`] and the [`Vocabulary`] that runs
//! its items (the creations of the object kinds it owns are the kinds' own,
//! `ObjectKindSpec::creations`). A part a
//! context leaves out is never another context's. In a context whose
//! networks are core's, the host's own UI is there without a part: its
//! network pane, Tab listing the context's listed node types, and its
//! node inspector; its menus are its library's alone. A part adds to them or replaces them: the
//! library's items come first and what it sets wins, the vocabulary runs
//! the library's items, an inspector adds under the node sheet. Elsewhere a
//! part left out is nothing there.
//!
//! **Adding the UI** ([`WithUi::with_ui`]) erases the state's type: the
//! host reads every package's UI as a [`hosting::Registered`], whose parts
//! take the state as `dyn Any` and downcast it back.
//!
//! **A frame**, for the open context's viewport parts, in order:
//! [`LiveTools::holds`]; then for each view [`Panels::panels`] (where its
//! panels are, which take the pointer from the view) and
//! [`LiveTools::tools`] before the camera's navigation, then core's gizmo,
//! handles and picks as the tools leave them ([`Takes`]); the frame's
//! commands; then once [`Display::settle`]; then for each drawn view
//! [`Display::overlay`], each panel's [`Panels::panel`], and
//! [`Display::edited`]. Keys come before all of it ([`LiveTools::key`]),
//! [`LiveTools::cancel`] puts a live gesture back ([`Ended`]) and
//! [`LiveTools::accept`] keeps it.
mod data;
mod display;
mod edited;
pub mod hosting;
mod library;
mod parts;
mod settings;
mod view;

pub use data::{
    Candidate, Ended, GizmoUse, LevelPreset, LevelSpec, LiveHandles, Panel, PanelScope, Takes,
};
pub use display::{DisplayKind, DisplayOption, DisplayScope};
pub use edited::{Edited, Ghost, GhostTone};
pub use library::{
    BarMenu, BarSection, ContextMenuSpec, Held, KeyBinding, KeyStore, KeysActive, LevelItems,
    Library, MarkingDefault, MenuDefaults, MenuEntry, ObjectMenus, Places, REPEAT, Shared, Shelf,
    ShelfEntry, Situation, Suggested, TabSpec, ViewMenus, ViewportSpec, VocabularyItem, When,
};
pub use parts::{
    Display, Inspector, Invoked, Levels, LiveTools, Panels, RowAction, Transforms, Vocabulary,
};
pub use settings::{PageRows, SettingsPage};
pub use view::{Holds, KeyPress};

use crate::host::{Cx, Waker};
use kinogaki_studio_api::menu::ItemHome;
use kinogaki_studio_api::{ContextId, Package, PackageUi};
use std::any::Any;
use std::fmt;

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

    /// Its pages of the Settings window, over its stored values.
    fn settings(&self) -> Vec<SettingsPage<Self::State>> {
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
    /// What it offers the host's menus, keys, Tab and viewport chrome.
    pub library: Library,
    /// Its selection levels; `None`: whole objects.
    pub levels: Option<Box<dyn Levels<S>>>,
    /// Its live tools' input over the views.
    pub tools: Option<Box<dyn LiveTools<S>>>,
    /// Its floating panels over the views.
    pub panels: Option<Box<dyn Panels<S>>>,
    /// The host's transform tools over its selection.
    pub transforms: Option<Box<dyn Transforms<S>>>,
    /// What it draws in the views.
    pub display: Option<Box<dyn Display<S>>>,
    /// Its inspector.
    pub inspector: Option<Box<dyn Inspector<S>>>,
    /// Its tools and commands.
    pub vocabulary: Option<Box<dyn Vocabulary<S>>>,
}

/// `with_*`: the context with part `$field` given.
macro_rules! with {
    ($($(#[$doc:meta])* $with:ident: $field:ident: $trait:ident),* $(,)?) => {
        $(
            $(#[$doc])*
            pub fn $with(mut self, part: impl $trait<S> + 'static) -> Self {
                self.$field = Some(Box::new(part));
                self
            }
        )*
    };
}

impl<S: ?Sized> ContextPart<S> {
    /// Context `id`, with no parts yet.
    pub fn new(id: ContextId) -> Self {
        ContextPart {
            id,
            library: Library::default(),
            levels: None,
            tools: None,
            panels: None,
            transforms: None,
            display: None,
            inspector: None,
            vocabulary: None,
        }
    }

    /// With its library.
    pub fn with_library(mut self, library: Library) -> Self {
        self.library = library;
        self
    }

    with! {
        /// With selection levels.
        with_levels: levels: Levels,
        /// With live tools.
        with_tools: tools: LiveTools,
        /// With panels.
        with_panels: panels: Panels,
        /// With the host's transform tools.
        with_transforms: transforms: Transforms,
        /// With what it draws in the views.
        with_display: display: Display,
        /// With an inspector.
        with_inspector: inspector: Inspector,
        /// With a vocabulary.
        with_vocabulary: vocabulary: Vocabulary,
    }
}

impl<S: ?Sized> fmt::Debug for ContextPart<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let has = |b: bool| if b { "yes" } else { "no" };
        f.debug_struct("ContextPart")
            .field("id", &self.id)
            .field("items", &self.library.items.len())
            .field("levels", &has(self.levels.is_some()))
            .field("tools", &has(self.tools.is_some()))
            .field("panels", &has(self.panels.is_some()))
            .field("transforms", &has(self.transforms.is_some()))
            .field("display", &has(self.display.is_some()))
            .field("inspector", &has(self.inspector.is_some()))
            .field("vocabulary", &has(self.vocabulary.is_some()))
            .finish()
    }
}

/// A [`Package`] with its UI.
pub trait WithUi {
    /// With UI `ui`: its contexts read once and its state's type erased.
    /// The registry checks its contexts' ids like the package's others
    /// (under the package's vendor, each given parts by one package) and
    /// its items' homes like its offerings'.
    fn with_ui(self, ui: impl UiPackage) -> Self;
}

impl WithUi for Package {
    fn with_ui(self, ui: impl UiPackage) -> Self {
        let registered = hosting::Registered::new(ui);
        let contexts = registered.contexts.iter().map(|c| c.id).collect();
        let items = (registered.contexts.iter())
            .flat_map(|c| {
                (c.library.items.iter()).map(|i| ItemHome {
                    context: c.id,
                    item: i.name.clone(),
                    home: i.home.clone(),
                    objects: i.places.objects,
                })
            })
            .collect();
        self.with_package_ui(PackageUi::new(contexts, items, registered))
    }
}
