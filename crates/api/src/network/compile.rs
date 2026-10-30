//! Compiled contexts: a context whose [`Evaluation`](super::Evaluation) is
//! `Compile` (Comp, shading) isn't cooked node by node. Core flattens the
//! network (subnets resolved, in cook order) into a [`Flat`] and hands it
//! to the context's [`NetworkCompiler`], which turns it into the package's
//! own plan; the package's executor runs the plan (a Comp plan on the GPU,
//! as the display draws). Core keeps the plan until the network's topology
//! or a flattened node's parameters change.
use crate::id::{ContextId, NodeTypeId};
use crate::params::Parameters;
use std::any::Any;
use std::fmt;
use std::sync::Arc;

/// Where a wire comes from: a flattened node and which of its outputs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Source {
    /// The node, by the host's id.
    pub node: u64,
    /// The output, by index into its outputs.
    pub output: usize,
}

/// One node of a [`Flat`] network.
#[derive(Clone, Debug)]
pub struct FlatNode<'a> {
    /// Its id (the host's; a [`Source`] names it).
    pub id: u64,
    /// Its name, for what a compiler reports.
    pub name: &'a str,
    /// Its type.
    pub node_type: NodeTypeId,
    /// Its parameters (a subnet's driven values resolved).
    pub parameters: &'a Parameters,
    /// Each input port's wires, in order.
    pub inputs: Vec<Vec<Source>>,
    /// Bypassed: what that means is the compiler's (Comp passes the first
    /// input's image on).
    pub bypass: bool,
}

impl FlatNode<'_> {
    /// Input `port`'s first wire.
    pub fn input(&self, port: usize) -> Option<Source> {
        self.inputs.get(port)?.first().copied()
    }
}

/// A network flattened for a compiler: the nodes the target reads, each
/// after the nodes it reads, subnets resolved (a wire into a subnet comes
/// from the node inside that feeds its output).
#[derive(Clone, Debug, Default)]
pub struct Flat<'a> {
    /// The nodes, in cook order.
    pub nodes: Vec<FlatNode<'a>>,
}

impl<'a> Flat<'a> {
    /// Node `id`.
    pub fn node(&self, id: u64) -> Option<&FlatNode<'a>> {
        self.nodes.iter().find(|n| n.id == id)
    }
}

/// What a compile may read besides the network.
#[derive(Clone, Copy, Debug, Default)]
pub struct CompileCx<'a> {
    outputs: &'a [String],
}

impl<'a> CompileCx<'a> {
    /// A compile where the render writes `outputs` (its output ids, the
    /// beauty first).
    pub fn new(outputs: &'a [String]) -> Self {
        Self { outputs }
    }

    /// The outputs the render writes, by id, the beauty first: what a
    /// render's image nodes may read.
    pub fn outputs(&self) -> &'a [String] {
        self.outputs
    }
}

/// Why a network can't be compiled to its target.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompileError {
    /// The node it is about, if one.
    pub node: Option<u64>,
    /// What is wrong, for the user.
    pub message: String,
}

impl CompileError {
    /// `message` about node `node`.
    pub fn at(node: u64, message: impl Into<String>) -> Self {
        Self {
            node: Some(node),
            message: message.into(),
        }
    }
}

impl fmt::Display for CompileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for CompileError {}

/// A compiled context's compiler: one for each context declared
/// `Evaluation::Compile`, registered by the package declaring the context
/// ([`Package::with_compiler`](crate::Package::with_compiler)). Its node
/// types are ordinary [`NodeType`](crate::NodeType)s (descriptions, ports,
/// schemas) whose `cook` core never calls.
pub trait NetworkCompiler: Send + Sync {
    /// The context it compiles.
    fn context(&self) -> ContextId;

    /// `flat`'s plan for showing output `target`: the package's own value,
    /// which its executor downcasts and runs.
    fn compile(
        &self,
        flat: &Flat<'_>,
        target: Source,
        cx: &CompileCx<'_>,
    ) -> Result<Arc<dyn Any + Send + Sync>, CompileError>;
}
