//! [`NetworkContext`]: a context's networks, declared as data.
use crate::id::{ContextId, NodeTypeId};
use crate::menu::{Home, Topic, TopicId, topics};

/// A context whose objects hold networks of core's one node shape (a
/// package's `kinogaki.mesh.network`): how they evaluate, the flags their
/// nodes offer and how they are stored. A package registers it with
/// [`Package::with_context`](crate::Package::with_context); an object kind
/// naming it ([`ObjectKindSpec::context`](crate::ObjectKindSpec::context))
/// then holds one in each of its objects.
///
/// ```
/// use kinogaki_studio_api::{ContextId, NodeTypeId};
/// use kinogaki_studio_api::menu::{Home, Topic, TopicId};
/// use kinogaki_studio_api::network::{Flag, Flags, NetworkContext, NetworkKind};
/// let model = TopicId::declared("model");
/// let import = NodeTypeId::declared("acme.mesh.import");
/// let context = NetworkContext::new(ContextId::declared("acme.trees.network"), "Trees", "trees")
///     .flags(Flags::ALL)
///     .topics([Topic::new(model, "Model")])
///     .adopts(import, Home::topic(model));
/// assert!(context.kind == NetworkKind::Chain && context.flags.contains(Flag::Frozen));
/// assert_eq!(context.adopts[0].home, Home::topic(model));
/// let labels: Vec<String> = context.listed_topics().into_iter().map(|t| t.label).collect();
/// assert_eq!(labels, ["Model", "Import"]);
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
    /// Its networks' own topics, in order: the top-level submenus its
    /// offerings and others' list under ([`listed_topics`](Self::listed_topics)
    /// adds core's).
    pub topics: Vec<Topic>,
    /// Other packages' node types its networks offer too, each at a home
    /// of this context's (a trees context's networks taking a mesh
    /// package's Import).
    pub adopts: Vec<Adopted>,
    /// Its networks' results show on the 2D canvas, not in the views: the
    /// canvas comes to the front while one is open.
    pub canvas: bool,
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
    /// Its home in this context's networks, in place of its own listing's.
    pub home: Home,
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
            topics: Vec::new(),
            adopts: Vec::new(),
            canvas: false,
        }
    }

    /// Its own topics `topics`, in order. Naming core's Import or Loops
    /// ([`topics::import`], [`topics::loops`]) places them there.
    pub fn topics(mut self, topics: impl IntoIterator<Item = Topic>) -> Self {
        self.topics = topics.into_iter().collect();
        self
    }

    /// Offering `node_type`, another package's, at `home`.
    pub fn adopts(mut self, node_type: NodeTypeId, home: Home) -> Self {
        self.adopts.push(Adopted { node_type, home });
        self
    }

    /// Offering core's loops (For Each, Loop Input) under Loops: a context
    /// whose networks carry geometry and cook.
    pub fn with_loops(self) -> Self {
        let loops = crate::cook::LOOPS.map(NodeTypeId::declared);
        (loops.into_iter()).fold(self, |context, id| {
            context.adopts(id, Home::topic(topics::LOOPS))
        })
    }

    /// Offering core's Import Object under Import: a context whose networks
    /// cook and read what other networks give.
    pub fn with_import_object(self) -> Self {
        let node = NodeTypeId::declared(crate::cook::IMPORT_OBJECT);
        self.adopts(node, Home::topic(topics::IMPORT))
    }

    /// The topics its networks list, in order: its own, then core's Loops
    /// (when it takes loops) and Import, unless it placed them among its
    /// own.
    pub fn listed_topics(&self) -> Vec<Topic> {
        let named = |id: TopicId| self.topics.iter().any(|t| t.id == id);
        let loops = self
            .adopts
            .iter()
            .any(|a| a.home.topic == Some(topics::LOOPS));
        let mut out = self.topics.clone();
        if loops && !named(topics::LOOPS) {
            out.push(topics::loops());
        }
        if !named(topics::IMPORT) {
            out.push(topics::import());
        }
        out
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

    /// Its networks' results show on the 2D canvas.
    pub fn on_canvas(mut self) -> Self {
        self.canvas = true;
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
