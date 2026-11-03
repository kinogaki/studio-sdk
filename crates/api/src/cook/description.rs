//! [`Description`]: what a node type is to the user and to a network: its
//! label and icon, its ports, its parameters and the viewport handles
//! bound to them. The environment reads it once, when the package registers, and
//! builds node creation, the network's ports, the inspector and the
//! handles from it.
use super::handle::Handle;
use crate::id::{ContextId, DataTypeId};
use crate::params::Schema;
use kinogaki_value::Condition;

/// A node type's label, icon, ports, parameters and handles.
///
/// ```
/// use kinogaki_studio_api::cook::{Description, Port};
/// use kinogaki_studio_api::params::{Number, Parameter, ValueKind, Schema};
/// use kinogaki_studio_api::DataTypeId;
/// let points = DataTypeId::declared("acme.points.cloud");
/// let jitter = Description::new("Jitter")
///     .input(Port::new("points", "Points", points).missing("Wire points into Jitter"))
///     .output(Port::new("points", "Points", points))
///     .parameters(Schema::default().parameter(Parameter::new(
///         "amount",
///         "Amount",
///         ValueKind::Number(Number::length().range(0.0, f64::INFINITY)),
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
    /// `kinogaki.mesh.network`. Each is its package's or an earlier
    /// package's (a later package offers it through
    /// [`NetworkContext::adopts`](crate::NetworkContext::adopts)).
    pub contexts: Vec<ContextId>,
    /// What part it plays in its network beyond what it computes.
    pub role: NodeRole,
    /// Where Tab, menus and shelves list it in its contexts; `None`: not
    /// listed (a node only code or a file adds).
    pub listing: Option<Listing>,
    /// The context of the network a node of this type holds inside it (a
    /// Material's Shading network); `None`: none (a subnet holds its own
    /// context's levels). A new node gets it empty; core keeps, saves,
    /// copies and navigates it, and the type's own cook doesn't read it.
    pub inside: Option<ContextId>,
    /// Which networks offer and keep its nodes beyond its
    /// [`contexts`](Self::contexts).
    pub reach: Reach,
}

/// Which networks offer a node type's nodes (Tab, menus, adds, pastes) and
/// keep them (a file read, a paste), beyond the contexts it lists and
/// those adopting it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Reach {
    /// Its contexts' networks alone: offered and kept there.
    #[default]
    Contexts,
    /// Every network, offered and kept: a node that holds any value (a
    /// stash of what an import made).
    Everywhere,
    /// Offered where an installed format reads into what the network
    /// carries, kept in every network (it stays when its formats go): a
    /// node reading files.
    Files,
    /// Kept in every network but the one it holds
    /// ([`inside`](Description::inside)), offered in its contexts: a
    /// holder carried with what it is used in (a Material).
    Kept,
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
            inside: None,
            reach: Reach::Contexts,
        }
    }

    /// Offered and kept as `reach` says.
    pub fn reach(mut self, reach: Reach) -> Self {
        self.reach = reach;
        self
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

    /// Holding a network of `context` inside each node (a Material its
    /// Shading network).
    pub fn holding(mut self, context: ContextId) -> Self {
        self.inside = Some(context);
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
    /// How many wires an input takes.
    pub arity: Arity,
    /// Whether an input needs a wire ([`required`](Self::required)).
    need: Need,
    /// Why the node can't cook while nothing is wired into a required
    /// input; empty says it plainly.
    pub missing: String,
    /// What an unwired input reads.
    pub unwired: Unwired,
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
            arity: Arity::One,
            need: Need::Required,
            missing: String::new(),
            unwired: Unwired::Nothing,
            operand: false,
        }
    }

    /// Taking many wires.
    pub fn multi(mut self) -> Self {
        self.arity = Arity::Many;
        self
    }

    /// May stay unwired.
    pub fn optional(mut self) -> Self {
        self.need = Need::Optional;
        self
    }

    /// Saying `missing` while a required input is unwired.
    pub fn missing(mut self, missing: impl Into<String>) -> Self {
        self.missing = missing.into();
        self
    }

    /// Optional, but required while `when` holds.
    pub fn required_when(mut self, when: Condition) -> Self {
        self.need = Need::When(when);
        self
    }

    /// Reading the parameter of its own name while unwired.
    pub fn constant(mut self) -> Self {
        self.unwired = Unwired::Parameter;
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
        match &self.need {
            Need::Required => true,
            Need::Optional => false,
            Need::When(when) => when.met(parameters),
        }
    }
}

/// How many wires an input takes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Arity {
    /// One at most.
    #[default]
    One,
    /// Any number, read in wire order ([`CookCx::port`](crate::CookCx::port)).
    Many,
}

/// Whether an input needs a wire for its node to cook.
#[derive(Clone, Debug, Default, PartialEq)]
enum Need {
    /// Always.
    #[default]
    Required,
    /// Never: it may stay unwired.
    Optional,
    /// While this holds (a Boolean's Tools while its operation
    /// subtracts); optional otherwise.
    When(Condition),
}

/// What an unwired input reads.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Unwired {
    /// Nothing.
    #[default]
    Nothing,
    /// The parameter of its own name (a shader socket's constant).
    Parameter,
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
