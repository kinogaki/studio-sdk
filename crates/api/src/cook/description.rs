//! [`Description`]: what a node type is to the user and to a network: its
//! label, its ports and its parameters. The environment reads it once,
//! when the package registers, and builds node creation, the network's
//! ports and the inspector from it.
use crate::id::DataTypeId;
use crate::params::Schema;

/// A node type's label, ports and parameters.
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
    /// Its inputs, in port order.
    pub inputs: Vec<Port>,
    /// Its outputs, in port order; output 0 is its result.
    pub outputs: Vec<Port>,
    /// Its parameters as the inspector shows them, at their defaults: a
    /// new node's [`Parameters`](crate::Parameters) are
    /// [`Schema::values`].
    pub parameters: Schema,
}

impl Description {
    /// A node type called `label`, with no ports and no parameters.
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            inputs: Vec::new(),
            outputs: Vec::new(),
            parameters: Schema::default(),
        }
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
