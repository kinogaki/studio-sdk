//! What a context adds to the viewport, one capability a trait: its live
//! tools' input ([`LiveTools`]), its floating panels ([`Panels`]), the
//! host's transform gizmo over its selection ([`Transforms`]), what it
//! draws over the open object ([`Display`]), its own time
//! ([`Timeline`]) and a live tool's numeric bar ([`NumericBar`]). The rest
//! of the viewport is core's: the left toolbar, the status bar, framing,
//! the gizmo, pivot picks and component picks (through
//! [`Levels`](crate::package::Levels)).
use super::part;
use crate::gizmo::{TransformPhase, TransformStep};
use crate::host::{Cx, NodeRef, ViewCx};
use crate::package::data::{Ended, GizmoUse, LiveHandles, NumericFields, Panel, Takes, TimeDomain};
use crate::package::edited::Edited;
use crate::package::view::{Holds, KeyPress};
use crate::sheet::{Edits, Schema};
use crate::{Painter, Ui};
use kinogaki_studio_api::Parameters;
use kinogaki_studio_api::geometry::Geometry;

part! {
    /// A context's live tools over the views: their input, keys and
    /// gestures. What a tool draws is the context's [`Display`] and
    /// [`Panels`]; its numeric bar its [`NumericBar`].
    pub trait LiveTools<S> {
        /// A live tool's input in a view, before camera navigation: its
        /// drags, clicks and wheel (Alt's drags are the camera's), read
        /// from the view ([`ViewCx::view`]) and this frame's input
        /// ([`ViewCx::input`]). What it says it [`Takes`] keeps core's
        /// picks, gizmo, handles, hover, wheel and right-click menu off
        /// this frame. A drag edits in the tool's gesture, one undo step.
        fn tools(&self, s: &mut S, cx: &mut dyn ViewCx) -> Takes;
        /// Ends live drags and gestures, for the reason `why` says
        /// ([`Ended`], with the order Escape is taken in).
        fn cancel(&self, s: &mut S, cx: &mut dyn Cx, why: Ended);
        /// A key or typed text over the viewport, before any binding (see
        /// [`KeyPress`]): a live tool's own keys (Enter), a focused value
        /// field's typing ([`HostShell::keyboard_taken`](crate::host::HostShell::keyboard_taken)
        /// says a field has the keys). Escape never comes here: it is
        /// [`cancel`](Self::cancel)'s. `true`: the tools took it, and
        /// nothing else sees it.
        fn key(&self, s: &mut S, cx: &mut dyn Cx, press: &KeyPress) -> bool {
            let _ = (s, cx, press);
            false
        }
        /// What a live gesture keeps from the rest of the viewport now
        /// (asked once a frame, before the views' input).
        fn holds(&self, s: &S, cx: &dyn Cx) -> Holds {
            let _ = (s, cx);
            Holds::default()
        }
        /// The node a live tool works on (its card is up). While there is
        /// one, the open network's templated branches are pickable in the
        /// views: a click wires the branch under the pointer into the
        /// node's operand input (Command takes it off), in the tool's
        /// gesture. Core marks the branch under the pointer and those wired
        /// into the node, and shows none of the node's own handles (its
        /// tool and card adjust it). `None`: no tool is live.
        fn live_node(&self, s: &S, cx: &dyn Cx) -> Option<NodeRef> {
            let _ = (s, cx);
            None
        }
        /// The handles over a live tool that keeps its rows itself (Extrude's
        /// arrow, in a held edit's step), and those rows now: core shows and
        /// drags them as a node's, and hands each move to
        /// [`set_rows`](Self::set_rows) in the tool's gesture. Asked once a
        /// frame. `None`: none; the live node shows none of its own.
        fn handles(&self, s: &S, cx: &dyn Cx) -> Option<LiveHandles> {
            let _ = (s, cx);
            None
        }
        /// A drag of a [`handles`](Self::handles) handle moved the tool's rows
        /// to `values`.
        fn set_rows(&self, s: &mut S, cx: &mut dyn Cx, values: &Parameters) {
            let _ = (s, cx, values);
        }
        /// The open object's own snap targets beyond its shown value (a
        /// curve's control points, a sketch's points and lines), in its
        /// space: points snap as points, polylines as edges, their middles
        /// and where they cross. [`ViewCx::snap`] and the host's gizmo
        /// search them with the shown value, in one order and with one
        /// feedback. Asked once a frame, before the views'
        /// [`tools`](Self::tools); the same value (its positions and
        /// topology shared) while nothing changed costs nothing. `None`:
        /// none.
        fn own_targets(&self, s: &S, cx: &dyn Cx) -> Option<Geometry> {
            let _ = (s, cx);
            None
        }
    }
}

part! {
    /// A context's floating panels over the views: a live tool's card, a
    /// check's findings. The host asks where they are before the views'
    /// input, so the pointer over one is the panel's and not the view's,
    /// and draws each once a frame after it.
    pub trait Panels<S> {
        /// Its panels over the call's view this frame, said from the state
        /// they draw from (never recorded while drawing), so they hold
        /// for the frame that draws them. A [`Pane`](crate::package::PanelScope::Pane)
        /// panel is placed over the whole view pane
        /// ([`View::pane`](crate::host::View::pane)) and drawn once
        /// whichever views give it; a [`View`](crate::package::PanelScope::View)
        /// one is the view's own (its id names its view). The host draws
        /// each id once.
        fn panels(&self, s: &S, cx: &dyn ViewCx) -> Vec<Panel>;
        /// Draws panel `panel` into `ui`, which is placed and clipped at
        /// its rect: a card ([`Ui::card`](crate::Ui::card)) or rows.
        fn panel(&self, s: &mut S, cx: &mut dyn Cx, panel: &Panel, ui: &mut dyn Ui);
    }
}

part! {
    /// The host's transform tools over a context's selection: the left
    /// toolbar's Select, Move, Rotate, Scale, Manipulator and Pivot, the
    /// Gear's tool settings, and the gizmo the host shows at the
    /// selection's pivot ([`Levels::pivot`](crate::package::Levels::pivot)) along
    /// the user's transform space. Without this part the views show the
    /// selected node's handles in the transform tool's place, as Maya's
    /// manipulators do.
    pub trait Transforms<S> {
        /// Which gizmo shows now: the selection's (the user picked
        /// components to move), the selected node's declared handles, or
        /// neither ([`GizmoUse`]).
        fn gizmo(&self, s: &S, cx: &dyn Cx) -> GizmoUse;
        /// A drag of the selection's gizmo, in the host's undo gesture:
        /// show `step` (a preview, not yet an edit: points moved through
        /// [`Edited::moving`]) while it `Began` or `Moved`, make it the edit
        /// on `Released`, and put back what the drag showed on `Cancelled`
        /// (the host cancels its gesture). A node the edit needs is added at
        /// the first step that [`moves`](crate::gizmo::TransformStep::moves),
        /// in that gesture: a click on the gizmo leaves nothing, and no undo
        /// step.
        fn transform(&self, s: &mut S, cx: &mut dyn Cx, phase: TransformPhase, step: &TransformStep);
        /// The Gear's rows: the transform tool's settings here (soft
        /// selection, symmetry), under core's space row; the Gear shows
        /// while it has rows. A live tool's settings are its card. Asked
        /// once a frame, so it is built from the state as it is.
        fn gear(&self, s: &S, cx: &dyn Cx) -> Schema<u32> {
            let _ = (s, cx);
            Schema::new()
        }
        /// The edits of [`gear`](Self::gear)'s rows, `rows` holding the
        /// edited values.
        fn apply_gear(&self, s: &mut S, cx: &mut dyn Cx, rows: &Schema<u32>, edits: &Edits<u32>) {
            let _ = (s, cx, rows, edits);
        }
    }
}

part! {
    /// What a context draws in the views besides the open object's value,
    /// which core draws itself.
    pub trait Display<S> {
        /// What it adds over the open object in the call's view this
        /// frame: overlays, guides, ghosts, a drag's moving points
        /// (`None`: nothing). A query, asked for each drawn view after
        /// [`settle`](Self::settle); the same `Arc`s while nothing changed
        /// cost nothing.
        fn edited(&self, s: &S, cx: &dyn ViewCx) -> Option<Edited>;
        /// Brings what it draws up to date, once a frame after every
        /// input and command of the frame, before any view is drawn:
        /// where a context adopts a change (a new topology, a selection)
        /// and asks its background jobs.
        fn settle(&self, s: &mut S, cx: &mut dyn Cx) {
            let _ = (s, cx);
        }
        /// Paints over the call's view after its navigation, so what it
        /// places follows this frame's camera: labels, heads-up values.
        fn overlay(&self, s: &S, cx: &dyn ViewCx, painter: &dyn Painter) {
            let _ = (s, cx, painter);
        }
    }
}

part! {
    /// A context's own time, which the Timeline shows and drives instead
    /// of the scene's while the context is open (a print's seconds).
    pub trait Timeline<S> {
        /// Its time now; `None`: the scene's (nothing of its own to show).
        fn time(&self, s: &S, cx: &dyn Cx) -> Option<TimeDomain>;
        /// Shows the context at time `t`, in seconds (a scrub, a step or a
        /// played frame).
        fn set_time(&self, s: &mut S, cx: &mut dyn Cx, t: f64);
    }
}

part! {
    /// A live tool's numeric bar: while it gives one, it stands in the
    /// transform tool's place under the active view.
    pub trait NumericBar<S> {
        /// Its fields now; `None`: no bar.
        fn fields(&self, s: &S, cx: &dyn Cx) -> Option<NumericFields>;
        /// A value typed or chosen in field `field` of
        /// [`fields`](Self::fields) (`current`: what the field held).
        fn apply(&self, s: &mut S, cx: &mut dyn Cx, field: usize, current: f64, value: f64);
    }
}
