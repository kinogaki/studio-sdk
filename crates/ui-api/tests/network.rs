//! The fake host's networks: a package's node types in an in-memory
//! network, edited through `HostNetwork` as one undo step an edit and
//! cooked as core cooks them.
use kinogaki_studio_api::cook::{NodeType, Port};
use kinogaki_studio_api::network::{
    Flag, FlagSet, Growth, NetworkContext, NetworkEdit, NetworkRead, NewNode, Place, Wire,
};
use kinogaki_studio_api::params::{Number, Parameter, ParameterKind, Value};
use kinogaki_studio_api::{
    ContextId, CookCx, CookError, Data, DataTypeId, Description, NodeRef, NodeTypeId, ObjectRef,
    Outputs, Package, PackageId, Schema, Typed,
};
use std::sync::LazyLock;
use studio_ui_api::host::{HostNetwork, HostUndo};
use studio_ui_api::testing::{FakeHost, FakeNetwork, HostCall};

static NUMBER: LazyLock<Typed<f64>> =
    LazyLock::new(|| Typed::new(DataTypeId::declared("acme.numbers.number")));

fn context() -> ContextId {
    ContextId::declared("acme.numbers.network")
}

const SOURCE: &str = "acme.numbers.source";
const ADD: &str = "acme.numbers.add";

fn value_parameter(name: &str, label: &str, default: f64) -> Parameter {
    Parameter::new(name, label, ParameterKind::Number(Number::plain()), default)
}

/// Makes its `value`.
struct Source;

impl NodeType for Source {
    fn id(&self) -> NodeTypeId {
        NodeTypeId::declared(SOURCE)
    }
    fn description(&self) -> Description {
        Description::new("Source")
            .context(context())
            .output(Port::new("number", "Number", NUMBER.id()))
            .parameters(Schema::default().parameter(value_parameter("value", "Value", 1.0)))
    }
    fn cook(&self, cx: &mut CookCx<'_>, _: &[Data]) -> Result<Outputs, CookError> {
        Ok(Outputs::one(NUMBER.wrap(cx.number("value")?)))
    }
}

/// Adds `amount` to its input.
struct Add;

impl NodeType for Add {
    fn id(&self) -> NodeTypeId {
        NodeTypeId::declared(ADD)
    }
    fn description(&self) -> Description {
        Description::new("Add")
            .context(context())
            .input(Port::new("number", "Number", NUMBER.id()))
            .output(Port::new("number", "Number", NUMBER.id()))
            .parameters(Schema::default().parameter(value_parameter("amount", "Amount", 1.0)))
    }
    fn cook(&self, cx: &mut CookCx<'_>, inputs: &[Data]) -> Result<Outputs, CookError> {
        let input = NUMBER.get(&inputs[0]).copied().unwrap_or_default();
        Ok(Outputs::one(NUMBER.wrap(input + cx.number("amount")?)))
    }
}

fn package() -> Package {
    let numbers = NetworkContext::new(context(), "Numbers", "numbers").flags(FlagSet {
        bypass: true,
        template: false,
        freeze: true,
    });
    Package::new(PackageId::declared("acme.numbers"), "Numbers")
        .with_context(numbers)
        .with_node_type(Source)
        .with_node_type(Add)
}

fn network() -> FakeNetwork {
    FakeNetwork::new(context(), &[&package()]).unwrap()
}

fn value(network: &FakeNetwork, node: NodeRef) -> f64 {
    let outputs = network.cook(node).unwrap();
    *NUMBER.get(&outputs.data[0]).unwrap()
}

fn add(network: &mut FakeNetwork, kind: &str) -> NodeRef {
    let node = NewNode::new(NodeTypeId::declared(kind));
    network.add_node(node, [0.0, 0.0]).unwrap()
}

#[test]
fn nodes_cook_through_their_types_and_core_flags() {
    let mut net = network();
    let source = add(&mut net, SOURCE);
    let a = add(&mut net, ADD);
    net.connect(
        Wire {
            node: source,
            output: 0,
        },
        a,
        0,
    )
    .unwrap();
    net.set_parameter(a, "amount", Value::Number(2.5)).unwrap();
    assert_eq!(value(&net, a), 3.5);
    assert_eq!(net.node(a).unwrap().name, "Add 1");
    // Bypassed, it passes its input on.
    net.set_flag(a, Flag::Bypass, true).unwrap();
    assert_eq!(value(&net, a), 1.0);
    net.set_flag(a, Flag::Bypass, false).unwrap();
    // Frozen, it keeps what it was given.
    net.freeze(a, Outputs::one(NUMBER.wrap(10.0))).unwrap();
    net.set_parameter(source, "value", Value::Number(5.0))
        .unwrap();
    assert_eq!(value(&net, a), 10.0);
    net.set_flag(a, Flag::Freeze, false).unwrap();
    assert_eq!(value(&net, a), 7.5);
    // What the context doesn't offer, or a type doesn't declare, is
    // refused.
    assert!(net.set_flag(a, Flag::Template, true).is_err());
    assert!(net.set_parameter(a, "size", Value::Number(1.0)).is_err());
    assert!(net.set_parameter(a, "amount", Value::Bool(true)).is_err());
    // Unwired, a required input says so.
    let lone = add(&mut net, ADD);
    assert!(net.cook(lone).is_err());
}

#[test]
fn placing_after_a_node_takes_its_readers_and_deleting_heals() {
    let mut net = network();
    let source = add(&mut net, SOURCE);
    let b = add(&mut net, ADD);
    net.connect(
        Wire {
            node: source,
            output: 0,
        },
        b,
        0,
    )
    .unwrap();
    net.set_display(Some(b)).unwrap();
    let after = Place::After {
        node: source,
        position: [0.0, 1.0],
    };
    let a = net
        .add_node_in(None, NewNode::new(NodeTypeId::declared(ADD)), after)
        .unwrap();
    assert_eq!(value(&net, b), 3.0);
    assert_eq!(
        net.node(b).unwrap().inputs[0],
        [Wire { node: a, output: 0 }]
    );
    // Growing a modify takes the display flag after the displayed node.
    let grown = net
        .add_node_in(
            None,
            NewNode::new(NodeTypeId::declared(ADD)),
            Place::Grow(Growth::Modify),
        )
        .unwrap();
    assert_eq!(net.display(), Some(grown));
    assert_eq!(value(&net, grown), 4.0);
    // Deleting heals the chain and moves the display flag back.
    net.remove(grown).unwrap();
    assert_eq!(net.display(), Some(b));
    net.remove(a).unwrap();
    assert_eq!(value(&net, b), 2.0);
}

#[test]
fn a_cycle_fails_the_commit_check() {
    let mut net = network();
    let a = add(&mut net, ADD);
    let b = add(&mut net, ADD);
    net.connect(Wire { node: a, output: 0 }, b, 0).unwrap();
    assert!(net.check().is_ok());
    net.connect(Wire { node: b, output: 0 }, a, 0).unwrap();
    assert!(net.check().is_err());
}

#[test]
fn the_host_edits_networks_as_undo_steps() {
    let object = ObjectRef(7);
    let mut host = FakeHost::new(context(), []);
    host.networks.insert(object, network());
    let mut made = None;
    host.edit_network(object, "Add Source", &mut |net| {
        made = Some(net.add_node(
            NewNode::new(NodeTypeId::declared(SOURCE)).displayed(),
            [0.0, 0.0],
        )?);
        Ok(())
    })
    .unwrap();
    let source = made.unwrap();
    assert_eq!(host.network(object).unwrap().display(), Some(source));
    // A failing edit changes nothing and records no step; nor does a
    // cycle, which the commit check refuses.
    let refused = host.edit_network(object, "Bad", &mut |net| {
        net.set_parameter(source, "value", Value::Number(9.0))?;
        net.set_parameter(source, "nothing", Value::Number(1.0))
    });
    assert!(refused.is_err());
    let cycle = host.edit_network(object, "Cycle", &mut |net| {
        let a = net.add_node(NewNode::new(NodeTypeId::declared(ADD)), [0.0, 0.0])?;
        net.connect(Wire { node: a, output: 0 }, a, 0)
    });
    assert!(cycle.is_err());
    assert_eq!(host.history(), ["Add Source"]);
    assert_eq!(host.networks[&object].cook(source).unwrap().data.len(), 1);
    // Undo and redo bring the network back and forth.
    host.undo();
    assert!(host.network(object).unwrap().nodes().is_empty());
    host.redo();
    assert_eq!(host.network(object).unwrap().nodes(), [source]);
    assert!(
        host.calls
            .contains(&HostCall::EditNetwork(object, "Add Source".into()))
    );
    // An object with no network is an error, not a panic.
    assert!(
        host.edit_network(ObjectRef(8), "None", &mut |_| Ok(()))
            .is_err()
    );
}

/// A Box holds a network of numbers inside it.
struct Box_;

impl NodeType for Box_ {
    fn id(&self) -> NodeTypeId {
        NodeTypeId::declared("acme.boxes.box")
    }
    fn description(&self) -> Description {
        Description::new("Box")
            .context(ContextId::declared("acme.boxes.network"))
            .holding(context())
    }
    fn cook(&self, _: &mut CookCx<'_>, _: &[Data]) -> Result<Outputs, CookError> {
        Ok(Outputs::default())
    }
}

/// A holding node starts with an empty network of its context, edited
/// through `edit_inside` (refused whole when the edit fails) and gone with
/// its node.
#[test]
fn a_node_holds_a_network_of_another_context() {
    let boxes = Package::new(PackageId::declared("acme.boxes"), "Boxes")
        .with_context(NetworkContext::new(
            ContextId::declared("acme.boxes.network"),
            "Boxes",
            "boxes",
        ))
        .with_node_type(Box_);
    let numbers = package();
    let mut net = FakeNetwork::new(
        ContextId::declared("acme.boxes.network"),
        &[&boxes, &numbers],
    )
    .unwrap();
    let held = net
        .add_node(
            NewNode::new(NodeTypeId::declared("acme.boxes.box")),
            [0.0, 0.0],
        )
        .unwrap();
    let inside = net.inside(held).expect("its network");
    assert_eq!(inside.context(), context());
    assert!(inside.nodes().is_empty());
    net.edit_inside(held, &mut |n| {
        n.add_node(NewNode::new(NodeTypeId::declared(SOURCE)), [0.0, 0.0])?;
        Ok(())
    })
    .unwrap();
    assert_eq!(net.inside(held).unwrap().nodes().len(), 1);
    // A failing edit changes nothing.
    let refused = net.edit_inside(held, &mut |n| {
        n.add_node(NewNode::new(NodeTypeId::declared(ADD)), [0.0, 0.0])?;
        n.add_node(
            NewNode::new(NodeTypeId::declared("acme.nothing.here")),
            [0.0, 0.0],
        )?;
        Ok(())
    });
    assert!(refused.is_err());
    assert_eq!(net.inside(held).unwrap().nodes().len(), 1);
    // A node holding nothing has no inside to edit.
    assert!(net.edit_inside(NodeRef(99), &mut |_| Ok(())).is_err());
    net.remove(held).unwrap();
    assert!(net.inside(held).is_none());
}
