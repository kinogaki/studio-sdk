//! A context's selection levels ([`Levels`]).
use super::part;
use crate::gizmo::PivotFrame;
use crate::host::{Cx, ObjectRef};
use crate::package::data::{Candidate, LevelPreset, LevelSpec};
use crate::{Pos, Rect};
use kinogaki_studio_api::{Combine, Components, Vec3};

part! {
    /// A context's selection levels (Mesh's Piece, Face, Edge, Point; CAD's
    /// Body, Face, Edge, CV…). The selection itself is the package's own
    /// [slot](kinogaki_studio_api::Slot).
    pub trait Levels<S> {
        /// Its levels, in order (number keys 1, 2, … choose them). Read once,
        /// when the package registers.
        fn levels(&self) -> Vec<LevelSpec>;
        /// The active levels, by index.
        fn active(&self, s: &S, cx: &dyn Cx) -> Vec<usize>;
        /// The level selections are kept at and the marking menus show.
        fn primary(&self, s: &S, cx: &dyn Cx) -> usize;
        /// Makes `level` the only active level, converting the selection to it.
        fn choose(&self, s: &mut S, cx: &mut dyn Cx, level: usize);
        /// Adds `level` to the active levels, or removes it (never the last).
        fn toggle(&self, s: &mut S, cx: &mut dyn Cx, level: usize);
        /// The level of what is selected, by index (an edge selected while
        /// faces are the active level: the edge level); `None`: nothing is.
        /// The library's entries are held against it, else against the
        /// primary level ([`When`](crate::package::When)).
        fn selected(&self, s: &S, cx: &dyn Cx) -> Option<usize> {
            let _ = (s, cx);
            None
        }
        /// The open object and its selected components, as components of the
        /// value it shows ([`HostDoc::shown`](crate::host::HostDoc::shown)):
        /// one set per element kind the selection holds (edges and faces after
        /// a Shift-mixed pick), no kind twice, each highlighted with its own
        /// kind's style; what a Material dropped on that object takes, and the
        /// selection the spreadsheet shows. A set may be empty: its level shows
        /// with nothing selected at it (an empty point set still draws the
        /// points). `None`: the context keeps no component selection there.
        fn components(&self, s: &S, cx: &dyn Cx) -> Option<(ObjectRef, Vec<Components>)> {
            let _ = (s, cx);
            None
        }
        /// What the pointer is over at the active level, as the host picked it
        /// through the open value's `Pickable` (`None`: nothing there): show it
        /// hovered.
        fn hovered(&self, s: &mut S, cx: &mut dyn Cx, hit: Option<Components>) {
            let _ = (s, cx, hit);
        }
        /// A click, box or lasso at the active level, as the host picked it
        /// through the open value's `Pickable` with its pick policy (`None`:
        /// nothing there), combined with the selection as `how` says (Shift
        /// adds, Command removes): the context decides what the hit selects.
        fn picked(&self, s: &mut S, cx: &mut dyn Cx, hit: Option<Components>, how: Combine) {
            let _ = (s, cx, hit, how);
        }
        /// The selection as the transform pivot reads it, in the context's
        /// space; `None`: nothing to pivot about (no pivot button). Called
        /// once a frame while the numeric bar shows.
        fn pivot(&self, s: &S, cx: &dyn Cx) -> Option<PivotFrame> {
            let _ = (s, cx);
            None
        }
        /// The user chose another pivot: a live tool takes it.
        fn pivot_changed(&self, s: &mut S, cx: &mut dyn Cx) {
            let _ = (s, cx);
        }
        /// The selection's normal, in the context's space: the Normal
        /// transform space's Y. `None`: Normal works as Object.
        fn normal(&self, s: &S, cx: &dyn Cx) -> Option<Vec3> {
            let _ = (s, cx);
            None
        }
        /// Its preset after its levels, if it has one (read once, as
        /// [`levels`](Self::levels)).
        fn preset(&self) -> Option<LevelPreset> {
            None
        }
        /// Whether its preset is on.
        fn preset_on(&self, s: &S, cx: &dyn Cx) -> bool {
            let _ = (s, cx);
            false
        }
        /// Turns its preset on or off (the button, or its number key).
        fn toggle_preset(&self, s: &mut S, cx: &mut dyn Cx) {
            let _ = (s, cx);
        }
        /// What lies under `pointer` in view `rect` at the active levels,
        /// nearest first: the stacked list's rows. Empty: the list holds
        /// objects only.
        fn candidates(&self, s: &S, cx: &dyn Cx, rect: Rect, pointer: Pos) -> Vec<Candidate> {
            let _ = (s, cx, rect, pointer);
            Vec::new()
        }
        /// The user picked `candidate` in the stacked list: select it as a
        /// click on it would (Shift adds, Command removes).
        fn pick_candidate(
            &self,
            s: &mut S,
            cx: &mut dyn Cx,
            candidate: &Candidate,
            shift: bool,
            command: bool,
        ) {
            let _ = (s, cx, candidate, shift, command);
        }
        /// Shows `candidate` hovered (the stacked list's row under the
        /// pointer); `None` clears it.
        fn hover_candidate(&self, s: &mut S, cx: &mut dyn Cx, candidate: Option<&Candidate>) {
            let _ = (s, cx, candidate);
        }
    }
}
