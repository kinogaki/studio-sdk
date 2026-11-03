//! [`NetworkEdit`]: the edits a package makes to a network.
use super::{Flag, NetworkRead, Wire};
use crate::Key;
use crate::cook::Outputs;
use crate::data::Data;
use crate::error::Error;
use crate::id::{CreationId, NodeTypeId};
use crate::params::{Parameters, Value};
use crate::refs::NodeRef;
use kinogaki_geometry::Time;

/// A network a package edits through the host, inside one undo step. Each
/// call refuses what doesn't fit (an unknown node, a wire into a port the
/// node hasn't got, a value its parameter doesn't take
/// ([`Parameter::check`](crate::params::Parameter::check)), a flag its
/// context doesn't offer) and changes nothing then; the host checks the
/// whole network again when the step is committed (no cycles, required
/// values) and drops the step when that fails.
pub trait NetworkEdit: NetworkRead {
    /// Adds `node` on the level inside `subnet` (`None`: the top level),
    /// placed and wired as `place` says, named as nothing on that level
    /// is: a new node of its type with its parameters set over the type's
    /// defaults (a [real size](crate::params::Number::real_size) in the
    /// document's unit), displayed when `node.display` says so (a grown
    /// node where the growth rule puts the flag).
    fn add_node_in(
        &mut self,
        subnet: Option<NodeRef>,
        node: NewNode,
        place: Place,
    ) -> Result<NodeRef, Error>;

    /// Adds `node` on the top level at `position`, wired to nothing
    /// ([`add_node_in`](Self::add_node_in) with [`Place::At`]).
    fn add_node(&mut self, node: NewNode, position: [f32; 2]) -> Result<NodeRef, Error> {
        self.add_node_in(None, node, Place::At(position))
    }

    /// Renames `node` to `name`, numbered when its level has a node so
    /// named. Paths naming it follow.
    fn rename(&mut self, node: NodeRef, name: &str) -> Result<(), Error>;

    /// Moves `node`'s tile (or a subnet's) to `position`; nothing it
    /// computes changes.
    fn set_position(&mut self, node: NodeRef, position: [f32; 2]) -> Result<(), Error>;

    /// Sets parameter `name` of `node` to `value`: on an animated row, its
    /// key at the current time (a new one between keys).
    fn set_parameter(&mut self, node: NodeRef, name: &str, value: Value) -> Result<(), Error>;

    /// Replaces every parameter of `node`: `parameters` hold each one its
    /// type declares. An animated row it changes is its key at the current
    /// time, as [`set_parameter`](Self::set_parameter) sets it.
    fn set_parameters(&mut self, node: NodeRef, parameters: Parameters) -> Result<(), Error>;

    /// Wires `from` into input `port` of `node`: after its wires on a
    /// multi-input port, in place of its wire on another.
    fn connect(&mut self, from: Wire, node: NodeRef, port: usize) -> Result<(), Error>;

    /// Removes wire `wire` (in wire order) of input `port` of `node`.
    fn disconnect(&mut self, node: NodeRef, port: usize, wire: usize) -> Result<(), Error>;

    /// Sets flag `flag` of `node`. Freezing goes through
    /// [`freeze`](Self::freeze), which needs the outputs to keep; turning
    /// [`Flag::Freeze`] off unfreezes.
    fn set_flag(&mut self, node: NodeRef, flag: Flag, on: bool) -> Result<(), Error>;

    /// Moves the display flag of `node`'s level to it (`None`: nothing on
    /// the top level is displayed).
    fn set_display(&mut self, node: Option<NodeRef>) -> Result<(), Error>;

    /// Deletes `node`. When its context heals chains, what it fed takes
    /// its first input's wire, and so does the display flag.
    fn remove(&mut self, node: NodeRef) -> Result<(), Error>;

    /// Makes `node` hold `delta` over its input: its type must be a held
    /// edit ([`NodeType::held`](crate::NodeType::held)) of `delta`'s data
    /// type. The base it applies to is captured by the next cook; a node
    /// holding a delta already applies the new one to the same base.
    fn set_delta(&mut self, node: NodeRef, delta: Data) -> Result<(), Error>;
    /// Makes `base` the value held node `node` applies its delta to, in
    /// place of the base it captured, keeping the delta: Bake (the node's
    /// result becomes its base, with an emptied delta set after) and
    /// re-keying a base after the input's topology changed. `base` is of
    /// the data type the node edits; `Err` when `node` is no held edit.
    fn rebase_held(&mut self, node: NodeRef, base: Data) -> Result<(), Error>;
    /// Edits the network node `node` holds inside it
    /// ([`Description::inside`](crate::cook::Description::inside)), within
    /// this edit (one undo step); `Err` when its type holds none.
    fn edit_inside(
        &mut self,
        node: NodeRef,
        edit: &mut dyn FnMut(&mut dyn NetworkEdit) -> Result<(), Error>,
    ) -> Result<(), Error> {
        let _ = (node, edit);
        Err(Error::invalid(
            "this network holds no networks inside its nodes",
        ))
    }

    /// Sets `key` on parameter `row` of `node`, replacing its key at that
    /// time: the row is animated from then on, its value at each moment
    /// its keys'. Refused for a row its type doesn't let animate
    /// ([`Parameter::animatable`](crate::params::Parameter::animatable)),
    /// a value the row doesn't take and a row a subnet's parameter drives
    /// (a row has one driver).
    fn set_key(&mut self, node: NodeRef, row: &str, key: Key) -> Result<(), Error> {
        let _ = (node, row, key);
        Err(Error::invalid("this network's parameters don't animate"))
    }

    /// Removes the key at `time` of parameter `row` of `node`; removing
    /// its last key is [`clear_channel`](Self::clear_channel).
    fn remove_key(&mut self, node: NodeRef, row: &str, time: Time) -> Result<(), Error> {
        let _ = (node, row, time);
        Err(Error::invalid("this network's parameters don't animate"))
    }

    /// Stops animating parameter `row` of `node`: it keeps the value it
    /// has at the scene's current time.
    fn clear_channel(&mut self, node: NodeRef, row: &str) -> Result<(), Error> {
        let _ = (node, row);
        Err(Error::invalid("this network's parameters don't animate"))
    }

    /// Freezes `node` with `outputs`, the outputs it has now (one value
    /// per output port): nothing upstream cooks for it until it is
    /// unfrozen.
    fn freeze(&mut self, node: NodeRef, outputs: Outputs) -> Result<(), Error>;
}

/// Where [`NetworkEdit::add_node_in`] puts a new node, and what it wires.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Place {
    /// At `position`, wired to nothing.
    At([f32; 2]),
    /// At `position`, after `node`: the new node reads `node`'s first
    /// output into its first input (when it has inputs), and every wire
    /// that read that output reads the new node instead.
    After {
        /// The node it follows.
        node: NodeRef,
        /// Where its tile goes.
        position: [f32; 2],
    },
    /// By the host's growth rule from what the level displays, placed on
    /// the host's grid (as the viewport's tools grow a network).
    Grow(Growth),
}

/// How a node added by [`Place::Grow`] joins what its level displays.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Growth {
    /// It reads the displayed result: inserted after the displayed node,
    /// it takes the display flag and that node's readers.
    Modify,
    /// It reads nothing of the displayed result: it starts a branch that
    /// the level's Merge gathers (the displayed Merge, else a new one
    /// after the displayed node), and the flag ends on what shows
    /// everything shown before plus the new node.
    Create,
}

/// A node to add ([`NetworkEdit::add_node_in`]).
#[derive(Clone, Debug, PartialEq)]
pub struct NewNode {
    /// Its type.
    pub node_type: NodeTypeId,
    /// Its name; `None`: its type's label, numbered as nothing on its level
    /// is named.
    pub name: Option<String>,
    /// Parameters set over its type's defaults.
    pub parameters: Parameters,
    /// Whether it takes the display flag.
    pub display: bool,
}

impl NewNode {
    /// A node of `node_type` with its defaults, not displayed.
    pub fn new(node_type: NodeTypeId) -> Self {
        Self {
            node_type,
            name: None,
            parameters: Parameters::new(),
            display: false,
        }
    }

    /// With parameter `name` set to `value`.
    pub fn with(mut self, name: impl Into<String>, value: impl Into<Value>) -> Self {
        self.parameters.insert(name.into(), value.into());
        self
    }

    /// Named `name`.
    pub fn named(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Displayed.
    pub fn displayed(mut self) -> Self {
        self.display = true;
        self
    }
}

/// A network to start a node's inside with: its nodes in order (each
/// displayed as it says, at most one), the wires between them, and the
/// insides of those that hold networks of their own (a Material in a
/// library). Studio lays the nodes out, makes and wires them in one step,
/// and refuses a wire naming a node or port that isn't there.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NetworkSeed {
    /// The nodes; wires and insides name them by index.
    pub nodes: Vec<NewNode>,
    /// The wires.
    pub wires: Vec<SeedWire>,
    /// The nodes' own insides, by node index.
    pub insides: Vec<(usize, NetworkSeed)>,
}

/// A wire of a [`NetworkSeed`]: output `output` of node `from` into input
/// `input` of node `to`, ports by name.
#[derive(Clone, Debug, PartialEq)]
pub struct SeedWire {
    /// The source node's index.
    pub from: usize,
    /// Its output port's name.
    pub output: String,
    /// The driven node's index.
    pub to: usize,
    /// Its input port's name.
    pub input: String,
}

impl NetworkSeed {
    /// `node` added; its index.
    pub fn node(&mut self, node: NewNode) -> usize {
        self.nodes.push(node);
        self.nodes.len() - 1
    }

    /// A wire from `from`'s output `output` into `to`'s input `input`.
    pub fn wire(&mut self, from: usize, output: &str, to: usize, input: &str) {
        self.wires.push(SeedWire {
            from,
            output: output.to_owned(),
            to,
            input: input.to_owned(),
        });
    }

    /// Node `at`'s inside started with `inside`.
    pub fn inside(&mut self, at: usize, inside: NetworkSeed) {
        self.insides.push((at, inside));
    }

    /// Whether it holds no node.
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
}

/// Another way Tab and the menus make a node: a node of `node.node_type`
/// with its parameters set, and its inside started with nodes (a Material
/// of an engine's preset). A package declares it
/// ([`Package::with_creation`](crate::Package::with_creation)) for any
/// installed node type, its own or another package's. Studio lists it
/// wherever `node.node_type` is listed, in that type's section, and makes
/// it in one undo step.
#[derive(Clone, Debug, PartialEq)]
pub struct NodeCreation {
    /// Its item id, which menus, keys and shelves name it by, under its
    /// package's vendor (`kinogaki.spectre.glass`).
    pub id: CreationId,
    /// What menus call it ("Glass").
    pub label: String,
    /// Its family within its type's section; empty: its type's family.
    pub family: String,
    /// More words search finds it by.
    pub aliases: Vec<String>,
    /// The node made; `name` `None` names it after `label`.
    pub node: NewNode,
    /// The network its inside starts with; the type must hold a network
    /// ([`Description::inside`](crate::cook::Description::inside)) when it
    /// isn't empty.
    pub inside: NetworkSeed,
}

impl NodeCreation {
    /// Creation `id` called `label`, making `node` with an empty inside.
    pub fn new(id: CreationId, label: impl Into<String>, node: NewNode) -> Self {
        Self {
            id,
            label: label.into(),
            family: String::new(),
            aliases: Vec::new(),
            node,
            inside: NetworkSeed::default(),
        }
    }

    /// Listed in `family` within its type's section.
    pub fn family(mut self, family: impl Into<String>) -> Self {
        self.family = family.into();
        self
    }

    /// Found by search under `aliases` too.
    pub fn aliases<S: Into<String>>(mut self, aliases: impl IntoIterator<Item = S>) -> Self {
        self.aliases = aliases.into_iter().map(Into::into).collect();
        self
    }

    /// With `node` next inside, wired to nothing.
    pub fn inside(mut self, node: NewNode) -> Self {
        self.inside.node(node);
        self
    }
}
