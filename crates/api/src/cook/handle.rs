//! [`Handle`]: a viewport handle a node type binds to its parameters.
use super::binding::Vector;
use super::frame::Frame;
use crate::params::{Parameters, Value};

/// What a handle edits, and so how it looks and drags. Places and
/// directions are in the handle's [`Frame`] when it has one, otherwise in
/// its [`Space`]; angles are in degrees. A shape's `center` names the
/// point of it the handle's [`Handle::anchor`] says (its middle unless
/// anchored).
#[derive(Clone, Debug, PartialEq)]
pub enum HandleKind {
    /// The Move gizmo at `position`: an arrow drags along its axis, a
    /// plane square in its plane, the middle in the view's plane.
    Translate {
        /// The place it moves.
        position: Vector,
    },
    /// The Rotate gizmo's rings about `pivot`: a ring adds its turn to
    /// its axis's angle in `angles` (about X, Y and Z).
    Rotate {
        /// The angles it turns.
        angles: Vector,
        /// Where the rings sit.
        pivot: Vector,
    },
    /// The Scale gizmo at `pivot`: a square scales its axis's factor in
    /// `factors`, a plane square the plane's two, the middle all three.
    Scale {
        /// The factors it multiplies.
        factors: Vector,
        /// Where the gizmo sits.
        pivot: Vector,
    },
    /// A dot at `position`, dragged in the view's plane: a pivot, an
    /// origin.
    Pivot {
        /// The place it moves.
        position: Vector,
    },
    /// A dot `distance` along `direction` from `origin`, dragged along
    /// that line: a depth, a length.
    Distance {
        /// The number parameter it sets.
        distance: String,
        /// Where the line starts.
        origin: Vector,
        /// Which way the line runs (any length).
        direction: Vector,
    },
    /// A ring of `radius` about `center`, facing the view, or lying in
    /// the plane square to an axis when the handle keeps to that plane
    /// ([`Constraint::Plane`]): a cylinder's radius on its base. Dragging
    /// it sets the radius to the pointer's distance from the ring's
    /// middle; an anchored ring grows from its anchor (a sphere resting on
    /// its base keeps the base where it is).
    Radius {
        /// The number parameter it sets.
        radius: String,
        /// The ring's middle.
        center: Vector,
    },
    /// A line from `origin` and a dot at its end, dragged to aim
    /// `direction` (written unit length).
    Direction {
        /// The direction it aims.
        direction: Vector,
        /// Where the line starts.
        origin: Vector,
    },
    /// A box of `size` about `center`: a dot on each face moves that face,
    /// and the dot in the middle, where `center` is bound to parameters,
    /// moves the box in the view's plane. Along an axis whose `center` is
    /// a parameter, the opposite face stays; along one whose `center` is
    /// fixed, the anchor stays (a box anchored at its middle grows on both
    /// sides, one anchored on its base grows upward). A face at the anchor
    /// of a fixed center has no dot, nor does an axis of fixed size (a
    /// flat grid's height).
    Box {
        /// The box's middle.
        center: Vector,
        /// Its size along x, y and z.
        size: Vector,
    },
    /// An ellipsoid of diameters `size` about `center`: a dot at each end
    /// of an axis sets that axis's size, the anchor staying where it is,
    /// and the dot in the middle, where `center` is bound to parameters,
    /// moves it in the view's plane. An end at the anchor has no dot.
    Sphere {
        /// The ellipsoid's middle.
        center: Vector,
        /// Its diameters along x, y and z.
        size: Vector,
    },
    /// The Rotate gizmo's rings at `frame`'s origin, along its axes: a
    /// ring turns the frame about its axis, writing its up and turn
    /// ([`Frame::orient`]), as Maya's rotate manipulator turns an object.
    /// The frame is placed in the handle's [`Space`]; [`Handle::frame`]
    /// does not move it. With a fixed turn, the Y ring is not there.
    Orient {
        /// The frame it turns.
        frame: Frame,
    },
}

impl HandleKind {
    /// What it reads and writes, in order.
    fn vectors(&self) -> Vec<&Vector> {
        match self {
            Self::Translate { position } | Self::Pivot { position } => vec![position],
            Self::Rotate { angles: v, pivot } | Self::Scale { factors: v, pivot } => {
                vec![v, pivot]
            }
            Self::Distance {
                origin, direction, ..
            } => vec![origin, direction],
            Self::Radius { center, .. } => vec![center],
            Self::Direction { direction, origin } => vec![direction, origin],
            Self::Box { center, size } | Self::Sphere { center, size } => vec![center, size],
            Self::Orient { frame } => vec![&frame.position, &frame.up],
        }
    }
}

/// Which space a handle's places and directions are in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Space {
    /// The edited object's own space, where its data is.
    #[default]
    Object,
    /// The scene's: the handle stays put while the object moves.
    World,
}

/// Which of a handle's moves it keeps. Axes are 0, 1 and 2 (x, y and z of
/// its [`Space`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Constraint {
    /// Every move its kind has.
    #[default]
    Free,
    /// Along one axis only: a gizmo shows that axis's arrow, ring or
    /// square; a dot moves along the axis.
    Axis(usize),
    /// In the plane normal to one axis only: a gizmo shows that plane's
    /// handles; a dot moves in the plane.
    Plane(usize),
}

/// A viewport handle a node type binds to its parameters, as
/// Houdini's handle bindings and Maya's manipulators do.
///
/// A node type lists its handles in its [`Description`](super::Description);
/// Studio runs them with no package code: while the node is selected and
/// its context is open, Studio draws each handle whose [`Handle::when`]
/// holds, hit-tests it, snaps its drags with the user's snapping, writes
/// the bound parameters as the pointer moves (the node recooks through
/// the usual cooking, incrementally where the node type is
/// [`Incremental`](super::Incremental)) and records the whole drag as one
/// undo step.
///
/// The transform kinds use Studio's transform gizmo: [`HandleKind::Translate`]
/// its Move arrows, plane squares and middle, [`HandleKind::Rotate`] and
/// [`HandleKind::Orient`] its rings, [`HandleKind::Scale`] its squares; a
/// [`Constraint`] keeps the handles of one axis or one plane. Each shows
/// while the user's matching transform tool is active (Move, Rotate,
/// Scale), in place of that tool's own gizmo, as Maya's manipulators do.
/// The other kinds are dots and rings on the shape they edit, shown
/// whatever the tool.
///
/// A handle in a [`Frame`] ([`Handle::frame`]) has its places, directions
/// and constraint axes in that frame: a primitive placed by Position, Up
/// and Turn binds its size handles in its own frame, so they sit on it
/// however it is placed. Its [`Handle::anchor`] says which point of a
/// shape its center names: a box built upward from its base is anchored
/// at the base.
///
/// A tool that needs more than a binding (one that draws a stroke, cuts,
/// or picks components) draws its own handles with the UI API's gizmo
/// service instead.
///
/// ```
/// use kinogaki_studio_api::cook::{Handle, HandleKind, Vector};
/// use kinogaki_studio_api::Value;
/// let bounds = Handle::new(
///    "bounds",
///    HandleKind::Box {
///        center: Vector::components("center_x", "center_y", "center_z"),
///        size: Vector::parameter("size"),
///    },
/// )
/// .when("bounded", true);
/// let mut values = kinogaki_studio_api::Parameters::new();
/// values.insert("bounded".into(), Value::Bool(false));
/// assert!(!bounds.shown(&values));
/// assert_eq!(bounds.parameters(), ["center_x", "center_y", "center_z", "size"]);
///
/// // A box built up from its base, in the frame of its placement.
/// use kinogaki_studio_api::cook::Frame;
/// let placed = Frame::new(Vector::parameter("position"), Vector::parameter("up"), "turn");
/// let size = Handle::new(
///    "size",
///    HandleKind::Box {
///        center: Vector::fixed([0.0; 3]),
///        size: Vector::components("width", "height", "depth"),
///    },
/// )
/// .frame(placed)
/// .anchor([0.0, -1.0, 0.0]);
/// assert_eq!(
///     size.parameters(),
///     ["width", "height", "depth", "position", "up", "turn"]
/// );
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct Handle {
    /// What it edits.
    pub kind: HandleKind,
    /// What it is called in its drag's undo step ("Move bounds").
    pub label: String,
    /// The space its places are in (and its frame's, if it has one).
    pub space: Space,
    /// The frame its places, directions and constraint axes are in,
    /// placed in its space; `None`: the space's own axes. Dragging never
    /// writes the frame's parameters (an [`HandleKind::Orient`] turns its
    /// own frame).
    pub frame: Option<Frame>,
    /// Which point of a [`HandleKind::Box`], [`HandleKind::Sphere`] or
    /// [`HandleKind::Radius`] its `center` names, in half sizes (radii)
    /// from the shape's middle along each axis: `[0, 0, 0]` the middle,
    /// `[0, -1, 0]` the bottom. The middle is `center − anchor × half
    /// size`. Other kinds ignore it.
    pub anchor: [f64; 3],
    /// Which moves it keeps.
    pub constraint: Constraint,
    /// The smallest and largest value it writes to a number (a distance,
    /// a radius, a size, a factor), within the parameter's own range.
    pub range: Option<[f64; 2]>,
    /// Parameter values it shows for, all of them (a bounding box handle
    /// while Bounding Type is Box); empty: always.
    pub when: Vec<(String, Value)>,
}

impl Handle {
    /// A `kind` handle called `label`, in object space with no frame,
    /// anchored at its middle, free, shown always.
    pub fn new(label: impl Into<String>, kind: HandleKind) -> Self {
        Self {
            kind,
            label: label.into(),
            space: Space::Object,
            frame: None,
            anchor: [0.0; 3],
            constraint: Constraint::Free,
            range: None,
            when: Vec::new(),
        }
    }

    /// In `space`.
    pub fn space(mut self, space: Space) -> Self {
        self.space = space;
        self
    }

    /// In `frame`.
    pub fn frame(mut self, frame: Frame) -> Self {
        self.frame = Some(frame);
        self
    }

    /// With its center naming point `anchor` of its shape.
    pub fn anchor(mut self, anchor: [f64; 3]) -> Self {
        self.anchor = anchor;
        self
    }

    /// Along `axis` only.
    pub fn axis(mut self, axis: usize) -> Self {
        self.constraint = Constraint::Axis(axis);
        self
    }

    /// In the plane normal to `axis` only.
    pub fn plane(mut self, axis: usize) -> Self {
        self.constraint = Constraint::Plane(axis);
        self
    }

    /// Writing numbers from `min` to `max` only.
    pub fn range(mut self, min: f64, max: f64) -> Self {
        self.range = Some([min, max]);
        self
    }

    /// Shown only while parameter `name` holds `value` (and every other
    /// such condition holds).
    pub fn when(mut self, name: impl Into<String>, value: impl Into<Value>) -> Self {
        self.when.push((name.into(), value.into()));
        self
    }

    /// Whether it shows for `parameters`.
    pub fn shown(&self, parameters: &Parameters) -> bool {
        self.when
            .iter()
            .all(|(name, value)| parameters.get(name) == Some(value))
    }

    /// The parameters it reads and writes, in order, then its frame's.
    pub fn parameters(&self) -> Vec<&str> {
        let number = match &self.kind {
            HandleKind::Distance { distance: n, .. } | HandleKind::Radius { radius: n, .. } => {
                Some(n.as_str())
            }
            _ => None,
        };
        let turn = match &self.kind {
            HandleKind::Orient { frame } => frame.turn.name(),
            _ => None,
        };
        number
            .into_iter()
            .chain(self.kind.vectors().into_iter().flat_map(Vector::names))
            .chain(turn)
            .chain(self.frame.iter().flat_map(Frame::names))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_handle_shows_while_every_condition_holds() {
        let handle = Handle::new(
            "radius",
            HandleKind::Radius {
                radius: "radius".into(),
                center: Vector::fixed([0.0; 3]),
            },
        )
        .when("on", true)
        .when("shape", Value::Index(1))
        .range(0.0, 10.0)
        .plane(2);
        let mut values = Parameters::new();
        values.insert("on".into(), Value::Bool(true));
        values.insert("shape".into(), Value::Index(0));
        assert!(!handle.shown(&values));
        values.insert("shape".into(), Value::Index(1));
        assert!(handle.shown(&values));
        assert_eq!(handle.parameters(), ["radius"]);
        assert_eq!(handle.constraint, Constraint::Plane(2));
        assert_eq!(handle.range, Some([0.0, 10.0]));
    }

    /// A handle in a frame reads the frame's parameters after its own; an
    /// Orient reads its frame's position, up and turn.
    #[test]
    fn frames_and_anchors_bind_with_the_handle() {
        let frame = Frame::new(
            Vector::components("px", "py", "pz"),
            Vector::components("ux", "uy", "uz"),
            "turn",
        );
        let top = Handle::new(
            "top",
            HandleKind::Radius {
                radius: "top".into(),
                center: Vector::components(0.0, "height", 0.0),
            },
        )
        .frame(frame.clone())
        .anchor([0.0, -1.0, 0.0])
        .plane(1);
        assert_eq!(
            top.parameters(),
            ["top", "height", "px", "py", "pz", "ux", "uy", "uz", "turn"]
        );
        assert_eq!(top.anchor, [0.0, -1.0, 0.0]);
        assert_eq!(top.frame.as_ref(), Some(&frame));
        let orient = Handle::new("orientation", HandleKind::Orient { frame });
        assert_eq!(
            orient.parameters(),
            ["px", "py", "pz", "ux", "uy", "uz", "turn"]
        );
        assert_eq!((orient.frame, orient.anchor), (None, [0.0; 3]));
    }
}
