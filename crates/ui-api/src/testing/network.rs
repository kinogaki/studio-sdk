//! [`FakeNetwork`]: one object's network in memory, edited and cooked
//! the way Studio's core does it, so a package tests its tools' network
//! edits and its node types in a network without Studio.
use kinogaki_studio_api::cook::{Description, NewCx, NodeRole, Ports};
use kinogaki_studio_api::network::{
    Flag, Growth, NetworkContext, NetworkEdit, NetworkRead, NewNode, NodeFlags, NodeInfo, Place,
    Wire,
};
use kinogaki_studio_api::parallel::Serial;
use kinogaki_studio_api::params::Value;
use kinogaki_studio_api::{
    CancelToken, ContextId, CookCx, CookError, Data, DataTypeId, Error, HeldEdit, NodeRef,
    NodeType, NodeTypeId, Outputs, Package, Parameters, SchemaCx,
};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::Arc;

/// The node types and held edits a [`FakeNetwork`] knows, from the
/// packages it was made with.
#[derive(Default)]
struct Types {
    nodes: BTreeMap<NodeTypeId, (Arc<dyn NodeType>, Description)>,
    held: BTreeMap<DataTypeId, Arc<dyn HeldEdit>>,
}

/// One node of a [`FakeNetwork`].
#[derive(Clone, Debug)]
struct Node {
    name: String,
    node_type: NodeTypeId,
    parameters: Parameters,
    inputs: Vec<Vec<Wire>>,
    flags: NodeFlags,
    level: Option<NodeRef>,
    position: [f32; 2],
    delta: Option<Data>,
    kept: Option<Outputs>,
}

/// One network of context `context`, in memory: what a package reads
/// through [`NetworkRead`] and edits through [`NetworkEdit`], with the
/// node types of the packages it was made with. Put it in
/// [`FakeHost::networks`](super::FakeHost::networks) and a package's
/// tools edit it through `HostNetwork` as they would Studio's, one undo
/// step an edit; [`cook`](Self::cook) cooks a node through its type.
///
/// Each edit refuses what Studio refuses, and changes nothing then: an
/// unknown node or type, a parameter the type doesn't declare, a value
/// its parameter doesn't take ([`Parameter::check`]), a wire into a port
/// the node hasn't got or from an output its source hasn't got, a flag
/// the context doesn't offer, a delta for a type that isn't a held edit
/// of it. [`check`](Self::check) is the commit check (no cycles), which
/// [`FakeHost`](super::FakeHost) runs at the end of each edit.
///
/// It is a test double, not Studio's core: it cooks every node anew (no
/// cache, no incremental plans, no time or selection reads), a held node
/// applies its delta to its input as it is now (core captures the base
/// once), and [`Place::Grow`] puts a node after the displayed one
/// ([`Growth::Modify`]) or unwired ([`Growth::Create`], with no Merge),
/// displayed either way.
///
/// ```
/// use kinogaki_studio_api::network::{NetworkContext, NetworkEdit, NetworkRead, NewNode};
/// use kinogaki_studio_api::{ContextId, Package, PackageId};
/// use studio_ui_api::testing::FakeNetwork;
///
/// let context = ContextId::declared("acme.trees.network");
/// let package = Package::new(PackageId::declared("acme.trees"), "Trees")
///     .with_context(NetworkContext::new(context, "Trees", "trees"));
/// let mut network = FakeNetwork::new(context, &[&package]).unwrap();
/// assert!(network.nodes().is_empty());
/// // A type no package brought is refused.
/// let unknown = NewNode::new(kinogaki_studio_api::NodeTypeId::declared("acme.trees.leaf"));
/// assert!(network.add_node(unknown, [0.0, 0.0]).is_err());
/// ```
///
/// [`Parameter::check`]: kinogaki_studio_api::params::Parameter::check
#[derive(Clone)]
pub struct FakeNetwork {
    context: NetworkContext,
    types: Arc<Types>,
    nodes: BTreeMap<NodeRef, Node>,
    /// The displayed node of each level (`None`: the top level).
    displays: BTreeMap<Option<NodeRef>, NodeRef>,
    next: u64,
    revision: u64,
}

impl fmt::Debug for FakeNetwork {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FakeNetwork")
            .field("context", &self.context.id)
            .field("nodes", &self.nodes)
            .field("displays", &self.displays)
            .finish_non_exhaustive()
    }
}

impl FakeNetwork {
    /// An empty network of `context`, which one of `packages` declares
    /// ([`Package::with_context`]), holding nodes of their node types and
    /// applying their held edits.
    ///
    /// # Errors
    /// When none of `packages` declares `context`.
    pub fn new(context: ContextId, packages: &[&Package]) -> Result<Self, Error> {
        let declared = packages
            .iter()
            .flat_map(|p| &p.contexts)
            .find(|c| c.id == context)
            .cloned()
            .ok_or_else(|| Error::invalid(format_args!("no package declares context {context}")))?;
        let mut types = Types::default();
        for package in packages {
            for t in &package.node_types {
                types.nodes.insert(t.id(), (t.clone(), t.description()));
            }
            for h in &package.held_edits {
                types.held.insert(h.delta(), h.clone());
            }
        }
        Ok(Self {
            context: declared,
            types: Arc::new(types),
            nodes: BTreeMap::new(),
            displays: BTreeMap::new(),
            next: 1,
            revision: 0,
        })
    }

    /// Counts the edits that changed it: equal revisions, the same
    /// network.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// The commit check: no wire runs in a cycle.
    ///
    /// # Errors
    /// Naming a node on a cycle.
    pub fn check(&self) -> Result<(), Error> {
        // Depth-first, coloring nodes grey while on the path.
        let mut done = BTreeSet::new();
        for &start in self.nodes.keys() {
            let mut path = BTreeSet::new();
            self.visit(start, &mut path, &mut done)?;
        }
        Ok(())
    }

    fn visit(
        &self,
        node: NodeRef,
        path: &mut BTreeSet<NodeRef>,
        done: &mut BTreeSet<NodeRef>,
    ) -> Result<(), Error> {
        if done.contains(&node) {
            return Ok(());
        }
        if !path.insert(node) {
            let name = &self.nodes[&node].name;
            return Err(Error::invalid(format_args!("{name} feeds itself")));
        }
        for wire in self.nodes[&node].inputs.iter().flatten() {
            self.visit(wire.node, path, done)?;
        }
        path.remove(&node);
        done.insert(node);
        Ok(())
    }

    /// Cooks `node` and what it reads, as core does: a bypassed node
    /// passes its first input's first wire on, a frozen one gives the
    /// outputs it keeps, a held one applies its delta through its held
    /// edit, any other cooks through its type, with [`Serial`] workers and
    /// a scratch slot of its own each time.
    ///
    /// # Errors
    /// The first failing node's error; a node `node` doesn't hold, or a
    /// required input left unwired, fails with a message.
    pub fn cook(&self, node: NodeRef) -> Result<Outputs, CookError> {
        let at = self
            .nodes
            .get(&node)
            .ok_or_else(|| CookError::message(format!("node {} isn't in the network", node.0)))?;
        if let Some(kept) = &at.kept {
            return Ok(kept.clone());
        }
        let first = at.inputs.first().and_then(|wires| wires.first());
        if at.flags.bypass {
            return match first {
                Some(wire) => self.output(*wire).map(Outputs::one),
                None => Err(CookError::message(format!(
                    "{} is bypassed with nothing wired",
                    at.name
                ))),
            };
        }
        let code = &self.types.nodes[&at.node_type].0;
        let ports = self.ports(at);
        let mut inputs = Vec::new();
        let mut wires = Vec::new();
        for (port, list) in ports.inputs.iter().zip(&at.inputs) {
            if list.is_empty() && port.required(&at.parameters) {
                let why = match port.missing.is_empty() {
                    true => format!("{} needs its {} input wired", at.name, port.label),
                    false => port.missing.clone(),
                };
                return Err(CookError::message(why));
            }
            for &wire in list {
                inputs.push(self.output(wire)?);
            }
            wires.push(list.len());
        }
        let cancel = CancelToken::default();
        let mut scratch = None;
        let mut cx = CookCx::new(&at.parameters, &cancel, &Serial, &mut scratch).with_ports(&wires);
        match (&at.delta, code.held()) {
            (Some(delta), Some(ty)) => {
                let held = self.types.held.get(&ty).ok_or_else(|| {
                    CookError::message(format!("no package brought the held edit of {ty}"))
                })?;
                let (base, rest) = inputs.split_first().ok_or_else(|| {
                    CookError::message(format!("{} holds an edit of nothing", at.name))
                })?;
                let rest: Vec<Option<Data>> = rest
                    .iter()
                    .enumerate()
                    .map(|(i, d)| held.reads_input(delta, i + 1).then(|| d.clone()))
                    .collect();
                held.apply(&mut cx, base, delta, &rest)
            }
            _ => code.cook(&mut cx, &inputs),
        }
    }

    /// The value `wire` carries.
    fn output(&self, wire: Wire) -> Result<Data, CookError> {
        let mut outputs = self.cook(wire.node)?;
        let count = outputs.data.len();
        (wire.output < count)
            .then(|| outputs.data.swap_remove(wire.output))
            .ok_or_else(|| CookError::message(format!("output {} of {count}", wire.output)))
    }

    /// `node`'s ports: its type's for its parameters.
    fn ports(&self, node: &Node) -> Ports {
        let (code, description) = &self.types.nodes[&node.node_type];
        code.ports(&node.parameters).unwrap_or_else(|| Ports {
            inputs: description.inputs.clone(),
            outputs: description.outputs.clone(),
        })
    }

    fn node_mut(&mut self, node: NodeRef) -> Result<&mut Node, Error> {
        self.nodes
            .get_mut(&node)
            .ok_or_else(|| Error::invalid(format_args!("node {} isn't in the network", node.0)))
    }

    fn known(&self, node: NodeRef) -> Result<&Node, Error> {
        self.nodes
            .get(&node)
            .ok_or_else(|| Error::invalid(format_args!("node {} isn't in the network", node.0)))
    }

    /// Checks `value` for parameter `name` of a node of `node_type` with
    /// `parameters`: declared (or a spare) and taken.
    fn check_parameter(
        &self,
        node_type: NodeTypeId,
        parameters: &Parameters,
        name: &str,
        value: &Value,
    ) -> Result<(), Error> {
        let (code, description) = &self.types.nodes[&node_type];
        let spares = code.spares(parameters);
        let declared = description
            .parameters
            .parameters()
            .chain(spares.iter().flat_map(|s| s.parameters()))
            .find(|p| p.name == name)
            .cloned()
            .ok_or_else(|| {
                Error::invalid(format_args!(
                    "{} has no parameter {name}",
                    description.label
                ))
            })?;
        declared.check(value).map_err(|d| Error::invalid(d.message))
    }

    /// A name for a new node on `level` as nothing there is called: `name`,
    /// or `name` numbered from 1.
    fn fresh(&self, level: Option<NodeRef>, name: &str, numbered: bool) -> String {
        let taken: BTreeSet<&str> = self
            .nodes
            .values()
            .filter(|n| n.level == level)
            .map(|n| n.name.as_str())
            .collect();
        if !numbered && !taken.contains(name) {
            return name.to_owned();
        }
        (1..)
            .map(|k| format!("{name} {k}"))
            .find(|n| !taken.contains(n.as_str()))
            .expect("a free name")
    }

    fn changed(&mut self) {
        self.revision += 1;
    }
}

impl NetworkRead for FakeNetwork {
    fn context(&self) -> ContextId {
        self.context.id
    }
    fn display(&self) -> Option<NodeRef> {
        self.displays.get(&None).copied()
    }
    fn yielded(&self) -> Option<NodeRef> {
        let output = self.nodes.iter().find(|(_, n)| {
            n.level.is_none() && self.types.nodes[&n.node_type].1.role == NodeRole::Output
        });
        output.map(|(&id, _)| id).or_else(|| self.display())
    }
    fn nodes(&self) -> Vec<NodeRef> {
        self.nodes.keys().copied().collect()
    }
    fn node(&self, node: NodeRef) -> Option<NodeInfo<'_>> {
        let n = self.nodes.get(&node)?;
        Some(NodeInfo {
            name: &n.name,
            node_type: n.node_type,
            parameters: &n.parameters,
            inputs: n.inputs.clone(),
            flags: n.flags,
            level: n.level,
            position: n.position,
            delta: n.delta.as_ref(),
        })
    }
}

impl NetworkEdit for FakeNetwork {
    fn add_node_in(
        &mut self,
        subnet: Option<NodeRef>,
        node: NewNode,
        place: Place,
    ) -> Result<NodeRef, Error> {
        if let Some(subnet) = subnet {
            self.known(subnet)?;
        }
        let (code, description) =
            self.types
                .nodes
                .get(&node.node_type)
                .cloned()
                .ok_or_else(|| {
                    Error::invalid(format_args!("node type {} isn't installed", node.node_type))
                })?;
        let defaults = description.parameters.values();
        let mut parameters = code.created(defaults, &NewCx::default());
        for (name, value) in &node.parameters {
            self.check_parameter(node.node_type, &parameters, name, value)?;
        }
        parameters.extend(node.parameters);
        let (position, after, display) = match place {
            Place::At(position) => (position, None, node.display),
            Place::After {
                node: after,
                position,
            } => (position, Some(after), node.display),
            Place::Grow(growth) => {
                let shown = self.displays.get(&subnet).copied();
                let after = shown.filter(|_| growth == Growth::Modify);
                let below = shown.map_or([0.0, 0.0], |s| {
                    let [x, y] = self.nodes[&s].position;
                    [x, y + 1.0]
                });
                (below, after, true)
            }
        };
        if let Some(after) = after {
            self.known(after)?;
        }
        let ports = match code.ports(&parameters) {
            Some(ports) => ports,
            None => Ports {
                inputs: description.inputs.clone(),
                outputs: description.outputs.clone(),
            },
        };
        let id = NodeRef(self.next);
        let name = match &node.name {
            Some(name) => self.fresh(subnet, name, false),
            None => self.fresh(subnet, &description.label, true),
        };
        self.next += 1;
        self.nodes.insert(
            id,
            Node {
                name,
                node_type: node.node_type,
                parameters,
                inputs: vec![Vec::new(); ports.inputs.len()],
                flags: NodeFlags::default(),
                level: subnet,
                position,
                delta: None,
                kept: None,
            },
        );
        if let Some(after) = after {
            let from = Wire {
                node: after,
                output: 0,
            };
            for (&other, n) in self.nodes.iter_mut() {
                if other == id {
                    continue;
                }
                for wire in n.inputs.iter_mut().flatten() {
                    if *wire == from {
                        wire.node = id;
                    }
                }
            }
            if let Some(first) = self.nodes.get_mut(&id).and_then(|n| n.inputs.first_mut()) {
                first.push(from);
            }
        }
        if display {
            self.displays.insert(subnet, id);
        }
        self.changed();
        Ok(id)
    }

    fn rename(&mut self, node: NodeRef, name: &str) -> Result<(), Error> {
        let level = self.known(node)?.level;
        if self.nodes[&node].name == name {
            return Ok(());
        }
        let name = self.fresh(level, name, false);
        self.node_mut(node)?.name = name;
        self.changed();
        Ok(())
    }

    fn set_position(&mut self, node: NodeRef, position: [f32; 2]) -> Result<(), Error> {
        self.node_mut(node)?.position = position;
        self.changed();
        Ok(())
    }

    fn set_parameter(&mut self, node: NodeRef, name: &str, value: Value) -> Result<(), Error> {
        let n = self.known(node)?;
        self.check_parameter(n.node_type, &n.parameters, name, &value)?;
        if n.parameters.get(name) == Some(&value) {
            return Ok(());
        }
        let mut after = n.parameters.clone();
        after.insert(name.to_owned(), value);
        self.set_parameters(node, after)
    }

    fn set_parameters(&mut self, node: NodeRef, parameters: Parameters) -> Result<(), Error> {
        let n = self.known(node)?;
        for (name, value) in &parameters {
            self.check_parameter(n.node_type, &parameters, name, value)?;
        }
        let (code, description) = &self.types.nodes[&n.node_type];
        if let Some(missing) = description
            .parameters
            .parameters()
            .find(|p| !parameters.contains_key(&p.name))
        {
            return Err(Error::invalid(format_args!(
                "{} needs a value for {}",
                description.label, missing.label
            )));
        }
        let after = code.edited(&n.parameters, parameters, &SchemaCx::default());
        let ports = code.ports(&after);
        let n = self.node_mut(node)?;
        n.parameters = after;
        if let Some(ports) = ports {
            n.inputs.resize(ports.inputs.len(), Vec::new());
        }
        self.changed();
        Ok(())
    }

    fn connect(&mut self, from: Wire, node: NodeRef, port: usize) -> Result<(), Error> {
        let source = self.known(from.node)?;
        let outputs = self.ports(source).outputs.len();
        if from.output >= outputs {
            return Err(Error::invalid(format_args!(
                "{} has no output {}",
                source.name, from.output
            )));
        }
        let target = self.known(node)?;
        let ports = self.ports(target);
        let multi = ports
            .inputs
            .get(port)
            .ok_or_else(|| Error::invalid(format_args!("{} has no input {port}", target.name)))?
            .multi;
        let wires = &mut self.node_mut(node)?.inputs[port];
        if multi {
            wires.push(from);
        } else {
            *wires = vec![from];
        }
        self.changed();
        Ok(())
    }

    fn disconnect(&mut self, node: NodeRef, port: usize, wire: usize) -> Result<(), Error> {
        let n = self.node_mut(node)?;
        let wires = n
            .inputs
            .get_mut(port)
            .filter(|w| wire < w.len())
            .ok_or_else(|| Error::invalid(format_args!("no wire {wire} into input {port}")))?;
        wires.remove(wire);
        self.changed();
        Ok(())
    }

    fn set_flag(&mut self, node: NodeRef, flag: Flag, on: bool) -> Result<(), Error> {
        if !self.context.flags.offers(flag) {
            return Err(Error::invalid(format_args!(
                "{} networks don't offer {flag:?}",
                self.context.label
            )));
        }
        if flag == Flag::Freeze && on {
            return Err(Error::invalid("freezing keeps outputs: call `freeze`"));
        }
        let n = self.node_mut(node)?;
        match flag {
            Flag::Bypass => n.flags.bypass = on,
            Flag::Template => n.flags.template = on,
            Flag::Freeze => {
                n.flags.frozen = false;
                n.kept = None;
            }
        }
        self.changed();
        Ok(())
    }

    fn set_display(&mut self, node: Option<NodeRef>) -> Result<(), Error> {
        match node {
            Some(node) => {
                let level = self.known(node)?.level;
                self.displays.insert(level, node);
            }
            None => {
                self.displays.remove(&None);
            }
        }
        self.changed();
        Ok(())
    }

    fn remove(&mut self, node: NodeRef) -> Result<(), Error> {
        let gone = self.known(node)?.clone();
        if self.nodes.values().any(|n| n.level == Some(node)) {
            return Err(Error::invalid(format_args!("{} holds nodes", gone.name)));
        }
        self.nodes.remove(&node);
        let heal = self
            .context
            .heals
            .then(|| gone.inputs.first().and_then(|w| w.first()).copied())
            .flatten();
        for n in self.nodes.values_mut() {
            for wires in &mut n.inputs {
                let mut kept = Vec::with_capacity(wires.len());
                for &wire in wires.iter() {
                    match (wire.node == node, heal) {
                        (false, _) => kept.push(wire),
                        (true, Some(heal)) => kept.push(heal),
                        (true, None) => {}
                    }
                }
                *wires = kept;
            }
        }
        if self.displays.get(&gone.level) == Some(&node) {
            match heal.map(|w| w.node) {
                Some(to) => self.displays.insert(gone.level, to),
                None => self.displays.remove(&gone.level),
            };
        }
        self.changed();
        Ok(())
    }

    fn set_delta(&mut self, node: NodeRef, delta: Data) -> Result<(), Error> {
        let n = self.known(node)?;
        let (code, description) = &self.types.nodes[&n.node_type];
        if code.held() != Some(delta.ty()) {
            return Err(Error::invalid(format_args!(
                "{} doesn't hold edits of {}",
                description.label,
                delta.ty()
            )));
        }
        let n = self.node_mut(node)?;
        n.delta = Some(delta);
        n.flags.held = true;
        self.changed();
        Ok(())
    }

    fn freeze(&mut self, node: NodeRef, outputs: Outputs) -> Result<(), Error> {
        let n = self.known(node)?;
        let ports = self.ports(n);
        let fits = outputs.data.len() == ports.outputs.len()
            && outputs
                .data
                .iter()
                .zip(&ports.outputs)
                .all(|(d, p)| d.ty() == p.data);
        if !self.context.flags.freeze || !fits {
            return Err(Error::invalid(format_args!(
                "{} can't freeze with those outputs",
                n.name
            )));
        }
        let n = self.node_mut(node)?;
        n.kept = Some(outputs);
        n.flags.frozen = true;
        self.changed();
        Ok(())
    }
}
