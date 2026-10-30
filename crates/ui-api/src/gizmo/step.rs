//! A transform drag the host runs for a context: the axes it works in
//! ([`TransformSpace`]), where it is ([`TransformPhase`]) and what it has
//! made of the pointer so far ([`TransformStep`]).
use super::{Handle, TransformTool};
use kinogaki_studio_api::Vec3;

/// The axes the transform gizmo works in (each tool's own, a user
/// setting).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TransformSpace {
    /// The scene's axes.
    #[default]
    World,
    /// The object's own axes (Local at an object level).
    Object,
    /// The object's parent's axes (object levels only).
    Parent,
    /// The selection's own orientation: Y along its normal
    /// ([`Levels::normal`](crate::package::Levels::normal); inside an
    /// object only).
    Normal,
    /// The view's: X right, Y up, Z toward the viewer.
    View,
}

/// Where a transform drag is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TransformPhase {
    /// A handle was pressed: the step is still nothing.
    Began,
    /// The pointer moved, or a value was typed.
    Moved,
    /// Released, or Enter: the step is the edit to make.
    Released,
    /// Escape, focus lost or an undo: put back what the drag showed.
    Cancelled,
}

/// What a transform drag has made of the pointer since the press: one
/// of a move, a turn or a stretch, about the gizmo's middle and along its
/// axes, in the edited object's space. Typed values, steps and snapping
/// are in it already.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TransformStep {
    /// What the drag does (the Manipulator's handle resolved to its tool).
    pub tool: TransformTool,
    /// The handle dragged (resolved as `tool`).
    pub handle: Handle,
    /// The gizmo's middle when pressed: what it turns and scales about.
    pub center: Vec3,
    /// Its axes when pressed (unit length).
    pub axes: [Vec3; 3],
    /// The move, in the edited object's units.
    pub translate: Vec3,
    /// The turn about each of `axes`, in degrees (one of them, for a ring).
    pub rotate: [f64; 3],
    /// The stretch along each of `axes` (1: none).
    pub scale: [f64; 3],
}

impl TransformStep {
    /// No change yet, at `center` along `axes`.
    pub fn none(tool: TransformTool, handle: Handle, center: Vec3, axes: [Vec3; 3]) -> Self {
        Self {
            tool,
            handle,
            center,
            axes,
            translate: Vec3::ZERO,
            rotate: [0.0; 3],
            scale: [1.0; 3],
        }
    }
}
