//! A context's parts, one trait each, over the package's state `S`: the
//! selection [`Levels`]; in the viewport its [`LiveTools`], [`Panels`],
//! [`Transforms`] and [`Display`]; the
//! [`Inspector`], the [`Vocabulary`] and the [`ObjectRows`] of the object
//! kinds it owns. A context has the parts it gives and nothing else: what
//! a part does is there only where the part is.
//!
//! Each method gets the package's state and the call's context (`cx`);
//! a query gets them shared (`&S`, `&dyn Cx`) and cannot change anything.
//! Parts are stateless values: their state is `S`, which the host lends
//! for one call at a time ([`WithUi`](super::WithUi)). Every method
//! runs on the UI thread and returns at once.
//!
//! **A frame**, for the open context's viewport parts, in order:
//! [`LiveTools::holds`]; then for each view [`Panels::panels`] (where its
//! panels are, which take the pointer from the view) and
//! [`LiveTools::tools`] before the camera's navigation, then core's gizmo,
//! handles and picks as the tools leave them ([`Takes`](super::Takes)); the
//! frame's commands; then once [`Display::settle`]; then for each drawn
//! view [`Display::overlay`], each panel's [`Panels::panel`], and
//! [`Display::edited`]. Keys come before all of it
//! ([`LiveTools::key`]), and [`LiveTools::cancel`] ends a live gesture
//! (Escape, an undo, the location leaving the context: [`Ended`](super::Ended)).
//!
//! Each trait is written once, through [`part!`]: the host's erased
//! form of it (over `dyn Any`, the state downcast back to `S` for each
//! call) is made from the same definition, so no method can be left out
//! of it.
mod content;
mod levels;
mod viewport;

pub use content::{Inspector, Invoked, ObjectRows, RowAction, Vocabulary};
pub use levels::Levels;
pub use viewport::{Display, LiveTools, Panels, Transforms};

use std::any::Any;

/// The state the host lent, as the package's own: `&dyn Any` as `&T`,
/// `&mut dyn Any` as `&mut T`.
pub(crate) trait Lend<T> {
    /// What it lends as.
    type Out;
    /// The state, downcast.
    fn lend(self) -> Self::Out;
}

impl<'a, T: 'static> Lend<T> for &'a (dyn Any + 'static) {
    type Out = &'a T;
    fn lend(self) -> &'a T {
        self.downcast_ref()
            .expect("the host lends each package its own state")
    }
}

impl<'a, T: 'static> Lend<T> for &'a mut (dyn Any + 'static) {
    type Out = &'a mut T;
    fn lend(self) -> &'a mut T {
        self.downcast_mut()
            .expect("the host lends each package its own state")
    }
}

/// A part's trait over `S`, and its erased form: the same trait over
/// `dyn Any` for an [`Erased`](super::hosting::Erased) part, each method
/// forwarded with the state downcast. A method taking the state names it
/// `s` and has it first; one that doesn't takes `&self` alone.
macro_rules! part {
    (
        $(#[$doc:meta])*
        pub trait $name:ident<S> {
            $($methods:tt)*
        }
    ) => {
        $(#[$doc])*
        pub trait $name<S: ?Sized>: Send + Sync {
            $($methods)*
        }

        const _: () = {
            type S = dyn std::any::Any;
            impl<T: 'static> $name<S> for $crate::package::hosting::Erased<T, dyn $name<T>> {
                $crate::package::parts::part!(@forward [] $($methods)*);
            }
        };
    };
    (@forward [$($done:tt)*]) => { $($done)* };
    (@forward [$($done:tt)*]
        $(#[$m:meta])* fn $f:ident(&self, s: $st:ty $(, $a:ident: $t:ty)* $(,)?) $(-> $r:ty)? ;
        $($rest:tt)*
    ) => {
        $crate::package::parts::part!(@forward [$($done)*
            fn $f(&self, s: $st $(, $a: $t)*) $(-> $r)? {
                self.part.$f(<_ as $crate::package::parts::Lend<T>>::lend(s) $(, $a)*)
            }
        ] $($rest)*);
    };
    (@forward [$($done:tt)*]
        $(#[$m:meta])* fn $f:ident(&self, s: $st:ty $(, $a:ident: $t:ty)* $(,)?) $(-> $r:ty)? { $($body:tt)* }
        $($rest:tt)*
    ) => {
        $crate::package::parts::part!(@forward [$($done)*
            fn $f(&self, s: $st $(, $a: $t)*) $(-> $r)? {
                self.part.$f(<_ as $crate::package::parts::Lend<T>>::lend(s) $(, $a)*)
            }
        ] $($rest)*);
    };
    (@forward [$($done:tt)*]
        $(#[$m:meta])* fn $f:ident(&self) $(-> $r:ty)? ;
        $($rest:tt)*
    ) => {
        $crate::package::parts::part!(@forward [$($done)*
            fn $f(&self) $(-> $r)? {
                self.part.$f()
            }
        ] $($rest)*);
    };
    (@forward [$($done:tt)*]
        $(#[$m:meta])* fn $f:ident(&self) $(-> $r:ty)? { $($body:tt)* }
        $($rest:tt)*
    ) => {
        $crate::package::parts::part!(@forward [$($done)*
            fn $f(&self) $(-> $r)? {
                self.part.$f()
            }
        ] $($rest)*);
    };
}
pub(crate) use part;
