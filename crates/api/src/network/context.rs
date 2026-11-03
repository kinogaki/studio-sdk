//! [`NetworkContext`]: a context's networks, declared as data.
use crate::id::{ContextId, NodeTypeId};

/// A context whose objects hold networks of core's one node shape (a
/// package's `kinogaki.mesh.network`): how they evaluate, the flags their
/// nodes offer and how they are stored. A package registers it with
/// [`Package::with_context`](crate::Package::with_context); an object kind
/// naming it ([`ObjectKindSpec::context`](crate::ObjectKindSpec::context))
/// then holds one in each of its objects.
///
/// ```
/// use kinogaki_studio_api::{ContextId, NodeTypeId};
/// use kinogaki_studio_api::network::{Flag, Flags, NetworkContext, NetworkKind};
/// let import = NodeTypeId::declared("acme.mesh.import");
/// let context = NetworkContext::new(ContextId::declared("acme.trees.network"), "Trees", "trees")
///     .flags(Flags::ALL)
///     .adopts(import, "Model");
/// assert!(context.kind == NetworkKind::Chain && context.flags.contains(Flag::Frozen));
/// assert_eq!((context.adopts[0].node_type, context.adopts[0].section.as_str()), (import, "Model"));
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct NetworkContext {
    /// Its id (`acme.trees.network`).
    pub id: ContextId,
    /// What the network pane and menus call it ("Trees").
    pub label: String,
    /// How its networks produce their results.
    pub evaluation: Evaluation,
    /// What its networks are: chains of nodes, or libraries of results.
    pub kind: NetworkKind,
    /// The core flags its nodes offer.
    pub flags: Flags,
    /// How its networks are stored in a project.
    pub storage: NetworkStorage,
    /// How its nodes are drawn in the network pane.
    pub layout: NodeLayout,
    /// Other packages' node types its networks offer too, each listed under
    /// a section of this context's (CAM's networks take Mesh's Import).
    pub adopts: Vec<Adopted>,
}

/// What a context's networks are.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NetworkKind {
    /// Nodes wired into chains: deleting a node joins its first input's
    /// wire to what it fed, closing the chain.
    #[default]
    Chain,
    /// A library of results (Studio's Material Libraries): its nodes take
    /// no wires and show no ports, every node is a result of its network
    /// (a holder's compiled inside, published by path), none carries the
    /// display flag, and they offer no flags.
    Library,
}

/// A node type a context offers though its own package doesn't make it
/// ([`NetworkContext::adopts`]). A node of a type no installed package
/// declares loads as a placeholder, as any missing node type does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Adopted {
    /// The node type (`kinogaki.mesh.import`).
    pub node_type: NodeTypeId,
    /// The heading Tab and the menus list it under in this context
    /// ("Model").
    pub section: String,
}

impl NetworkContext {
    /// Context `id` called `label`, stored under elements spelt `element`:
    /// cooked node by node, healing on delete, offering no flags.
    pub fn new(id: ContextId, label: impl Into<String>, element: impl Into<String>) -> Self {
        Self {
            id,
            label: label.into(),
            evaluation: Evaluation::Cook,
            kind: NetworkKind::Chain,
            flags: Flags::NONE,
            storage: NetworkStorage {
                element: element.into(),
            },
            layout: NodeLayout::Vertical,
            adopts: Vec::new(),
        }
    }

    /// Offering `node_type`, another package's, listed under `section`.
    pub fn adopts(mut self, node_type: NodeTypeId, section: impl Into<String>) -> Self {
        self.adopts.push(Adopted {
            node_type,
            section: section.into(),
        });
        self
    }

    /// Its nodes drawn as `layout` says.
    pub fn layout(mut self, layout: NodeLayout) -> Self {
        self.layout = layout;
        self
    }

    /// Offering `flags`.
    pub fn flags(mut self, flags: Flags) -> Self {
        self.flags = flags;
        self
    }

    /// Compiled, not cooked: its package's
    /// [`NetworkCompiler`](super::NetworkCompiler) plans its networks.
    pub fn compiled(mut self) -> Self {
        self.evaluation = Evaluation::Compile;
        self
    }

    /// A library of results ([`NetworkKind::Library`]), offering no flags.
    pub fn library(mut self) -> Self {
        self.kind = NetworkKind::Library;
        self.flags = Flags::NONE;
        self
    }
}

/// How a context's nodes are drawn in the network pane.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NodeLayout {
    /// Tiles with their inputs on top and outputs below, wires running
    /// down (Houdini's SOPs: Mesh, Comp).
    #[default]
    Vertical,
    /// A header and one row per port, outputs then inputs, wires running
    /// right (shader nodes: Material).
    Horizontal,
}

/// How a context's networks produce their results.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Evaluation {
    /// Core cooks the nodes one by one, each after what it reads, each
    /// through its [`NodeType::cook`](crate::NodeType::cook) (Mesh, CAD).
    #[default]
    Cook,
    /// The context's package compiles the whole network into a plan of its
    /// own, which its executor runs (Comp, shading); core doesn't cook
    /// the nodes.
    Compile,
}

/// One of the core flags a node may carry.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Flag {
    /// Passes input 0's first wire on as output 0, as if the node weren't
    /// there.
    Bypass,
    /// Its result shows as a faint reference beside the displayed one,
    /// never picked.
    Template,
    /// Keeps the outputs it was frozen with; nothing upstream cooks for
    /// it again ([`NetworkEdit::freeze`](super::NetworkEdit::freeze)).
    Frozen,
}

impl Flag {
    fn bit(self) -> u8 {
        1 << self as u8
    }
}

/// A set of [`Flag`]s: what a node carries, or what a context's nodes
/// offer.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Flags(u8);

impl Flags {
    /// No flag.
    pub const NONE: Flags = Flags(0);
    /// Every flag.
    pub const ALL: Flags = Flags(0b111);

    /// `flags`, and nothing else.
    pub fn of(flags: &[Flag]) -> Self {
        flags.iter().fold(Self::NONE, |set, f| set.with(*f, true))
    }

    /// Whether it holds `flag`.
    pub fn contains(self, flag: Flag) -> bool {
        self.0 & flag.bit() != 0
    }

    /// It with `flag` on or off.
    pub fn with(self, flag: Flag, on: bool) -> Self {
        match on {
            true => Flags(self.0 | flag.bit()),
            false => Flags(self.0 & !flag.bit()),
        }
    }
}

/// How a context's networks are stored. Core writes each network as the
/// `network` child of its object's element, typed `element` (`def trees
/// "network" {…}`), with a `def node "n<id>"` element per node: its `type`,
/// name and position, its flags, its wires (`in_<port>` slots connected
/// to `out_<port>` slots), its parameters in one layout every node type
/// shares, and its held delta and kept outputs through their data types'
/// codecs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NetworkStorage {
    /// The network element's spelling and name: a lowercase word, unique
    /// among the elements an object holds (`trees`).
    pub element: String,
}
