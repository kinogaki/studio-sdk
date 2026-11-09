//! Registry admission of homes (`menu`): every offering lists at a topic
//! of each place it is offered at or in its package's family; core has no
//! family; no group is blank; a library's item of a node type takes that
//! type's home; two families never share a label; an engine's Render
//! Output is a creation of core's kind in the engine's family.
use kinogaki_studio_api::cook::{Category, Description, Reach};
use kinogaki_studio_api::menu::{Home, ItemHome, Topic, TopicId, topics};
use kinogaki_studio_api::network::{NetworkContext, NewNode, NodeCreation};
use kinogaki_studio_api::*;
use kinogaki_studio_api_testing::{assert_refused, core};

const TREES: ContextId = ContextId::declared("acme.trees.network");
const MODEL: TopicId = TopicId::declared("model");

/// A node type listed at `home`, offered in `context`'s networks (Trees
/// unless said), reaching as `reach` says.
struct Grow {
    id: &'static str,
    home: Home,
    reach: Reach,
    context: ContextId,
}

impl NodeType for Grow {
    fn id(&self) -> NodeTypeId {
        NodeTypeId::declared(self.id)
    }
    fn description(&self) -> Description {
        Description::new("Grow")
            .context(self.context)
            .reach(self.reach)
            .listed(self.home.clone(), Category::Create, &[])
    }
    fn cook(&self, _: &mut CookCx<'_>, _: &[Data]) -> Result<Outputs, Error> {
        Ok(Outputs::default())
    }
}

/// Package `id` labelled `label` declaring Trees networks (their one topic
/// Model) and Grow at `home`.
fn trees(id: &'static str, label: &str, home: Home) -> Package {
    let context = NetworkContext::new(TREES, "Trees", "trees").topics([Topic::new(MODEL, "Model")]);
    let grow = Grow {
        id: "acme.trees.grow",
        home,
        reach: Reach::Contexts,
        context: TREES,
    };
    Package::new(PackageId::declared(id), label)
        .with_context(context)
        .with_node_type(grow)
}

#[test]
fn a_home_names_a_topic_of_every_place_its_offering_is_listed_at() {
    let ok = |home: Home| core::registry().register(trees("acme.trees", "Trees", home));
    assert!(ok(Home::topic(MODEL).group("Leaves")).is_ok());
    assert!(ok(Home::topic(topics::IMPORT)).is_ok(), "core adds Import");
    assert!(ok(Home::family()).is_ok());
    assert_refused(
        ok(Home::topic(topics::comp::COLOR)),
        &[
            "node type acme.trees.grow names topic color",
            "acme.trees.network's networks lack",
        ],
    );
    // Reaching every network, only Import (or its family) fits.
    let everywhere = |home: Home| {
        let grow = Grow {
            id: "acme.trees.stash",
            home,
            reach: Reach::Everywhere,
            context: TREES,
        };
        core::registry().register(trees("acme.trees", "Trees", Home::family()).with_node_type(grow))
    };
    assert!(everywhere(Home::topic(topics::IMPORT)).is_ok());
    assert_refused(
        everywhere(Home::topic(MODEL)),
        &["names topic model", "every network lack"],
    );
    // An adoption names a topic of its context; an object creation one of
    // the object levels'.
    let adopted = NetworkContext::new(ContextId::declared("acme.trees.grove"), "Grove", "grove")
        .adopts(NodeTypeId::declared("acme.trees.grow"), Home::topic(MODEL));
    let grove = trees("acme.trees", "Trees", Home::family()).with_context(adopted);
    assert_refused(
        core::registry().register(grove),
        &["adoption of acme.trees.grow", "names topic model"],
    );
    let made = |home| {
        let creation = KindCreation::new(CreationId::declared("acme.trees.output"), "Output", home);
        trees("acme.trees", "Trees", Home::family())
            .with_object_creation(render::RENDER_OUTPUT, creation)
    };
    assert!(
        core::registry()
            .register(made(Home::topic(topics::objects::RENDER)))
            .is_ok()
    );
    assert_refused(
        core::registry().register(made(Home::topic(MODEL))),
        &[
            "object creation acme.trees.output names topic model",
            "the object levels lack",
        ],
    );
}

#[test]
fn core_has_no_family_and_no_group_is_blank() {
    let own = ContextId::declared("kinogaki.core.trees");
    let core_node = Grow {
        id: "kinogaki.core.grow",
        home: Home::family(),
        reach: Reach::Contexts,
        context: own,
    };
    let context = NetworkContext::new(own, "Trees", "trees");
    let package = Package::new(PackageId::declared("kinogaki.core"), "Studio")
        .with_context(context)
        .with_node_type(core_node);
    assert_refused(
        Registry::new().register(package),
        &["names a family: core lists under topics"],
    );
    let blank = trees("acme.trees", "Trees", Home::topic(MODEL).group(" "));
    assert_refused(core::registry().register(blank), &["names a blank group"]);
}

#[test]
fn two_families_never_share_a_label() {
    let mut registry = core::registry();
    let preset = |id: &'static str, home: Home| {
        let grow = NewNode::new(NodeTypeId::declared("acme.trees.grow"));
        NodeCreation::new(CreationId::declared(id), "Oak", home, grow)
    };
    registry
        .register(trees("acme.trees", "Trees", Home::family()))
        .unwrap();
    let topical = Package::new(PackageId::declared("acme.oaks"), "Trees")
        .with_creation(preset("acme.oaks.oak", Home::topic(MODEL)));
    registry.register(topical).unwrap();
    let twin = Package::new(PackageId::declared("acme.pines"), "Trees")
        .with_creation(preset("acme.pines.pine", Home::family()));
    assert_refused(
        registry.register(twin),
        &["acme.pines lists in a family labelled Trees, as acme.trees does"],
    );
}

#[test]
fn a_node_types_item_takes_its_home_and_any_other_names_its_own() {
    let item = |name: &str, home: Option<Home>| ItemHome {
        context: TREES,
        item: name.into(),
        home,
        objects: false,
    };
    let with = |items: Vec<ItemHome>| {
        let ui = PackageUi::new(vec![TREES], items, ());
        trees("acme.trees", "Trees", Home::topic(MODEL)).with_package_ui(ui)
    };
    let fine = with(vec![
        item("acme.trees.grow", None),
        item("command:prune", Some(Home::topic(MODEL).group("Cut"))),
    ]);
    assert!(core::registry().register(fine).is_ok());
    assert_refused(
        core::registry().register(with(vec![item("acme.trees.grow", Some(Home::family()))])),
        &["item acme.trees.grow names a home of its own, but it is a node type's item"],
    );
    assert_refused(
        core::registry().register(with(vec![item("command:prune", None)])),
        &["item command:prune names no home"],
    );
    assert_refused(
        core::registry().register(with(vec![item(
            "command:prune",
            Some(Home::topic(topics::comp::COLOR)),
        )])),
        &["names topic color"],
    );
}

#[test]
fn an_engines_render_output_is_a_creation_of_cores_kind_in_its_family() {
    let wake: kinogaki_render_api::Wake = std::sync::Arc::new(|| {});
    use kinogaki_render_api::RenderEngine;
    let engine = (kinogaki_render_api::RecordingEngine::new(wake)
        .description()
        .rebuilt())
    .id(kinogaki_render_api::EngineId::declared("acme.ray.engine"))
    .build();
    let ray = Package::new(PackageId::declared("acme.ray"), "Ray").with_engine_description(engine);
    let [(kind, output)] = ray.object_creations() else {
        panic!("one creation");
    };
    assert_eq!(
        (*kind, output.id.as_str()),
        (render::RENDER_OUTPUT, "acme.ray.engine")
    );
    assert_eq!(
        (output.label.as_str(), &output.home),
        ("Render Output", &Home::family())
    );
    let engine = Value::Text("acme.ray.engine".into());
    assert_eq!(output.values.get(render::ENGINE), Some(&engine));
    assert_refused(
        Registry::new().register(ray.clone()),
        &["kinogaki.core.render_output, which no package declares"],
    );
    assert!(core::registry().register(ray).is_ok());
}
