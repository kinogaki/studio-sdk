//! The plain values the host interface exchanges: references to objects
//! and nodes, where the user is, one view as a package sees it, what a
//! node's rows drawn by the host did, and the user's preferences.
use crate::gizmo::TransformTool;
use crate::{Modifiers, Pos, Projection, Rect, Response, Slot};
use kinogaki_studio_api::geometry::Ray;
use kinogaki_studio_api::{ContextId, Units, Vec3};
pub use kinogaki_studio_api::{NodeRef, ObjectRef};

/// Where the user is: an object level, or a place inside an object (its
/// context's network, a subnet in it, a node the level is opened on).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Place {
    /// The open level's context (the scene's at an object level).
    pub context: ContextId,
    /// The object whose inside is open; at an object level, the group or
    /// Part whose children show (`None`: the scene root).
    pub object: Option<ObjectRef>,
    /// Whether this is an object level.
    pub objects: bool,
    /// The subnet open inside the object's network.
    pub subnet: Option<NodeRef>,
    /// The node the level is opened on (a Material's shading network, a
    /// mode such as CAD's sketch).
    pub node: Option<NodeRef>,
}

/// What [`Cx::node_rows`](super::Cx::node_rows) did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NodeRows {
    /// An edit of the rows was applied this call (the tool calls
    /// [`HostTools::ran`](super::HostTools::ran) again to keep adjusting
    /// the node).
    pub changed: bool,
    /// The rows drawn are fewer than the inspector's: a card offers More.
    pub hidden: bool,
}

/// One view's camera, for the edited object's space (the open object's,
/// so a package's tools work in object space wherever the object is).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewCamera {
    /// Object space to the view's pixels and back, for this view's
    /// rectangle.
    pub projection: Projection,
    /// Where the camera is, in object space.
    pub eye: Vec3,
    /// What it looks at, in object space.
    pub target: Vec3,
    /// Whether it projects orthographically.
    pub orthographic: bool,
    /// The view's ground: the plane new geometry lands on and its grid.
    pub ground: Ground,
}

/// A view's ground grid as it draws now, in the edited object's space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ground {
    /// A point on its plane.
    pub origin: Vec3,
    /// Its plane's normal (the project's up axis), length 1.
    pub normal: Vec3,
    /// Its finest lines' spacing at this zoom, in units (the project's
    /// spacing on a fixed grid): what grid snapping and drawn creation
    /// step by.
    pub step: f64,
}

/// One view as a package's viewport layer sees it this frame: what its
/// pointer handling needs, as plain data.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
    /// Where it is on screen.
    pub rect: Rect,
    /// Its index among the views (0 with one view).
    pub index: usize,
    /// It is the active view (the others only draw).
    pub interactive: bool,
    /// A menu, the Tab browser or a popover owns the input.
    pub modal: bool,
    /// The view's own response this frame (its clicks and drags).
    pub response: Response,
    /// The pointer is over the viewport's controls: core's, and the
    /// context's own ([`controls`](crate::package::ViewportLayer::controls),
    /// which isn't told its own).
    pub over_chrome: bool,
    /// The view cube took the pointer this frame.
    pub over_cube: bool,
    /// A handle Studio shows is under the pointer in this view, or one is
    /// being dragged: a live tool leaves the press to it.
    pub over_handle: bool,
    /// The pointer, when it is over the view.
    pub pointer: Option<Pos>,
    /// The modifiers held.
    pub modifiers: Modifiers,
    /// Ctrl+Shift: a click opens the list of everything under the pointer.
    pub stacking: bool,
    /// A painter slot under everything else drawn over the view (point
    /// labels' backdrops).
    pub under: Slot,
    /// Its camera.
    pub camera: ViewCamera,
}

impl View {
    /// The ray through screen point `p`, in the edited object's space.
    pub fn ray(&self, p: Pos) -> Ray {
        let (w, h) = (self.rect.width().max(1.0), self.rect.height().max(1.0));
        let uv = [
            f64::from((p.x - self.rect.min.x) / w),
            f64::from((p.y - self.rect.min.y) / h),
        ];
        self.camera.projection.ray(uv)
    }
}

/// What the inspector shows: the open level's active node, or the object
/// selected at an object level.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Inspected {
    /// The node inspected, in the open network.
    pub node: Option<NodeRef>,
    /// The object inspected, at an object level.
    pub object: Option<ObjectRef>,
}

/// The user's preferences a package's tools follow, as plain data: read
/// once a call, never stored by the package (they change between calls).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HostPrefs {
    /// The project's units.
    pub units: Units,
    /// Whether snapping is on (the Snap button; X).
    pub snapping: bool,
    /// The grid's spacing, in scene units.
    pub grid_spacing: f64,
    /// Rotate's snap angle, in degrees.
    pub rotate_step: f64,
    /// Scale's snap step.
    pub scale_step: f64,
    /// How near the pointer a snap target must be, in points.
    pub snap_radius: f32,
    /// The viewport's transform mode (Q, W, E, R, T).
    pub transform: TransformTool,
}

impl Default for HostPrefs {
    /// Millimetres, Y up, snapping off on a 10 mm grid, 15° and 0.1 steps,
    /// the Select tool.
    fn default() -> Self {
        HostPrefs {
            units: Units::MILLIMETERS,
            snapping: false,
            grid_spacing: 10.0,
            rotate_step: 15.0,
            scale_step: 0.1,
            snap_radius: 12.0,
            transform: TransformTool::Select,
        }
    }
}
