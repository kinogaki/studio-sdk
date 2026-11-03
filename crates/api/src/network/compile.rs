//! Compiled contexts: a context whose [`Evaluation`](super::Evaluation) is
//! `Compile` (Comp, shading) isn't cooked node by node. Core flattens the
//! network (subnets resolved, in cook order) into a [`Flat`] and hands it
//! to the context's [`NetworkCompiler`], which turns it into the package's
//! own plan; the package's executor runs the plan (a Comp plan on the GPU,
//! as the display draws). Core keeps the plan until the network's topology
//! or a flattened node's parameters change.
use crate::Error;
use crate::cook::NodeType;
use crate::id::{ContextId, NodeTypeId};
use crate::network::Wire;
use crate::params::Parameters;
use crate::refs::NodeRef;
use std::any::Any;
use std::fmt;
use std::sync::Arc;

/// One node of a [`Flat`] network.
#[derive(Clone, Debug)]
pub struct FlatNode<'a> {
    /// Its id (the host's; a [`Wire`] names it).
    pub id: NodeRef,
    /// Its name, for what a compiler reports.
    pub name: &'a str,
    /// Its type.
    pub node_type: NodeTypeId,
    /// Its parameters (a subnet's driven values resolved).
    pub parameters: &'a Parameters,
    /// Each input port's wires, in order.
    pub inputs: Vec<Vec<Wire>>,
    /// Bypassed: what that means is the compiler's (Comp passes the first
    /// input's image on).
    pub bypass: bool,
}

impl FlatNode<'_> {
    /// Input `port`'s first wire.
    pub fn input(&self, port: usize) -> Option<Wire> {
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
    pub fn node(&self, id: NodeRef) -> Option<&FlatNode<'a>> {
        self.nodes.iter().find(|n| n.id == id)
    }
}

/// The node types a compile sees, by id ([`CompileCx::with_types`]): the
/// host's installed types, or a [`Registry`](crate::Registry)'s.
pub trait NodeTypes {
    /// Node type `id`, when there is one.
    fn node_type(&self, id: NodeTypeId) -> Option<&dyn NodeType>;
}

/// What a compile may read besides the network: the outputs the render
/// writes and the types of the nodes it compiles.
#[derive(Clone, Copy, Default)]
pub struct CompileCx<'a> {
    outputs: &'a [String],
    types: Option<&'a dyn NodeTypes>,
}

impl<'a> CompileCx<'a> {
    /// A compile where the render writes `outputs` (its output ids, the
    /// beauty first), seeing no node types.
    pub fn new(outputs: &'a [String]) -> Self {
        Self {
            outputs,
            types: None,
        }
    }

    /// Seeing node types through `types` (the host passes its installed
    /// types). A borrowed trait object keeps the context covariant.
    pub fn with_types(mut self, types: &'a dyn NodeTypes) -> Self {
        self.types = Some(types);
        self
    }

    /// The outputs the render writes, by id, the beauty first: what a
    /// render's image nodes may read.
    pub fn outputs(&self) -> &'a [String] {
        self.outputs
    }

    /// Node type `id`, when it is installed (`None` for every id in a
    /// compile that sees no types).
    ///
    /// A compiler reads what a type declares (its description, its
    /// [`ports`](NodeType::ports) and [`spares`](NodeType::spares))
    /// through the trait first, and downcasts it to its own concrete type
    /// (`NodeType` is [`Any`]: `(ty as &dyn Any).downcast_ref::<Mine>()`)
    /// only for what that type alone knows (a Comp operator's plan node, a
    /// shader's engine vocabulary).
    pub fn node_type(&self, id: NodeTypeId) -> Option<&'a dyn NodeType> {
        self.types?.node_type(id)
    }
}

impl fmt::Debug for CompileCx<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CompileCx")
            .field("outputs", &self.outputs)
            .field("types", &self.types.is_some())
            .finish()
    }
}

/// What a compile made: the package's plan, opaque to the host (its runner
/// runs it), and the render outputs it reads, which the host asks the
/// render for before running it.
#[derive(Clone)]
pub struct Compiled {
    /// The package's own plan.
    pub plan: Arc<dyn Any + Send + Sync>,
    /// The render outputs it reads, by id.
    pub reads: Vec<String>,
}

impl fmt::Debug for Compiled {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Compiled")
            .field("reads", &self.reads)
            .finish_non_exhaustive()
    }
}

/// A compiled context's compiler: one for each context declared
/// `Evaluation::Compile`, registered by the package declaring the context
/// ([`Package::with_compiler`](crate::Package::with_compiler)). Its node
/// types are ordinary [`NodeType`](crate::NodeType)s (descriptions, ports,
/// schemas) whose `cook` core never calls.
pub trait NetworkCompiler: Send + Sync {
    /// The context it compiles.
    fn context(&self) -> ContextId;

    /// `flat`'s plan for showing output `target` (the package's own value,
    /// which its runner downcasts and runs) and what it reads.
    ///
    /// # Errors
    /// Why the network can't be compiled (a required input unwired, a wire
    /// of the wrong kind), its element the node at fault (its
    /// [`NodeRef`]'s number) when one is.
    fn compile(&self, flat: &Flat<'_>, target: Wire, cx: &CompileCx<'_>)
    -> Result<Compiled, Error>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Error;
    use crate::cook::{CookCx, Description, Outputs};
    use crate::data::Data;

    /// A type a compiler knows by its own struct.
    struct Mine(u32);

    impl NodeType for Mine {
        fn id(&self) -> NodeTypeId {
            NodeTypeId::declared("acme.trees.mine")
        }
        fn description(&self) -> Description {
            Description::new("Mine")
        }
        fn cook(&self, _: &mut CookCx<'_>, _: &[Data]) -> Result<Outputs, Error> {
            Err(Error::invalid("compiled"))
        }
    }

    impl NodeTypes for Mine {
        fn node_type(&self, id: NodeTypeId) -> Option<&dyn NodeType> {
            (id == self.id()).then_some(self as &dyn NodeType)
        }
    }

    #[test]
    fn a_compile_sees_node_types_through_its_lookup() {
        let mine = Mine(7);
        let outputs = ["beauty".to_owned()];
        let cx = CompileCx::new(&outputs).with_types(&mine);
        let ty = cx.node_type(mine.id()).expect("installed");
        assert_eq!(ty.description().label, "Mine");
        let any: &dyn Any = ty;
        assert_eq!(any.downcast_ref::<Mine>().map(|m| m.0), Some(7));
        assert!(
            cx.node_type(NodeTypeId::declared("acme.trees.other"))
                .is_none()
        );
        assert_eq!(cx.outputs(), outputs);
        // A compile that sees no types finds none.
        assert!(CompileCx::default().node_type(mine.id()).is_none());
        assert!(CompileCx::new(&outputs).node_type(mine.id()).is_none());
    }
}
