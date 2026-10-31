//! Network contexts as packages declare them: registered with their
//! package, refused twice or under another vendor's name; node types
//! holding deltas name a registered data type; conditionally required
//! ports follow parameters; a compiled context comes with exactly one
//! compiler, which plans a flattened network.
use kinogaki_studio_api::cook::{Description, Port};
use kinogaki_studio_api::network::{
    CompileCx, CompileError, Compiled, Flag, FlagSet, Flat, FlatNode, NetworkCompiler,
    NetworkContext, Source,
};
use kinogaki_studio_api::{
    ContextId, CookCx, CookError, Data, DataTypeId, NodeType, NodeTypeId, Outputs, Package,
    PackageId, Parameters, Registry, RegistryError, Value,
};
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
    ) -> Result<Compiled, CompileError> {
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
        Ok(Compiled {
            plan: Arc::new(names),
            reads: cx.outputs().to_vec(),
        })
    }
}

#[test]
fn a_compiled_context_registers_with_one_compiler_and_plans_a_flat_network() {
    let compiled = || context("acme.trees.network").compiled();
    // A compiled context may come without its compiler (its plans aren't
    // shown); a compiler for a context no one compiles, or a second one,
    // is refused.
    let mut r = Registry::new();
    let lonely = Package::new(PackageId::declared("acme.rocks"), "Rocks")
        .with_context(context("acme.rocks.network").compiled());
    r.register(lonely).unwrap();
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
        Err(RegistryError::DuplicateCompiler { .. })
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
    assert_eq!(plan.reads, ["beauty"]);
    let names = plan.plan.downcast_ref::<Vec<String>>().unwrap();
    assert_eq!(names, &["leaf", "trunk", "beauty"]);
    let missing = compiler
        .compile(&flat, Source { node: 9, output: 0 }, &CompileCx::default())
        .unwrap_err();
    assert_eq!(
        (missing.node, missing.to_string()),
        (Some(9), "no such node".into())
    );
}

/// A runner of `acme.trees.network`'s plans (never made here).
#[cfg(feature = "wgpu")]
struct Runs;

#[cfg(feature = "wgpu")]
impl kinogaki_render_api::display::PlanRunner for Runs {
    fn make(
        &self,
        _: &kinogaki_render_api::display::wgpu::Device,
        _: &kinogaki_render_api::display::wgpu::Queue,
    ) -> Box<dyn kinogaki_render_api::display::Runner> {
        unreachable!("not made in this test")
    }
}

#[cfg(feature = "wgpu")]
#[test]
fn a_runner_runs_a_compiled_context_its_package_declares() {
    let compiled = || context("acme.trees.network").compiled();
    let trees = |runs: &[&str]| {
        runs.iter().fold(
            Package::new(PackageId::declared("acme.trees"), "Trees")
                .with_context(compiled())
                .with_compiler(Chain),
            |p, c| p.with_runner(ContextId::declared(c), Runs),
        )
    };
    assert!(
        Registry::new()
            .register(trees(&["acme.trees.network"]))
            .is_ok()
    );
    assert!(matches!(
        Registry::new().register(trees(&["acme.rocks.network"])),
        Err(RegistryError::StrayRunner { .. })
    ));
    assert!(matches!(
        Registry::new().register(trees(&["acme.trees.network", "acme.trees.network"])),
        Err(RegistryError::DuplicateRunner { .. })
    ));
}

/// A compiled context one package declares (Studio's Comp) takes its
/// compiler from another; a second compiler, from anyone, is refused.
#[test]
fn a_compiled_context_takes_its_compiler_from_another_package() {
    let mut r = Registry::new();
    let declares = Package::new(PackageId::declared("acme.trees"), "Trees")
        .with_context(context("acme.trees.network").compiled());
    r.register(declares).unwrap();
    assert!(
        r.compiler(ContextId::declared("acme.trees.network"))
            .is_none()
    );
    let compiles = Package::new(PackageId::declared("acme.forest"), "Forest").with_compiler(Chain);
    r.register(compiles).unwrap();
    assert!(
        r.compiler(ContextId::declared("acme.trees.network"))
            .is_some()
    );
    let again = Package::new(PackageId::declared("acme.grove"), "Grove").with_compiler(Chain);
    assert!(matches!(
        r.register(again),
        Err(RegistryError::DuplicateCompiler { first, .. }) if first == PackageId::declared("acme.forest")
    ));
}

/// A node type `id`, offered in context `offered`, holding a network of
/// context `inside`.
struct Holder {
    id: &'static str,
    offered: &'static str,
    inside: &'static str,
}

impl NodeType for Holder {
    fn id(&self) -> NodeTypeId {
        NodeTypeId::declared(self.id)
    }
    fn description(&self) -> Description {
        Description::new("Holder")
            .context(ContextId::declared(self.offered))
            .holding(ContextId::declared(self.inside))
    }
    fn cook(&self, _: &mut CookCx<'_>, _: &[Data]) -> Result<Outputs, CookError> {
        Ok(Outputs::default())
    }
}

/// A node holds a network of a declared context; contexts may nest
/// (Houdini's matnet under a SOP network), but never hold their own
/// networks, directly or through others: the package closing the loop is
/// refused, naming it.
#[test]
fn nodes_hold_networks_of_declared_contexts_without_loops() {
    let holder = |id, offered, inside| Holder {
        id,
        offered,
        inside,
    };
    // A context no one declares.
    let stray = Package::new(PackageId::declared("acme.trees"), "Trees")
        .with_context(context("acme.trees.grove"))
        .with_node_type(holder(
            "acme.trees.oak",
            "acme.trees.grove",
            "acme.trees.nowhere",
        ));
    assert!(matches!(
        Registry::new().register(stray),
        Err(RegistryError::UnknownContext { .. })
    ));
    // A context holding its own.
    let own = Package::new(PackageId::declared("acme.trees"), "Trees")
        .with_context(context("acme.trees.grove"))
        .with_node_type(holder(
            "acme.trees.oak",
            "acme.trees.grove",
            "acme.trees.grove",
        ));
    assert!(matches!(
        Registry::new().register(own),
        Err(RegistryError::InsideCycle { .. })
    ));
    // Grove holds Leaves, Leaves hold Veins: nesting is fine.
    let mut r = Registry::new();
    let trees = Package::new(PackageId::declared("acme.trees"), "Trees")
        .with_context(context("acme.trees.grove"))
        .with_context(context("acme.trees.leaves"))
        .with_context(context("acme.trees.veins"))
        .with_node_type(holder(
            "acme.trees.oak",
            "acme.trees.grove",
            "acme.trees.leaves",
        ))
        .with_node_type(holder(
            "acme.trees.leaf",
            "acme.trees.leaves",
            "acme.trees.veins",
        ));
    r.register(trees).unwrap();
    // A later package whose Veins node holds a Grove closes the loop.
    let loop_ = Package::new(PackageId::declared("acme.moss"), "Moss").with_node_type(holder(
        "acme.moss.spore",
        "acme.trees.veins",
        "acme.trees.grove",
    ));
    let Err(RegistryError::InsideCycle { node, path, .. }) = r.register(loop_) else {
        panic!("the loop is refused");
    };
    assert_eq!(node, NodeTypeId::declared("acme.moss.spore"));
    let path: Vec<&str> = path.iter().map(|c| c.as_str()).collect();
    assert_eq!(
        path,
        [
            "acme.trees.veins",
            "acme.trees.grove",
            "acme.trees.leaves",
            "acme.trees.veins"
        ]
    );
}
