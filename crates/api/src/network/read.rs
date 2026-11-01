//! [`NetworkRead`]: a network as packages read it.
use crate::components::Components;
use crate::data::Data;
use crate::id::{ContextId, NodeTypeId};
use crate::params::{Parameters, Value};
use crate::refs::NodeRef;
use crate::time::Time;

/// One network, read through the host (a package's UI reaches an
/// object's through the UI API's `HostNetwork`). Nodes are named by the host's ids, which are unique through every
/// level of the network (a node keeps its id moving into or out of a
/// subnet).
pub trait NetworkRead {
    /// The context it is a network of.
    fn context(&self) -> ContextId;

    /// The node its top level displays (its display flag).
    fn display(&self) -> Option<NodeRef>;

    /// The node the network yields to its object: its node of
    /// [`NodeRole::Output`](crate::cook::NodeRole::Output), else the
    /// displayed one.
    fn yielded(&self) -> Option<NodeRef>;

    /// Every node, on every level, in id order.
    fn nodes(&self) -> Vec<NodeRef>;

    /// Node `node`, when the network holds it.
    fn node(&self, node: NodeRef) -> Option<NodeInfo<'_>>;
    /// The network node `node` holds inside it, when its type holds one
    /// ([`Description::inside`](crate::cook::Description::inside)).
    fn inside(&self, node: NodeRef) -> Option<&dyn NetworkRead> {
        let _ = node;
        None
    }
    /// Parameter `row` of `node` at `time`: its keys' value there when it
    /// is animated, else its value. `None` when the node or the row isn't
    /// there, or the network doesn't animate (a package's handles read
    /// the value at the scene's current time, not the static one).
    fn value_at(&self, node: NodeRef, row: &str, time: Time) -> Option<Value> {
        let _ = (node, row, time);
        None
    }
}

/// One node as [`NetworkRead::node`] reads it.
#[derive(Clone, Debug)]
pub struct NodeInfo<'a> {
    /// Its name, unique on its level.
    pub name: &'a str,
    /// Its type.
    pub node_type: NodeTypeId,
    /// Its parameters by name: every one its type declares, and the
    /// spares its type declares for them. An animated row holds its static
    /// value, the one it keeps when its keys are cleared
    /// ([`NetworkRead::value_at`] reads its value at a moment).
    pub parameters: &'a Parameters,
    /// Its animated parameters (rows with keys), by name.
    pub animated: Vec<String>,
    /// Its input wires, one list a port in port order, in wire order.
    pub inputs: Vec<Vec<Wire>>,
    /// Its core flags.
    pub flags: NodeFlags,
    /// The subnet holding it, or `None` on the top level.
    pub level: Option<NodeRef>,
    /// Where its tile is.
    pub position: [f32; 2],
    /// The delta it holds over its input (a held edit's,
    /// [`NetworkEdit::set_delta`](super::NetworkEdit::set_delta)).
    pub delta: Option<&'a Data>,
    /// Its first output as last cooked, when it has cooked since its last
    /// change (a held edit's result, for baking it into its base).
    pub result: Option<Data>,
    /// The selection that output carries downstream
    /// ([`Outputs::selection`](crate::Outputs::selection)) as last cooked,
    /// on the same terms as [`result`](Self::result): what `.selection`
    /// names in a group expression.
    pub selection: Option<Components>,
}

/// A wire into a node: the node it leaves and which of its outputs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Wire {
    /// The node it leaves.
    pub node: NodeRef,
    /// The output's index among that node's outputs.
    pub output: usize,
}

/// A node's core flags.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NodeFlags {
    /// It passes input 0 on ([`Flag::Bypass`](super::Flag::Bypass)).
    pub bypass: bool,
    /// It shows as a reference ([`Flag::Template`](super::Flag::Template)).
    pub template: bool,
    /// It keeps the outputs it was frozen with
    /// ([`Flag::Freeze`](super::Flag::Freeze)).
    pub frozen: bool,
    /// It holds a delta over its input ([`NetworkEdit::set_delta`](super::NetworkEdit::set_delta)).
    pub held: bool,
}
