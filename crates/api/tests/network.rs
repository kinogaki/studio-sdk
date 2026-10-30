//! Network contexts as packages declare them: registered with their
//! package, refused twice or under another vendor's name; node types
//! holding deltas name a registered data type; conditionally required
//! ports follow parameters; a compiled context comes with exactly one
//! compiler, which plans a flattened network.
use kinogaki_studio_api::cook::{Description, Port};
use kinogaki_studio_api::network::{
    CompileCx, CompileError, Flag, FlagSet, Flat, FlatNode, NetworkCompiler, NetworkContext, Source,
};
use kinogaki_studio_api::{
    ContextId, CookCx, CookError, Data, DataTypeId, NodeType, NodeTypeId, Outputs, Package,
    PackageId, Parameters, Registry, RegistryError, Value,
};
use std::any::Any;
use std::sync::Arc;

fn context(id: &str) -> NetworkContext {
    NetworkContext::new(ContextId::declared(id), "Trees", "trees")
}

/// A node type holding deltas of `acme.trees.growth`.
struct Grow;

impl NodeType for Grow {
    fn id(&self) -> NodeTypeId {
        NodeTypeId::declared("acme.trees.grow")
    }
    fn description(&self) -> Description {
        Description::new("Grow").context(ContextId::declared("acme.trees.network"))
    }
    fn held(&self) -> Option<DataTypeId> {
        Some(DataTypeId::declared("acme.trees.growth"))
    }
    fn cook(&self, _: &mut CookCx<'_>, _: &[Data]) -> Result<Outputs, CookError> {
        Ok(Outputs::default())
    }
}

#[test]
fn a_context_registers_once_under_its_vendor() {
    let mut registry = Registry::new();
    let trees = Package::new(PackageId::declared("acme.trees"), "Trees")
        .with_context(context("acme.trees.network").flags(FlagSet::all()));
    registry.register(trees).unwrap();
    let declared = registry
        .network_context(ContextId::declared("acme.trees.network"))
        .unwrap();
    assert!(declared.heals && declared.flags.offers(Flag::Freeze));
    let again = Package::new(PackageId::declared("acme.more"), "More")
        .with_context(context("acme.trees.network"));
    assert!(matches!(
        registry.register(again),
        Err(RegistryError::DuplicateId {
            what: "network context",
            ..
        })
    ));
    let foreign = Package::new(PackageId::declared("other.trees"), "Other")
        .with_context(context("acme.other.network"));
    assert!(matches!(
        registry.register(foreign),
        Err(RegistryError::ForeignId { .. })
    ));
    let twice = Package::new(PackageId::declared("acme.twice"), "Twice")
        .with_context(context("acme.twice.network"))
        .with_context(context("acme.twice.network"));
    assert!(matches!(
        registry.register(twice),
        Err(RegistryError::DuplicateId {
            what: "network context",
            ..
        })
    ));
    assert!(!FlagSet::default().offers(Flag::Bypass));
}

#[test]
fn a_held_node_type_names_a_registered_delta() {
    let mut registry = Registry::new();
    let trees = Package::new(PackageId::declared("acme.trees"), "Trees").with_node_type(Grow);
    assert!(matches!(
        registry.register(trees),
        Err(RegistryError::UnknownDataType { .. })
    ));
}

#[test]
fn a_port_required_while_a_parameter_holds_a_value() {
    let tools = Port::new("tools", "Tools", DataTypeId::declared("acme.trees.wood"))
        .required_when("operation", Value::Index(1));
    assert!(tools.optional);
    let mut parameters = Parameters::from([("operation".to_owned(), Value::Index(0))]);
    assert!(!tools.required(&parameters));
    parameters.insert("operation".into(), Value::Index(1));
    assert!(tools.required(&parameters));
    assert!(
        Port::new("trunk", "Trunk", DataTypeId::declared("acme.trees.wood")).required(&parameters)
    );
}

/// A compiler of `acme.trees.network` (declared compiled): its plan is
/// the chain of node names from the target back to its source, a bypassed
/// node left out.
struct Chain;

impl NetworkCompiler for Chain {
    fn context(&self) -> ContextId {
        ContextId::declared("acme.trees.network")
    }
    fn compile(
        &self,
        flat: &Flat<'_>,
        target: Source,
        cx: &CompileCx<'_>,
    ) -> Result<Arc<dyn Any + Send + Sync>, CompileError> {
        let mut names = Vec::new();
        let mut at = Some(target);
        while let Some(source) = at {
            let node = flat
                .node(source.node)
                .ok_or_else(|| CompileError::at(source.node, "no such node"))?;
            if !node.bypass {
                names.push(node.name.to_owned());
            }
            at = node.input(0);
        }
        names.push(cx.outputs().first().cloned().unwrap_or_default());
        Ok(Arc::new(names))
    }
}

#[test]
fn a_compiled_context_registers_with_one_compiler_and_plans_a_flat_network() {
    let compiled = || context("acme.trees.network").compiled();
    // Without its compiler, or a compiler for a context it doesn't
    // compile, the package is refused.
    let mut r = Registry::new();
    let lonely = Package::new(PackageId::declared("acme.trees"), "Trees").with_context(compiled());
    assert!(matches!(
        r.register(lonely),
        Err(RegistryError::NoCompiler { .. })
    ));
    let stray = Package::new(PackageId::declared("acme.trees"), "Trees")
        .with_context(context("acme.trees.network"))
        .with_compiler(Chain);
    assert!(matches!(
        r.register(stray),
        Err(RegistryError::StrayCompiler { .. })
    ));
    let twice = Package::new(PackageId::declared("acme.trees"), "Trees")
        .with_context(compiled())
        .with_compiler(Chain)
        .with_compiler(Chain);
    assert!(matches!(
        r.register(twice),
        Err(RegistryError::StrayCompiler { .. })
    ));
    let trees = Package::new(PackageId::declared("acme.trees"), "Trees")
        .with_context(compiled())
        .with_compiler(Chain);
    r.register(trees).unwrap();
    let compiler = r
        .compiler(ContextId::declared("acme.trees.network"))
        .expect("its compiler");
    assert!(
        r.compiler(ContextId::declared("acme.rocks.network"))
            .is_none()
    );
    // trunk ← branch (bypassed) ← leaf.
    let none = Parameters::new();
    let node = |id, name, input: Option<u64>, bypass| FlatNode {
        id,
        name,
        node_type: NodeTypeId::declared("acme.trees.part"),
        parameters: &none,
        inputs: vec![
            input
                .map(|n| Source { node: n, output: 0 })
                .into_iter()
                .collect(),
        ],
        bypass,
    };
    let flat = Flat {
        nodes: vec![
            node(1, "trunk", None, false),
            node(2, "branch", Some(1), true),
            node(3, "leaf", Some(2), false),
        ],
    };
    let outputs = ["beauty".to_owned()];
    let plan = compiler
        .compile(
            &flat,
            Source { node: 3, output: 0 },
            &CompileCx::new(&outputs),
        )
        .unwrap();
    let names = plan.downcast_ref::<Vec<String>>().unwrap();
    assert_eq!(names, &["leaf", "trunk", "beauty"]);
    let missing = compiler
        .compile(&flat, Source { node: 9, output: 0 }, &CompileCx::default())
        .unwrap_err();
    assert_eq!(
        (missing.node, missing.to_string()),
        (Some(9), "no such node".into())
    );
}
