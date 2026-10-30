//! [`NetworkEdit`]: the edits a package makes to a network.
use super::{Flag, NetworkRead, Wire};
use crate::cook::Outputs;
use crate::data::Data;
use crate::error::Error;
use crate::id::NodeTypeId;
use crate::params::{Parameters, Value};
use crate::refs::NodeRef;

/// A network a package edits through the host, inside one undo step. Each
/// call refuses what doesn't fit (an unknown node, a wire into a port the
/// node hasn't got, a value its parameter doesn't take
/// ([`Parameter::check`](crate::params::Parameter::check)), a flag its
/// context doesn't offer) and changes nothing then; the host checks the
/// whole network again when the step is committed (no cycles, required
/// values) and drops the step when that fails.
pub trait NetworkEdit: NetworkRead {
    /// Adds `node` on the top level at `position`, named as nothing there
    /// is: a new node of its type with its parameters set over the type's
    /// defaults (a [real size](crate::params::Number::real_size) in the
    /// document's unit), displayed when `node.display` says so.
    fn add_node(&mut self, node: NewNode, position: [f32; 2]) -> Result<NodeRef, Error>;

    /// Sets parameter `name` of `node` to `value`.
    fn set_parameter(&mut self, node: NodeRef, name: &str, value: Value) -> Result<(), Error>;

    /// Replaces every parameter of `node`: `parameters` hold each one its
    /// type declares.
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

    /// Freezes `node` with `outputs`, the outputs it has now (one value
    /// per output port): nothing upstream cooks for it until it is
    /// unfrozen.
    fn freeze(&mut self, node: NodeRef, outputs: Outputs) -> Result<(), Error>;
}

/// A node to add ([`NetworkEdit::add_node`]).
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

    /// Displayed.
    pub fn displayed(mut self) -> Self {
        self.display = true;
        self
    }
}
