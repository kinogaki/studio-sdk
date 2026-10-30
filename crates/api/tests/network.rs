//! Network contexts as packages declare them: registered with their
//! package, refused twice or under another vendor's name; node types
//! holding deltas name a registered data type; conditionally required
//! ports follow parameters.
use kinogaki_studio_api::cook::{Description, Port};
use kinogaki_studio_api::network::{Flag, FlagSet, NetworkContext};
use kinogaki_studio_api::{
    ContextId, CookCx, CookError, Data, DataTypeId, NodeType, NodeTypeId, Outputs, Package,
    PackageId, Parameters, Registry, RegistryError, Value,
};

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
