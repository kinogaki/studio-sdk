//! A context's inspector ([`Inspector`]), how its items run
//! ([`Vocabulary`]) and the inspector rows of the object kinds it owns
//! ([`ObjectRows`]).
use super::part;
use crate::Ui;
use crate::host::{Cx, InspectorCx, NodeRef, ObjectRef};
use crate::sheet::{Edits, Schema};
use kinogaki_studio_api::ObjectKindId;

part! {
    /// The inspector panel while the context is open, drawn under the node
    /// sheet of the inspected node (where there is one).
    pub trait Inspector<S> {
        /// Draws the panel (its sheets through [`Ui::sheet`]).
        fn show(&self, s: &mut S, cx: &mut dyn InspectorCx, ui: &mut dyn Ui);
        /// The user took `action` on row `row` (a parameter's name) of node
        /// `node`'s sheet: [`RowAction::Pick`] on a row offering a pick in the
        /// viewport (a group from the node's input). Core's sheet draws the
        /// rows and owns file, path and reference rows itself.
        fn row_action(&self, s: &mut S, cx: &mut dyn Cx, node: NodeRef, row: &str, action: RowAction) {
            let _ = (s, cx, node, row, action);
        }
        /// The text being typed into row `row` of node `node`'s sheet, as it
        /// is now (each change); `None` once typing ends. For live feedback
        /// (the elements a group expression names), never the edit itself.
        fn typing(&self, s: &mut S, cx: &mut dyn Cx, node: NodeRef, row: &str, text: Option<&str>) {
            let _ = (s, cx, node, row, text);
        }
    }
}

/// What the user did on an inspector row ([`Inspector::row_action`]). New
/// actions may be added; a context that doesn't know one ignores it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum RowAction {
    /// Pick in the viewport for the row (its pick button).
    Pick,
}

part! {
    /// How a context's items run, and what of them applies now. The items
    /// themselves, and every menu, shelf, key and Tab entry holding them, are
    /// its [`Library`](crate::package::Library).
    pub trait Vocabulary<S> {
        /// Its name, which actions carry ("acme.trees").
        fn name(&self) -> &str;
        /// Runs `item`, as `how` says: a tool over the views, its node over
        /// the network, or again with its last values (the host's Repeat). An
        /// item can be run where it doesn't apply (a key, a shelf tile): it
        /// then does nothing but say why
        /// ([`HostStatus::fail`](crate::host::HostStatus::fail)).
        fn run(&self, s: &mut S, cx: &mut dyn Cx, item: &str, how: Invoked);
        /// Whether `item` applies now: at the active level, to the selection,
        /// in this mode (Fillet needs edges selected; Repeat a last tool).
        /// Menus, Tab and the marking menus hide what doesn't apply; nothing
        /// is greyed. Never a passing state (a cook under way): [`run`](Self::run)
        /// refuses that, saying why. Called while a menu shows, once an entry
        /// a frame.
        fn shown(&self, s: &S, cx: &dyn Cx, item: &str) -> bool {
            let _ = (s, cx, item);
            true
        }
        /// `item`'s label now, when its state renames it ("Repeat Bevel",
        /// "Slice Off"); `None`: its library label.
        fn label(&self, s: &S, cx: &dyn Cx, item: &str) -> Option<String> {
            let _ = (s, cx, item);
            None
        }
    }
}

/// Where an item was run from: what running it means.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Invoked {
    /// Over the views, from a menu, shelf, key or Tab listing tools: the
    /// tool runs on what is selected.
    #[default]
    Views,
    /// From Tab over the network: its node is added.
    Network,
    /// The host's Repeat: the item runs again with its last values (the
    /// package keeps them; an item it has none for runs as from the views).
    Repeat,
    /// At an object level (the scene root, a Part), from its menus and
    /// shelves: the item runs on the selected objects it takes
    /// ([`Places::objects`](crate::package::Places::objects)).
    Objects,
}

part! {
    /// The inspector rows of the object kinds a context owns, below an
    /// object's Name and Type.
    pub trait ObjectRows<S> {
        /// The kinds whose rows it gives. Read once.
        fn kinds(&self) -> Vec<ObjectKindId>;
        /// Object `object`'s rows.
        fn rows(&self, s: &S, cx: &dyn Cx, object: ObjectRef) -> Schema<u32>;
        /// The edits of those rows.
        fn apply(
            &self,
            s: &mut S,
            cx: &mut dyn Cx,
            object: ObjectRef,
            rows: &Schema<u32>,
            edits: &Edits<u32>,
        );
    }
}
