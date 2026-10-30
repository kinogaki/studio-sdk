//! [`Description`]: what a node type is to the user and to a network: its
//! label and icon, its ports, its parameters and the viewport handles
//! bound to them. The environment reads it once, when the package registers, and
//! builds node creation, the network's ports, the inspector and the
//! handles from it.
use super::handle::Handle;
use crate::id::DataTypeId;
use crate::params::Schema;

/// A node type's label, icon, ports, parameters and handles.
///
/// ```
/// use kinogaki_studio_api::cook::{Description, Port};
/// use kinogaki_studio_api::params::{Number, Parameter, ParameterKind, Schema};
/// use kinogaki_studio_api::DataTypeId;
/// let points = DataTypeId::declared("acme.points.cloud");
/// let jitter = Description::new("Jitter")
///     .input(Port::new("points", "Points", points).missing("Wire points into Jitter"))
///     .output(Port::new("points", "Points", points))
///     .parameters(Schema::default().parameter(Parameter::new(
///         "amount",
///         "Amount",
///         ParameterKind::Number(Number::length().range(0.0, f64::INFINITY)),
///         0.1,
///     )));
/// assert_eq!(jitter.parameters.values()["amount"].number(), Some(0.1));
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct Description {
    /// What node creation and a new node call it ("Extrude").
    pub label: String,
    /// Its icon's name, where Studio shows a node by an icon (its row in
    /// the outliner): Studio's (`"cube"`) or a package's
    /// (`"acme.trees/leaf"`, [`icon`](crate::icon)); empty for none.
    pub icon: String,
    /// Its inputs, in port order.
    pub inputs: Vec<Port>,
    /// Its outputs, in port order; output 0 is its result.
    pub outputs: Vec<Port>,
    /// Its parameters as the inspector shows them, at their defaults: a
    /// new node's [`Parameters`](crate::Parameters) are
    /// [`Schema::values`].
    pub parameters: Schema,
    /// The viewport handles Studio shows while a node of this type is
    /// selected, each bound to some of its parameters.
    pub handles: Vec<Handle>,
}

impl Description {
    /// A node type called `label`, with no icon, ports, parameters or
    /// handles.
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            icon: String::new(),
            inputs: Vec::new(),
            outputs: Vec::new(),
            parameters: Schema::default(),
            handles: Vec::new(),
        }
    }

    /// With icon `name`.
    pub fn icon(mut self, name: impl Into<String>) -> Self {
        self.icon = name.into();
        self
    }

    /// With input `port` next.
    pub fn input(mut self, port: Port) -> Self {
        self.inputs.push(port);
        self
    }

    /// With output `port` next.
    pub fn output(mut self, port: Port) -> Self {
        self.outputs.push(port);
        self
    }

    /// With `parameters`.
    pub fn parameters(mut self, parameters: Schema) -> Self {
        self.parameters = parameters;
        self
    }

    /// With `handle` next.
    pub fn handle(mut self, handle: Handle) -> Self {
        self.handles.push(handle);
        self
    }
}

/// One input or output of a node type.
#[derive(Clone, Debug, PartialEq)]
pub struct Port {
    /// Its stable key (`geometry`, `cut_away`), unique among the node's
    /// inputs or outputs; projects save wires under it.
    pub name: String,
    /// What its hover shows ("Surface", "Result").
    pub label: String,
    /// The data type an input takes or an output carries.
    pub data: DataTypeId,
    /// An input taking any number of wires, read in wire order
    /// ([`CookCx::port`](crate::CookCx::port)).
    pub multi: bool,
    /// An input that may stay unwired.
    pub optional: bool,
    /// Why the node can't cook while nothing is wired into a required
    /// input; empty says it plainly.
    pub missing: String,
}

impl Port {
    /// Port `name` labelled `label`, of `data`: single and required.
    pub fn new(name: impl Into<String>, label: impl Into<String>, data: DataTypeId) -> Self {
        Self {
            name: name.into(),
            label: label.into(),
            data,
            multi: false,
            optional: false,
            missing: String::new(),
        }
    }

    /// Taking many wires.
    pub fn multi(mut self) -> Self {
        self.multi = true;
        self
    }

    /// May stay unwired.
    pub fn optional(mut self) -> Self {
        self.optional = true;
        self
    }

    /// Saying `missing` while a required input is unwired.
    pub fn missing(mut self, missing: impl Into<String>) -> Self {
        self.missing = missing.into();
        self
    }
}
