//! [`NetworkContext`]: a context's networks, declared as data.
use crate::id::ContextId;

/// A context whose objects hold networks of core's one node shape (a
/// package's `kinogaki.mesh.network`): how they evaluate, the flags their
/// nodes offer and how they are stored. A package registers it with
/// [`Package::with_context`](crate::Package::with_context); an object kind
/// naming it ([`ObjectKindSpec::context`](crate::ObjectKindSpec::context))
/// then holds one in each of its objects.
///
/// ```
/// use kinogaki_studio_api::ContextId;
/// use kinogaki_studio_api::network::{FlagSet, NetworkContext};
/// let context = NetworkContext::new(ContextId::declared("acme.trees.network"), "Trees", "trees")
///     .flags(FlagSet::all());
/// assert!(context.heals && context.flags.freeze);
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct NetworkContext {
    /// Its id (`acme.trees.network`).
    pub id: ContextId,
    /// What the network pane and menus call it ("Trees").
    pub label: String,
    /// How its networks produce their results.
    pub evaluation: Evaluation,
    /// Whether deleting a node joins its first input's wire to what it
    /// fed, closing the chain; otherwise what it fed is left unwired.
    pub heals: bool,
    /// The core flags its nodes offer.
    pub flags: FlagSet,
    /// How its networks are stored in a project.
    pub storage: NetworkStorage,
    /// How its nodes are drawn in the network pane.
    pub layout: NodeLayout,
}

impl NetworkContext {
    /// Context `id` called `label`, stored under elements spelt `element`:
    /// cooked node by node, healing on delete, offering no flags.
    pub fn new(id: ContextId, label: impl Into<String>, element: impl Into<String>) -> Self {
        Self {
            id,
            label: label.into(),
            evaluation: Evaluation::Cook,
            heals: true,
            flags: FlagSet::default(),
            storage: NetworkStorage {
                element: element.into(),
            },
            layout: NodeLayout::Vertical,
        }
    }

    /// Its nodes drawn as `layout` says.
    pub fn layout(mut self, layout: NodeLayout) -> Self {
        self.layout = layout;
        self
    }

    /// Offering `flags`.
    pub fn flags(mut self, flags: FlagSet) -> Self {
        self.flags = flags;
        self
    }

    /// Compiled, not cooked: its package's
    /// [`NetworkCompiler`](super::NetworkCompiler) plans its networks.
    pub fn compiled(mut self) -> Self {
        self.evaluation = Evaluation::Compile;
        self
    }

    /// Healing deleted nodes' chains or not.
    pub fn heals(mut self, heals: bool) -> Self {
        self.heals = heals;
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
    /// Keeps the outputs it has now; nothing upstream cooks for it again
    /// ([`NetworkEdit::freeze`](super::NetworkEdit::freeze)).
    Freeze,
}

/// The core flags a context's nodes offer.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FlagSet {
    /// [`Flag::Bypass`].
    pub bypass: bool,
    /// [`Flag::Template`].
    pub template: bool,
    /// [`Flag::Freeze`].
    pub freeze: bool,
}

impl FlagSet {
    /// Every flag.
    pub fn all() -> Self {
        Self {
            bypass: true,
            template: true,
            freeze: true,
        }
    }

    /// Whether it offers `flag`.
    pub fn offers(self, flag: Flag) -> bool {
        match flag {
            Flag::Bypass => self.bypass,
            Flag::Template => self.template,
            Flag::Freeze => self.freeze,
        }
    }
}

/// How a context's networks are stored. Core writes each network as one
/// child element of its object's element, spelt `element` (`def trees
/// "trees" {…}`), with a `def node "n<id>"` element per node: its `type`,
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
