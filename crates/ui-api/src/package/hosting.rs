//! What the host reads of a package's UI, its state's type erased: a
//! [`Registered`] package ([`Registered::of`]) whose context parts take
//! the state the host keeps ([`State`]) as `dyn Any` and downcast it back.
//! A package's author never meets these: they are the host's (Studio's,
//! and the testing crate's `Hosting`).
use super::{ContextPart, PageRows, SettingsPage, UiPackage};
use super::{Display, Inspector, Levels, LiveTools, Materials, Panels};
use super::{Transforms, Vocabulary};
use crate::host::{Cx, Waker};
use kinogaki_studio_api::Package;
use std::any::Any;
use std::fmt;
use std::marker::PhantomData;
use std::sync::Arc;

/// The host's state of one package (`UiPackage::State`, erased).
pub type State = Box<dyn Any>;

/// A part over `S` as the same part over `dyn Any`: the host's state,
/// lent as `dyn Any`, is downcast back to `S` for each call (each part's
/// trait makes its own forwarding, `parts::part!`).
pub(crate) struct Erased<S, P: ?Sized> {
    pub(crate) part: Box<P>,
    state: PhantomData<fn() -> S>,
}

impl<S, P: ?Sized> Erased<S, P> {
    fn new(part: Box<P>) -> Self {
        Erased {
            part,
            state: PhantomData,
        }
    }
}

/// Each part of `$part`, erased.
macro_rules! erase {
    ($part:expr, $S:ty, { $($field:ident: $trait:ident),* $(,)? }) => {
        ContextPart {
            id: $part.id,
            library: $part.library,
            $($field: $part.$field.map(|p| {
                Box::new(Erased::<$S, dyn $trait<$S>>::new(p)) as Box<dyn $trait<dyn Any>>
            }),)*
        }
    };
}

impl<S: 'static> ContextPart<S> {
    /// The same context over the host's erased state.
    fn erase(self) -> ContextPart<dyn Any> {
        erase!(self, S, {levels: Levels, tools: LiveTools, panels: Panels, transforms: Transforms, display: Display, inspector: Inspector, vocabulary: Vocabulary, materials: Materials})
    }
}

impl<S: 'static> SettingsPage<S> {
    /// The same page over the host's erased state.
    fn erase(self) -> SettingsPage<dyn Any> {
        SettingsPage {
            scope: self.scope,
            label: self.label,
            rows: Box::new(Erased::<S, dyn PageRows<S>>::new(self.rows)),
        }
    }
}

/// A package's UI, its state's type erased: what the host reads from the
/// package ([`Registered::of`]).
pub struct Registered {
    /// Its contexts, over the erased state.
    pub contexts: Vec<ContextPart<dyn Any>>,
    /// Its pages of the Settings window, over the erased state.
    pub settings: Vec<SettingsPage<dyn Any>>,
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
    /// `ui`, registered: its contexts read once and its state's type
    /// erased.
    pub(super) fn new(ui: impl UiPackage) -> Self {
        Registered {
            contexts: ui.contexts().into_iter().map(ContextPart::erase).collect(),
            settings: ui.settings().into_iter().map(SettingsPage::erase).collect(),
            package: Arc::new(ui),
        }
    }

    /// `package`'s UI; `None`: a headless package.
    pub fn of(package: &Package) -> Option<&Registered> {
        package.ui().as_ref()?.parts().downcast_ref()
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

    /// Context `id`'s part.
    pub fn context(&self, id: kinogaki_studio_api::ContextId) -> Option<&ContextPart<dyn Any>> {
        self.contexts.iter().find(|c| c.id == id)
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
