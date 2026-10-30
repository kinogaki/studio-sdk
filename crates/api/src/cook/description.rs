//! [`Description`]: what a node type is to the user and to a network: its
//! label and icon, its ports, its parameters and the viewport handles
//! bound to them. The environment reads it once, when the package registers, and
//! builds node creation, the network's ports, the inspector and the
//! handles from it.
use super::handle::Handle;
use crate::id::{ContextId, DataTypeId};
use crate::params::{Schema, Value};

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
    /// The contexts whose networks offer it (Tab, menus): a Mesh tool's
    /// `kinogaki.mesh.network`, and CAM's too when CAM cooks it.
    pub contexts: Vec<ContextId>,
    /// What part it plays in its network beyond what it computes.
    pub role: NodeRole,
    /// Where Tab, menus and shelves list it in its contexts; `None`: not
    /// listed (a node only code or a file adds).
    pub listing: Option<Listing>,
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
            contexts: Vec::new(),
            role: NodeRole::Plain,
            listing: None,
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

    /// Offered in networks of `context` too.
    pub fn context(mut self, context: ContextId) -> Self {
        self.contexts.push(context);
        self
    }

    /// Playing `role`.
    pub fn role(mut self, role: NodeRole) -> Self {
        self.role = role;
        self
    }

    /// Listed in Tab, menus and shelves under `section` ("Filter", its
    /// tile colored as `category`), in `family` within it, found by search
    /// under its label and `aliases` too (other applications' names).
    pub fn listed(
        mut self,
        section: impl Into<String>,
        category: Category,
        family: impl Into<String>,
        aliases: &[&str],
    ) -> Self {
        self.listing = Some(Listing {
            section: section.into(),
            category,
            family: family.into(),
            aliases: aliases.iter().map(|&a| a.to_owned()).collect(),
        });
        self
    }
}

/// Where Tab, menus and shelves list a node type.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Listing {
    /// Its heading in Tab and the menus ("Filter").
    pub section: String,
    /// What it does, as its tile's color says.
    pub category: Category,
    /// Its family within the section ("Blur").
    pub family: String,
    /// More words search finds it by.
    pub aliases: Vec<String>,
}

/// What a node does, as its tile's color says; every context's sections
/// map onto these.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Category {
    /// Makes something from nothing (a primitive, a render's image).
    Create,
    /// Reads without changing (a measure, a viewer).
    Inspect,
    /// Changes what it reads.
    Modify,
    /// Builds from what it reads (a loft, a merge).
    Construct,
    /// Moves, turns or scales.
    Transform,
    /// Shapes a surface's look (normals, UVs, a grade).
    Surface,
    /// Plumbing: switches, nulls, outputs.
    Utility,
}

/// The part a node plays in its network beyond what it computes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NodeRole {
    /// An ordinary node.
    #[default]
    Plain,
    /// Its context's Merge: its first input gathers branches, and growing
    /// a network adds one where branches join.
    Merge,
    /// What its network yields to its object, whatever the display flag
    /// shows; it stays on the network's top level.
    Output,
}

/// A node type's inputs and outputs, when they follow its parameters
/// ([`NodeType::ports`](crate::NodeType::ports)).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Ports {
    /// Its inputs, in port order.
    pub inputs: Vec<Port>,
    /// Its outputs, in port order; output 0 is its result.
    pub outputs: Vec<Port>,
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
    /// An optional input that is required while parameter `.0` holds
    /// `.1` (a Boolean's Tools while its operation subtracts).
    pub required_when: Option<(String, Value)>,
    /// An input that, unwired, reads the parameter of the same name
    /// instead (a shader socket's constant).
    pub constant: bool,
    /// An input taking another branch's whole result: a branch picked in
    /// the viewport while the node's tool is live wires into it.
    pub operand: bool,
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
            required_when: None,
            constant: false,
            operand: false,
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

    /// Optional, but required while parameter `parameter` holds `value`.
    pub fn required_when(mut self, parameter: impl Into<String>, value: impl Into<Value>) -> Self {
        self.optional = true;
        self.required_when = Some((parameter.into(), value.into()));
        self
    }

    /// Reading the parameter of its own name while unwired.
    pub fn constant(mut self) -> Self {
        self.constant = true;
        self
    }

    /// Taking another branch's whole result, wired by a branch picked in
    /// the viewport while the node's tool is live.
    pub fn operand(mut self) -> Self {
        self.operand = true;
        self
    }

    /// Whether a node with `parameters` needs a wire here.
    pub fn required(&self, parameters: &crate::Parameters) -> bool {
        match &self.required_when {
            Some((name, value)) => parameters.get(name) == Some(value),
            None => !self.optional,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_listed_type_says_where_tab_shows_it() {
        let blur =
            Description::new("Blur").listed("Filter", Category::Modify, "Blur", &["Defocus"]);
        assert_eq!(
            blur.listing,
            Some(Listing {
                section: "Filter".into(),
                category: Category::Modify,
                family: "Blur".into(),
                aliases: vec!["Defocus".into()],
            })
        );
        assert_eq!(Description::new("Internal").listing, None);
    }
}
