//! Networks: every object whose kind names a context
//! ([`ObjectKindSpec::context`](crate::ObjectKindSpec::context)) holds a
//! network of that context, and Studio's core owns it. Core creates,
//! edits, validates, cooks, undoes and saves it, as Houdini's host owns
//! its node graphs; packages supply the node types
//! ([`NodeType`](crate::NodeType)).
//!
//! - **Contexts** ([`NetworkContext`]) are declared as data: how the
//!   context's networks evaluate, which flags their nodes offer, whether
//!   deleting a node heals the chain, and the element they are stored
//!   under.
//! - **Reading** ([`NetworkRead`]): a network's nodes, their types,
//!   parameters, wires and flags, by the host's ids.
//! - **Editing** ([`NetworkEdit`]): the edits a package makes to a network
//!   through the host (add a node, set a parameter, wire, flag, delete,
//!   hold a delta, freeze). The host makes each edit one undo step and
//!   checks the network once, when the edit is committed.
//!
//! - **Compiling** ([`NetworkCompiler`]): a context declared
//!   `Evaluation::Compile` isn't cooked node by node; its package's
//!   compiler turns the flattened network into a plan its executor runs.
//!
//! Every node of every context is one shape: its type, its
//! [`Parameters`](crate::Parameters), its input wires, the core flags,
//! and optionally a held delta (an Edit Mesh's) or the outputs it keeps
//! while frozen. Projects store a node's parameters in one layout for
//! every type; a held delta and kept outputs save through their data
//! types' codecs.
mod compile;
mod context;
mod edit;
mod read;

pub use compile::{CompileCx, CompileError, Flat, FlatNode, NetworkCompiler, Source};
pub use context::{Evaluation, Flag, FlagSet, NetworkContext, NetworkStorage};
pub use edit::{NetworkEdit, NewNode};
pub use read::{NetworkRead, NodeFlags, NodeInfo, Wire};
