//! The transform gizmo's pure geometry: where its handles are on screen
//! ([`GizmoFrame`]), which one the pointer is over ([`hit`]) and what a
//! drag of one means ([`GizmoFrame::delta`], [`scale`], [`angle_change`]).
//!
//! Arrows and plane squares move, rings rotate, and squares, plane squares
//! and the middle scale; the Manipulator combines them. The host paints
//! the gizmo; a plugin's tool owns what a drag transforms and uses this
//! geometry to turn pointer motion into it. The maths is the host's own,
//! so hits and drags agree with what the host draws.
mod geometry;
mod hit;
mod pivot;

pub use geometry::{
    GizmoFrame, Handle, MANIPULATOR_CUBE, MANIPULATOR_RING, angle_change, distance, inside, scale,
    stepped,
};
pub use hit::{ALL_AXES, hit, hit_axes};
pub use pivot::{Pivot, PivotFrame};

/// The viewport's transform mode (Q, W, E, R and T).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TransformTool {
    /// No handles: clicks select.
    Select,
    /// Arrows, plane squares and the middle move.
    Move,
    /// Rings rotate.
    Rotate,
    /// Squares, plane squares and the middle scale.
    Scale,
    /// Move, rotate and scale handles at once.
    Manipulator,
}

impl TransformTool {
    /// What a drag of `handle` does: the single-purpose tool and its
    /// handle. The Manipulator's rings rotate and its cubes scale along
    /// their axis; its arrows, squares and middle move.
    pub fn resolve(self, handle: Handle) -> (Self, Handle) {
        match (self, handle) {
            (Self::Manipulator, Handle::Ring(axis)) => (Self::Rotate, Handle::Axis(axis)),
            (Self::Manipulator, Handle::Stretch(axis)) => (Self::Scale, Handle::Axis(axis)),
            (Self::Manipulator, other) => (Self::Move, other),
            other => other,
        }
    }
}
