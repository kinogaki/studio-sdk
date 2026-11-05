//! Shading nodes: one node type over each material node an engine declares,
//! which is all a Material's network is made of. Its ports and parameters are
//! the def's; it never cooks: core compiles a Material's inside to a
//! [`MaterialNetwork`](kinogaki_render_api::MaterialNetwork).
use crate::cook::{Category, Description, Port, SchemaCx};
use crate::data::Data;
use crate::id::{ContextId, DataTypeId, NodeTypeId};
use crate::params::{Parameter, Parameters, Schema, SchemaItem};
use crate::{CookCx, Error, NodeType, Outputs};
use kinogaki_render_api::{Input, NodeDef};
use std::sync::Arc;

/// Inside a Material node: its shading network.
pub const SHADING: ContextId = ContextId::declared("kinogaki.core.shading");

/// What a shading wire carries, as the network pane sees it: which sockets
/// meet is checked as a Material compiles.
pub const SHADER: DataTypeId = DataTypeId::declared("kinogaki.core.shader");

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
        let wire = SHADER;
        let aliases: Vec<&str> = def.aliases.iter().map(|a| &**a).collect();
        let mut description = Description::new(&*def.label).context(SHADING).listed(
            &*self.engine,
            Category::Surface,
            &*def.family,
            &aliases,
        );
        let driven = (def.inputs.iter()).filter(|i| i.socket().is_some());
        for input in driven {
            description =
                description.input(Port::new(input.name(), input.label(), wire).optional());
        }
        for output in &def.outputs {
            description = description.output(Port::new(&*output.name, &*output.label, wire));
        }
        description.parameters(parameters(def))
    }

    fn cook(&self, _: &mut CookCx<'_>, _: &[Data]) -> Result<Outputs, Error> {
        Err(Error::invalid("a shading node compiles with its Material"))
    }

    /// The rows the def's conditions show for `parameters`, greyed where a
    /// value doesn't apply.
    fn schema(&self, mut rows: Schema, parameters: &Parameters, _: &SchemaCx<'_>) -> Schema {
        let value = |name: &str| {
            (self.def.inputs.iter())
                .filter_map(Input::value)
                .find(|d| &*d.name == name)
        };
        rows.items.retain(|item| match item {
            SchemaItem::Parameter(p) => value(&p.name).is_none_or(|d| d.is_shown(parameters)),
            _ => true,
        });
        for item in &mut rows.items {
            if let SchemaItem::Parameter(p) = item
                && let Some(d) = value(&p.name)
            {
                p.enabled &= d.applies(parameters);
            }
        }
        rows
    }
}

/// `def`'s values as rows: its groups' in their order, each as a section, then
/// the rest.
fn parameters(def: &NodeDef) -> Schema {
    let values = || def.inputs.iter().filter_map(Input::value);
    let mut schema = Schema::default();
    for d in values().filter(|d| !def.groups.iter().any(|g| g.name == d.group)) {
        schema = schema.parameter(Parameter::setting(d));
    }
    for group in &def.groups {
        schema = schema.section(&*group.name, &*group.label, group.open);
        for d in values().filter(|d| d.group == group.name) {
            schema = schema.parameter(Parameter::setting(d));
        }
    }
    schema
}
