//! What a context takes from the host's cooks ([`Results`]).
//!
//! The host cooks every object and adopts what comes back: it keeps the
//! answers, reports problems (the error line, the failed node's
//! warnings), counts what the open object shows, keeps the output
//! selection, Freeze and the spreadsheet's pin, and publishes each
//! object's result. A context keeps only its gestures' part: while one of
//! its gestures stands on what the open object shows (a drag previewing
//! over the shown packet), the host keeps the next answer waiting
//! ([`Results::hold`]); it may refuse to show a gesture's earlier state or
//! a refinement for now ([`Results::takes`]); and it hears whenever what
//! the open object shows changes ([`Results::shown`]), to drop or resume
//! its gestures.
use super::parts::{Erased, own, own_mut};
use crate::host::{Cx, ObjectRef};
use kinogaki_studio_api::{Components, Data, Drawn};
use std::any::Any;

/// How a context takes the host's cooks of its objects. Every method runs
/// on the UI thread and returns at once.
pub trait Results<S: ?Sized>: Send + Sync {
    /// Asked before each answer is taken: whether a live gesture stands on
    /// what the open object shows (a drag composing onto the cooking
    /// commit's preview). The host then keeps the answer waiting and shows
    /// no refinement until it ends. Gesture state that no longer stands (a
    /// preview of an older revision) is dropped here.
    fn hold(&self, s: &mut S, cx: &mut dyn Cx) -> bool;
    /// Whether the open object may show `change` now: a gesture's earlier
    /// state ([`ShownKind::Continuing`]) or a refinement
    /// ([`ShownKind::Refined`]). Answers, failures and closings always
    /// show. By default, yes.
    fn takes(&self, s: &S, cx: &dyn Cx, change: &Shown) -> bool {
        let _ = (s, cx, change);
        true
    }
    /// What the open object shows changed (`change`).
    fn shown(&self, s: &mut S, cx: &mut dyn Cx, change: &Shown);
}

/// A change to what the open object shows.
#[derive(Clone, Debug)]
pub struct Shown {
    /// The document revision the shown result was cooked at.
    pub revision: u64,
    /// Why it changed.
    pub kind: ShownKind,
    /// The open object (`None`: an object level is open).
    pub object: Option<ObjectRef>,
    /// What it shows now, and how: the value its network displays and
    /// its draw pieces (`None` when it failed or is closed). It reaches
    /// the board ([`HostDoc::shown`](crate::host::HostDoc::shown)) when
    /// the host next publishes.
    pub result: Option<(Data, Drawn)>,
    /// The selection the displayed node's output carries, as components
    /// of `result`'s value.
    pub selection: Option<Components>,
}

/// Why what the open object shows changed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShownKind {
    /// The current revision's result.
    Answer,
    /// An earlier state of the open gesture, shown while its latest edit
    /// cooks.
    Continuing,
    /// Exact draw pieces for the shown result.
    Refined,
    /// The current revision failed to cook: the last good result stays.
    Failed,
    /// No object is open (an object level), or the open one has no result
    /// yet.
    Closed,
}

impl<S: 'static> Results<dyn Any> for Erased<S, dyn Results<S>> {
    fn hold(&self, s: &mut dyn Any, cx: &mut dyn Cx) -> bool {
        self.part.hold(own_mut(s), cx)
    }
    fn takes(&self, s: &dyn Any, cx: &dyn Cx, change: &Shown) -> bool {
        self.part.takes(own(s), cx, change)
    }
    fn shown(&self, s: &mut dyn Any, cx: &mut dyn Cx, change: &Shown) {
        self.part.shown(own_mut(s), cx, change);
    }
}
