//! [`Snappable`]: what a gizmo move may land on in a value: its points,
//! edges' middles, faces' centres, crossings, edges and surfaces nearest
//! the pointer. The host keeps everything else about snapping: the user's
//! targets and the master, what the move snaps from, the grid, ranking
//! across objects, the handle's axis or plane, and the feedback drawn.
use crate::components::Components;
use crate::data::Data;
use crate::facets::{Drawn, Matrix, WorldRay};

/// What a snap may land on, as the user's snap targets say.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct SnapTargets {
    /// Points.
    pub points: bool,
    /// Edges' middles.
    pub middles: bool,
    /// Faces' centres.
    pub face_centers: bool,
    /// Where two edges cross on screen.
    pub crossings: bool,
    /// The nearest place on an edge.
    pub edges: bool,
    /// The place on a surface under the pointer.
    pub faces: bool,
}

impl SnapTargets {
    /// Whether any target is on.
    pub fn any(self) -> bool {
        self != Self::default()
    }
}

/// What a snap landed on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SnapKind {
    /// A point.
    Point,
    /// An edge's middle.
    Middle,
    /// A face's centre.
    FaceCenter,
    /// Where two edges cross on screen.
    Crossing,
    /// The nearest place on an edge.
    Edge,
    /// The place on a surface under the pointer.
    Face,
}

impl SnapKind {
    /// What the snap marker calls it.
    pub fn label(self) -> &'static str {
        match self {
            Self::Point => "Point",
            Self::Middle => "Middle",
            Self::FaceCenter => "Face center",
            Self::Crossing => "Crossing",
            Self::Edge => "Edge",
            Self::Face => "Surface",
        }
    }
}

/// One snap query against one object's value, in one view.
#[derive(Clone, Copy, Debug)]
pub struct SnapQuery<'a> {
    /// The object's space to clip space: the view's projection times the
    /// object's world placement.
    pub clip_from_object: Matrix,
    /// The view's size in pixels.
    pub size: [f64; 2],
    /// The pointer in the view, in pixels from its top left.
    pub pointer: [f64; 2],
    /// The pointer's ray, in the object's space.
    pub ray: WorldRay,
    /// How near the pointer a target must be on screen, in pixels.
    pub radius: f64,
    /// Components the move carries (the edited object's own, in a
    /// component drag): never targets. `None`: nothing of the value moves.
    pub moving: Option<&'a Components>,
    /// Whether targets the value's surface hides count (select through).
    pub through: bool,
}

/// A target a query found, in the object's space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SnapHit {
    /// What it landed on.
    pub kind: SnapKind,
    /// Where.
    pub position: [f64; 3],
}

/// How a value is snapped to.
pub trait Snappable: Send + Sync {
    /// The best of `targets` in `data` (drawn as `drawn`) for `query`: the
    /// nearest on screen within its radius, a point before an edge before
    /// a surface; `None` when none is near.
    fn snap(
        &self,
        data: &Data,
        drawn: &Drawn,
        query: &SnapQuery<'_>,
        targets: SnapTargets,
    ) -> Option<SnapHit>;

    /// The point of `data` nearest `to` (both in the object's space) among
    /// `moving` (`None`: all of it): what a "closest" snap moves from.
    /// `None`: the value has no points to offer, and the move snaps from
    /// its pivot.
    fn closest(
        &self,
        data: &Data,
        drawn: &Drawn,
        moving: Option<&Components>,
        to: [f64; 3],
    ) -> Option<[f64; 3]> {
        let _ = (data, drawn, moving, to);
        None
    }
}
