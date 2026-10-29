//! [`Handle`]: a viewport handle a node type binds to its parameters.
use crate::params::{Parameters, Value};

/// Three numbers a handle reads and writes: a place or a direction.
#[derive(Clone, Debug, PartialEq)]
pub enum Vector {
    /// One parameter of three numbers ([`Value::Numbers`], a
    /// [`ParameterKind::Vector`](crate::params::ParameterKind::Vector)).
    Parameter(String),
    /// Three number parameters: x, y and z.
    Components([String; 3]),
    /// A fixed value, not a parameter (a shape always centered on the
    /// origin). Dragging it changes nothing.
    Fixed([f64; 3]),
}

impl Vector {
    /// Parameter `name`, three numbers.
    pub fn parameter(name: impl Into<String>) -> Self {
        Self::Parameter(name.into())
    }

    /// Number parameters `x`, `y` and `z`.
    pub fn components(x: impl Into<String>, y: impl Into<String>, z: impl Into<String>) -> Self {
        Self::Components([x.into(), y.into(), z.into()])
    }

    /// The fixed `value`.
    pub fn fixed(value: [f64; 3]) -> Self {
        Self::Fixed(value)
    }

    /// The parameters it names (none when fixed).
    pub fn names(&self) -> Vec<&str> {
        match self {
            Self::Parameter(name) => vec![name],
            Self::Components(names) => names.iter().map(String::as_str).collect(),
            Self::Fixed(_) => Vec::new(),
        }
    }

    /// Its value in `parameters`; `None` when a parameter is missing or
    /// holds something else.
    pub fn read(&self, parameters: &Parameters) -> Option<[f64; 3]> {
        match self {
            Self::Parameter(name) => match parameters.get(name)?.numbers()? {
                &[x, y, z] => Some([x, y, z]),
                _ => None,
            },
            Self::Components([x, y, z]) => Some([
                parameters.get(x)?.number()?,
                parameters.get(y)?.number()?,
                parameters.get(z)?.number()?,
            ]),
            Self::Fixed(value) => Some(*value),
        }
    }

    /// Writes `value` into `parameters` (nothing when fixed).
    pub fn write(&self, parameters: &mut Parameters, value: [f64; 3]) {
        match self {
            Self::Parameter(name) => {
                parameters.insert(name.clone(), Value::Numbers(value.to_vec()));
            }
            Self::Components(names) => {
                for (name, v) in names.iter().zip(value) {
                    parameters.insert(name.clone(), Value::Number(v));
                }
            }
            Self::Fixed(_) => {}
        }
    }
}

/// What a handle edits, and so how it looks and drags. Places are in the
/// handle's [`Space`]; angles are in degrees.
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
    /// A ring of `radius` about `center`, facing the view: dragging it
    /// sets the radius to the pointer's distance from the center.
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
    /// A box of `size` about `center`: a dot on each face moves that face
    /// (the opposite one stays), and the dot in the middle moves the box
    /// in the view's plane.
    Box {
        /// The box's middle.
        center: Vector,
        /// Its size along x, y and z.
        size: Vector,
    },
    /// An ellipsoid of diameters `size` about `center`: a dot at each end
    /// of an axis sets that axis's size, and the dot in the middle moves
    /// it in the view's plane.
    Sphere {
        /// The ellipsoid's middle.
        center: Vector,
        /// Its diameters along x, y and z.
        size: Vector,
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
/// its Move arrows, plane squares and middle, [`HandleKind::Rotate`] its
/// rings, [`HandleKind::Scale`] its squares; a [`Constraint`] keeps the
/// handles of one axis or one plane. The other kinds are dots on the
/// shape they edit.
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
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct Handle {
    /// What it edits.
    pub kind: HandleKind,
    /// What it is called in its drag's undo step ("Move bounds").
    pub label: String,
    /// The space its places are in.
    pub space: Space,
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
    /// A `kind` handle called `label`, in object space, free, shown
    /// always.
    pub fn new(label: impl Into<String>, kind: HandleKind) -> Self {
        Self {
            kind,
            label: label.into(),
            space: Space::Object,
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

    /// The parameters it reads and writes, in order.
    pub fn parameters(&self) -> Vec<&str> {
        let number = match &self.kind {
            HandleKind::Distance { distance: n, .. } | HandleKind::Radius { radius: n, .. } => {
                Some(n.as_str())
            }
            _ => None,
        };
        number
            .into_iter()
            .chain(self.kind.vectors().into_iter().flat_map(Vector::names))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vectors_read_and_write_one_or_three_parameters() {
        let mut values = Parameters::new();
        let one = Vector::parameter("center");
        let three = Vector::components("x", "y", "z");
        assert_eq!(one.read(&values), None);
        one.write(&mut values, [1.0, 2.0, 3.0]);
        three.write(&mut values, [4.0, 5.0, 6.0]);
        assert_eq!(one.read(&values), Some([1.0, 2.0, 3.0]));
        assert_eq!(three.read(&values), Some([4.0, 5.0, 6.0]));
        assert_eq!(values["y"], Value::Number(5.0));
        let fixed = Vector::fixed([0.0; 3]);
        fixed.write(&mut values, [9.0; 3]);
        assert_eq!(fixed.read(&values), Some([0.0; 3]));
        values.insert("center".into(), Value::Numbers(vec![1.0]));
        assert_eq!(one.read(&values), None, "a vector has three numbers");
    }

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
}
