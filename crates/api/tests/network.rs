//! Network contexts as packages declare them: registered with their
//! package, refused twice or under another vendor's name; node types
//! holding deltas name a registered data type; conditionally required
//! ports follow parameters; a compiled context comes with exactly one
//! compiler, which plans a flattened network; a library declares its
//! nodes unwired results; packages add ways to make other packages'
//! nodes.
use kinogaki_studio_api::cook::{Description, Port};
use kinogaki_studio_api::network::{
    CompileCx, Compiled, Flag, Flags, Flat, FlatNode, NetworkCompiler, NetworkContext, NetworkKind,
    NewNode, NodeCreation, Wire,
};
use kinogaki_studio_api::value::Condition;
use kinogaki_studio_api::{
    ContextId, CookCx, CreationId, Data, DataTypeId, Error, NodeRef, NodeType, NodeTypeId, Outputs,
    Package, PackageId, Parameters, Registry, Value,
};
use kinogaki_studio_api_testing::assert_refused;
use std::sync::Arc;

fn context(id: &'static str) -> NetworkContext {
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
    fn cook(&self, _: &mut CookCx<'_>, _: &[Data]) -> Result<Outputs, Error> {
        Ok(Outputs::default())
    }
}

#[test]
fn a_context_registers_once_under_its_vendor() {
    let mut registry = Registry::new();
    let trees = Package::new(PackageId::declared("acme.trees"), "Trees")
        .with_context(context("acme.trees.network").flags(Flags::ALL));
    registry.register(trees).unwrap();
    let again = Package::new(PackageId::declared("acme.more"), "More")
        .with_context(context("acme.trees.network"));
    assert_refused(registry.register(again), &[", which ", "network context"]);
    let foreign = Package::new(PackageId::declared("other.trees"), "Other")
        .with_context(context("acme.other.network"));
    assert_refused(registry.register(foreign), &["under another vendor's name"]);
    let twice = Package::new(PackageId::declared("acme.twice"), "Twice")
        .with_context(context("acme.twice.network"))
        .with_context(context("acme.twice.network"));
    assert_refused(registry.register(twice), &[", which ", "network context"]);
    assert!(!Flags::NONE.contains(Flag::Bypass));
}

#[test]
fn a_library_holds_unwired_results_without_flags() {
    let plain = context("acme.trees.network").flags(Flags::ALL);
    assert_eq!(plain.kind, NetworkKind::Chain);
    let library = plain.clone().library();
    assert_eq!(library.kind, NetworkKind::Library);
    assert_eq!(library.flags, Flags::NONE);
    assert_eq!((library.id, library.storage), (plain.id, plain.storage));
}

#[test]
fn a_package_adds_ways_to_make_another_packages_nodes() {
    let mut registry = Registry::new();
    // Another package's type, which Studio lists the creations with.
    let grow = NodeTypeId::declared("acme.trees.grow");
    let oak = NodeCreation::new(
        CreationId::declared("acme.forest.oak"),
        "Oak",
        NewNode::new(grow).named("oak").with("rate", 2.0),
    )
    .family("Broadleaf")
    .aliases(["quercus"])
    .inside(NewNode::new(grow).displayed());
    let forest = Package::new(PackageId::declared("acme.forest"), "Forest")
        .with_creation(oak.clone())
        .with_creation(NodeCreation::new(
            CreationId::declared("acme.forest.pine"),
            "Pine",
            NewNode::new(grow),
        ));
    registry.register(forest).unwrap();
    let made: Vec<&NodeCreation> = registry.creations().collect();
    assert_eq!(made[0], &oak);
    assert_eq!(made[1].id.as_str(), "acme.forest.pine");
    assert_eq!(made[1].node.name, None);
    assert!(made[1].family.is_empty() && made[1].inside.is_empty());
    assert_eq!(oak.node.name.as_deref(), Some("oak"));
    assert_eq!(oak.node.parameters["rate"], Value::Number(2.0));
    assert!(oak.inside.nodes[0].display);
}

#[test]
fn a_held_node_type_names_a_registered_delta() {
    let mut registry = Registry::new();
    let trees = Package::new(PackageId::declared("acme.trees"), "Trees")
        .with_context(context("acme.trees.network"))
        .with_node_type(Grow);
    assert_refused(registry.register(trees), &["which isn't registered"]);
}

#[test]
fn a_port_required_while_a_parameter_holds_a_value() {
    let tools = Port::new("tools", "Tools", DataTypeId::declared("acme.trees.wood")).required_when(
        Condition::when("operation", Value::Choice("subtract".into())),
    );
    let mut parameters =
        Parameters::from([("operation".to_owned(), Value::Choice("union".into()))]);
    assert!(!tools.required(&parameters));
    parameters.insert("operation".into(), Value::Choice("subtract".into()));
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
        target: Wire,
        cx: &CompileCx<'_>,
    ) -> Result<Compiled, Error> {
        let mut names = Vec::new();
        let mut at = Some(target);
        while let Some(source) = at {
            let node = flat
                .node(source.node)
                .ok_or_else(|| Error::invalid("no such node").element(source.node.0 as usize))?;
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
    assert_refused(r.register(stray), &["brings a compiler for"]);
    let twice = Package::new(PackageId::declared("acme.trees"), "Trees")
        .with_context(compiled())
        .with_compiler(Chain)
        .with_compiler(Chain);
    assert_refused(r.register(twice), &["already brought"]);
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
        id: NodeRef(id),
        name,
        node_type: NodeTypeId::declared("acme.trees.part"),
        parameters: &none,
        inputs: vec![
            input
                .map(|n| Wire {
                    node: NodeRef(n),
                    output: 0,
                })
                .into_iter()
                .collect(),
        ],
        bypass,
        kept: None,
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
            Wire {
                node: NodeRef(3),
                output: 0,
            },
            &CompileCx::new(&outputs),
        )
        .unwrap();
    assert_eq!(plan.reads, ["beauty"]);
    let names = plan.plan.downcast_ref::<Vec<String>>().unwrap();
    assert_eq!(names, &["leaf", "trunk", "beauty"]);
    let missing = compiler
        .compile(
            &flat,
            Wire {
                node: NodeRef(9),
                output: 0,
            },
            &CompileCx::default(),
        )
        .unwrap_err();
    assert_eq!(
        (missing.diagnostic.element, missing.message()),
        (Some(9), "no such node")
    );
}

/// A runner of `acme.trees.network`'s plans (never made here).
#[cfg(feature = "wgpu")]
struct Runs;

#[cfg(feature = "wgpu")]
impl kinogaki_studio_api::network::run::PlanRunner for Runs {
    fn make(
        &self,
        _: &kinogaki_render_api::display::wgpu::Device,
        _: &kinogaki_render_api::display::wgpu::Queue,
    ) -> Box<dyn kinogaki_studio_api::network::run::Runner> {
        unreachable!("not made in this test")
    }
}

#[cfg(feature = "wgpu")]
#[test]
fn a_runner_runs_a_compiled_context_its_package_declares() {
    let compiled = || context("acme.trees.network").compiled();
    let trees = |runs: &[&'static str]| {
        runs.iter().fold(
            Package::new(PackageId::declared("acme.trees"), "Trees")
                .with_context(compiled())
                .with_compiler(Chain),
            |p, &c| p.with_runner(ContextId::declared(c), Runs),
        )
    };
    assert!(
        Registry::new()
            .register(trees(&["acme.trees.network"]))
            .is_ok()
    );
    assert_refused(
        Registry::new().register(trees(&["acme.rocks.network"])),
        &["brings a runner for"],
    );
    assert_refused(
        Registry::new().register(trees(&["acme.trees.network", "acme.trees.network"])),
        &["already brought"],
    );
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
    assert_refused(r.register(again), &["which acme.forest already brought"]);
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
    fn cook(&self, _: &mut CookCx<'_>, _: &[Data]) -> Result<Outputs, Error> {
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
    assert_refused(
        Registry::new().register(stray),
        &["which no package declares"],
    );
    // A context holding its own.
    let own = Package::new(PackageId::declared("acme.trees"), "Trees")
        .with_context(context("acme.trees.grove"))
        .with_node_type(holder(
            "acme.trees.oak",
            "acme.trees.grove",
            "acme.trees.grove",
        ));
    assert_refused(
        Registry::new().register(own),
        &["would make contexts hold each other"],
    );
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
    assert_refused(
        r.register(loop_),
        &[
            "acme.moss's acme.moss.spore would make contexts hold each other: \
           acme.trees.veins → acme.trees.grove → acme.trees.leaves → acme.trees.veins",
        ],
    );
}

/// An engine's description, by id.
fn engine(id: &'static str) -> kinogaki_render_api::EngineDescription {
    let wake: kinogaki_render_api::Wake = Arc::new(|| {});
    use kinogaki_render_api::RenderEngine;
    (kinogaki_render_api::RecordingEngine::new(wake)
        .description()
        .rebuilt())
    .id(kinogaki_render_api::EngineId::declared(id))
    .build()
}

#[test]
fn an_engine_setting_is_never_named_as_a_render_outputs_own() {
    use kinogaki_render_api::Parameter;
    use kinogaki_studio_api::params::ValueKind;
    let own = Parameter::new("output", "Output", ValueKind::File, "a.exr").group("main");
    let d = (engine("acme.lights.engine").rebuilt())
        .settings(Vec::new(), vec![own])
        .build();
    let package =
        Package::new(PackageId::declared("acme.lights"), "Lights").with_engine_description(d);
    assert_refused(
        Registry::new().register(package),
        &["a render output's own"],
    );
}

/// An engine with its own prim type `ty` beside the standard ones, and
/// output `aov` among its outputs.
fn engine_with(
    id: &'static str,
    ty: &'static str,
    aov: kinogaki_render_api::Aov,
) -> kinogaki_render_api::EngineDescription {
    use kinogaki_render_api::{AovSpec, PrimType, PrimTypeId, Role};
    let d = engine(id);
    let mut capabilities = d.capabilities().clone();
    capabilities
        .types
        .push(PrimType::new(PrimTypeId::declared(ty), "Glow", Role::Light));
    capabilities.aovs.push(AovSpec {
        aov,
        default: false,
    });
    d.rebuilt().capabilities(capabilities).build()
}

#[test]
fn an_engines_own_prim_type_is_declared_once_under_its_vendor_and_its_outputs_are_standard() {
    use kinogaki_render_api::{Aov, AovClass};
    let own = || Aov::new("glowMask", "Glow Mask", AovClass::Scalar);
    let package = |id: &'static str, engine: kinogaki_render_api::EngineDescription| {
        Package::new(PackageId::declared(id), id).with_engine_description(engine)
    };
    let mut registry = Registry::new();
    let first = engine_with("acme.glow.engine", "acme.glow.glow", own());
    assert!(registry.register(package("acme.glow", first)).is_ok());
    let again = engine_with("acme.other.engine", "acme.glow.glow", own());
    assert_refused(
        registry.register(package("acme.other", again)),
        &["prim type acme.glow.glow", "which acme.glow declared"],
    );
    let foreign = engine_with("rival.glow.engine", "acme.glow.halo", own());
    assert_refused(
        registry.register(package("rival.glow", foreign)),
        &["under another vendor's name", "acme.glow.halo"],
    );
    // The standard lights are every engine's; a standard output is as the SDK has it.
    let unlike = Aov::new("albedo", "Albedo", AovClass::Scalar);
    let odd = engine_with("acme.odd.engine", "acme.odd.glow", unlike);
    assert_refused(
        registry.register(package("acme.odd", odd)),
        &["output albedo unlike the standard one"],
    );
}

#[test]
fn an_engine_id_is_declared_once_under_its_vendor() {
    let package = |id: &'static str, engines: &[&'static str]| {
        (engines.iter()).fold(Package::new(PackageId::declared(id), id), |p, e| {
            p.with_engine_description(engine(e))
        })
    };
    let mut registry = Registry::new();
    assert!(
        registry
            .register(package("acme.glow", &["acme.glow.engine"]))
            .is_ok()
    );
    assert_refused(
        registry.register(package("acme.sparks", &["acme.glow.engine"])),
        &["render engine", "which acme.glow declared"],
    );
    assert_refused(
        registry.register(package(
            "twice.glow",
            &["twice.glow.spark", "twice.glow.spark"],
        )),
        &["which twice.glow declared"],
    );
    assert_refused(
        registry.register(package("rival.glow", &["acme.glow.spark"])),
        &["under another vendor's name", "acme.glow.spark"],
    );
    assert!(
        registry
            .register(package("rival.glow", &["rival.glow.spark"]))
            .is_ok()
    );
}

#[cfg(feature = "wgpu")]
struct Glow;

#[cfg(feature = "wgpu")]
impl kinogaki_render_api::display::Engine for Glow {
    fn description(&self) -> kinogaki_render_api::EngineDescription {
        engine("acme.glow.engine")
    }
    fn make(
        &self,
        _: &kinogaki_render_api::display::wgpu::Device,
        _: &kinogaki_render_api::display::wgpu::Queue,
        _: kinogaki_render_api::Wake,
        _: &kinogaki_render_api::display::Options,
    ) -> Result<kinogaki_render_api::display::Made, Error> {
        unreachable!("not made in this test")
    }
}

#[cfg(feature = "wgpu")]
#[test]
fn a_package_makes_the_engines_it_describes() {
    let glow = Package::new(PackageId::declared("acme.glow"), "Glow").with_engine(Glow);
    assert_eq!(glow.engines.len(), 1);
    assert!(Registry::new().register(glow.clone()).is_ok());
    let mut stray = glow;
    stray.engines.clear();
    assert_refused(
        Registry::new().register(stray),
        &["render engine acme.glow.engine, which it doesn't describe"],
    );
}
