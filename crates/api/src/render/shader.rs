//! Shading nodes: one node type over each material node an engine declares,
//! which is all a Material's network is made of. Its ports and parameters are
//! the def's; it never cooks: core compiles a Material's inside to a
//! [`MaterialNetwork`](kinogaki_render_api::MaterialNetwork).
use crate::cook::{Category, Description, Port};
use crate::data::Data;
use crate::id::{ContextId, CreationId, DataTypeId, NodeTypeId};
use crate::network::{NewNode, NodeCreation};
use crate::{CookCx, Error, NodeType, Outputs};
use kinogaki_render_api::NodeDef;
use kinogaki_render_api::materialx::Type;
use std::sync::Arc;

/// Inside a Material node: its shading network.
pub const SHADING: ContextId = ContextId::declared("kinogaki.core.shading");

/// Core's Material node: what holds a shading network ([`SHADING`]).
pub const MATERIAL: NodeTypeId = NodeTypeId::declared("kinogaki.core.material");

/// A starting material an engine offers in Tab: a Material node
/// labelled `label` with `inside` seeded in it, displayed, listed under
/// `family` and found by `aliases`.
pub fn material_creation(
    id: CreationId,
    label: &str,
    family: &str,
    aliases: &[&str],
    inside: NewNode,
) -> NodeCreation {
    NodeCreation::new(id, label, NewNode::new(MATERIAL))
        .family(family)
        .aliases(aliases.iter().copied())
        .inside(inside.displayed())
}

/// What a shading wire carries, as the network pane sees it: which sockets
/// meet is checked as a Material compiles.
pub const SHADER: DataTypeId = DataTypeId::declared("kinogaki.core.shader");

/// What the wire at a material node's port of `socket` carries: an image
/// ([`standard::image`](crate::standard::image)) at an Image input, where the
/// host's Image node is wired, a shading wire ([`SHADER`]) at every other.
pub fn wire_type(socket: &Type) -> DataTypeId {
    match socket {
        Type::Filename => crate::standard::image().id(),
        _ => SHADER,
    }
}

/// The node type of material node `def`, listed under its engine's label.
pub struct ShaderNode {
    def: Arc<NodeDef>,
    engine: Arc<str>,
}

impl ShaderNode {
    /// The node type of `def`, which the engine labelled `engine` brings.
    pub fn new(def: Arc<NodeDef>, engine: Arc<str>) -> Self {
        Self { def, engine }
    }

    /// What it is the node type of.
    pub fn def(&self) -> &Arc<NodeDef> {
        &self.def
    }
}

impl NodeType for ShaderNode {
    fn id(&self) -> NodeTypeId {
        self.def.id
    }

    fn description(&self) -> Description {
        let def = &self.def;
        let aliases: Vec<&str> = def.aliases.iter().map(|a| &**a).collect();
        let mut description = Description::new(&*def.label).context(SHADING).listed(
            &*self.engine,
            Category::Surface,
            &*def.family,
            &aliases,
        );
        for input in &def.inputs {
            if let Some(socket) = input.socket() {
                let port = Port::new(input.name(), input.label(), wire_type(socket));
                description = description.input(port.optional());
            }
        }
        for output in &def.outputs {
            let port = Port::new(&*output.name, &*output.label, wire_type(&output.socket));
            description = description.output(port);
        }
        description.parameters(def.schema())
    }

    fn cook(&self, _: &mut CookCx<'_>, _: &[Data]) -> Result<Outputs, Error> {
        Err(Error::invalid("a shading node compiles with its Material"))
    }
}
