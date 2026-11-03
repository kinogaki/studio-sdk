//! [`HostCall`]: what a package asked of a [`FakeHost`](super::FakeHost).
use kinogaki_studio_api::params::Severity;
use kinogaki_studio_api::{Bounds, ContextId, RowsFor, SlotId, Time};
use studio_ui_api::gizmo::{TransformSpace, TransformTool};
#[cfg(doc)]
use studio_ui_api::host::{
    Cx, HostDoc, HostNetwork, HostSelection, HostSettings, HostShell, HostSlots, HostStatus,
    HostTransform, HostUndo, HostView,
};
use studio_ui_api::host::{NodeRef, ObjectRef, Place};

/// What a package asked of a [`FakeHost`](super::FakeHost), in order (queries are not
/// recorded).
#[derive(Clone, Debug, PartialEq)]
pub enum HostCall {
    /// [`HostUndo::begin_gesture`].
    BeginGesture(String),
    /// [`HostUndo::finish_gesture`].
    FinishGesture,
    /// [`HostUndo::cancel_gesture`].
    CancelGesture,
    /// [`HostUndo::undo`].
    Undo,
    /// [`HostUndo::redo`].
    Redo,
    /// [`HostSlots::edit_slot_any`], by slot and step label.
    EditSlot(SlotId, String),
    /// [`HostSelection::select_objects`].
    SelectObjects(Vec<ObjectRef>),
    /// [`HostSelection::pick_node`].
    PickNode(NodeRef, bool),
    /// [`HostDoc::go_to`].
    GoTo(Place),
    /// [`HostDoc::set_time`].
    SetTime(Time),
    /// [`HostDoc::set_object_body`], by object and step label, whether or
    /// not the body was taken.
    SetBody(ObjectRef, String),
    /// [`HostView::frame_bounds`].
    Frame(Bounds),
    /// [`HostView::redraw`].
    Redraw,
    /// [`HostStatus::fail`].
    Fail(String),
    /// [`HostStatus::info`].
    Info(String),
    /// [`HostStatus::console`]: level, subject, text.
    Console(Severity, Option<String>, String),
    /// [`HostShell::copy_text`].
    CopyText(String),
    /// [`Cx::latest_any`] running a job, by its kind.
    Latest(String),
    /// [`HostShell::run_action`].
    RunAction(String, String),
    /// [`HostTransform::set_transform_tool`].
    SetTransformTool(TransformTool),
    /// [`HostTransform::set_snapping`].
    SetSnapping(bool),
    /// [`HostTransform::set_transform_space`].
    SetTransformSpace(TransformSpace),
    /// [`HostSettings::set_section_any`].
    SetSection(ContextId),
    /// [`HostNetwork::edit_network`], by object and step label, whether or
    /// not the edit held.
    EditNetwork(ObjectRef, String),
    /// [`Cx::node_rows`].
    NodeRows(ObjectRef, NodeRef, RowsFor),
}
