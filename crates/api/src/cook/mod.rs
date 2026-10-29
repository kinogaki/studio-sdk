//! The node cook contract: what a node type computes, and what the
//! environment gives it to compute with.
//!
//! Studio's core owns the cooking DAG for every context: evaluation order,
//! dirty propagation and invalidation, the per-node result cache and its
//! memory budget, latest-wins scheduling and cancellation, worker threads,
//! held edits and time dependency. Packages own only what a node computes:
//! [`NodeType::cook`], optionally [`Incremental`] cooking, and the
//! [`Reads`] that say what besides its inputs and parameters invalidates
//! it. A package never keeps an evaluator, a worker or a private cache:
//! per-node state lives in the core-owned scratch slot
//! ([`CookCx::scratch`]).
//!
//! A node type also says what it is ([`NodeType::description`]: its label,
//! ports and parameters), which the environment reads once when the
//! package registers.
mod context;
mod description;
mod incremental;

pub use context::CookCx;
pub use description::{Description, Port};
pub use incremental::{CookPlan, Incremental, Prepared};

use crate::components::Components;
use crate::data::Data;
use crate::error::Error;
use crate::id::NodeTypeId;
use crate::params::{Diagnostic, Parameters};
use std::fmt;

/// What, besides its inputs and parameters, a node's result depends on.
/// Core recooks a node when one of these changes, and only then: a node
/// that doesn't declare `time` cooks once for every frame. What a node
/// reads may follow from its parameters (a code node reads the time only
/// when its code does).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Reads {
    /// The scene time ([`CookCx::time`]); otherwise the node sees a fixed
    /// time.
    pub time: bool,
    /// The names of the nodes wired into it ([`CookCx::input_name`]);
    /// otherwise it sees none.
    pub input_names: bool,
    /// The component selection it acts on ([`CookCx::selection`]);
    /// otherwise it sees none and a selection change doesn't recook it.
    pub selection: bool,
}

/// What a cook produced.
#[derive(Clone, Debug, Default)]
pub struct Outputs {
    /// One value per output port, in port order; output 0 is the node's
    /// result. An output nothing reads ([`CookCx::wants`]) may hold any
    /// value of its port's type (an empty one): core never hands it on.
    pub data: Vec<Data>,
    /// The component selection output 0 carries downstream (what the next
    /// node sees as [`CookCx::incoming`]).
    pub selection: Option<Components>,
    /// Problems that left the result usable (warnings, notes).
    pub warnings: Vec<Diagnostic>,
}

impl Outputs {
    /// `result` as the only output, carrying no selection, without
    /// warnings.
    pub fn one(result: Data) -> Self {
        Self {
            data: vec![result],
            ..Self::default()
        }
    }
}

/// Why a cook produced nothing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CookError {
    /// It saw its cancellation raised; core drops the cook.
    Cancelled,
    /// It failed; the node shows these (errors first, with spans for code
    /// rows).
    Failed(Vec<Diagnostic>),
}

impl CookError {
    /// A failure saying `message`.
    pub fn message(message: impl Into<String>) -> Self {
        Self::Failed(vec![Diagnostic::error(message)])
    }
}

impl From<Diagnostic> for CookError {
    fn from(diagnostic: Diagnostic) -> Self {
        Self::Failed(vec![diagnostic])
    }
}

impl From<Error> for CookError {
    fn from(error: Error) -> Self {
        match error {
            Error::Cancelled => Self::Cancelled,
            other => Self::message(other.to_string()),
        }
    }
}

impl fmt::Display for CookError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cancelled => f.write_str("cancelled"),
            Self::Failed(diagnostics) => {
                let messages: Vec<&str> = diagnostics.iter().map(|d| d.message.as_str()).collect();
                f.write_str(&messages.join("; "))
            }
        }
    }
}

impl std::error::Error for CookError {}

/// What one node type computes. Cooks run on core's workers, several at
/// once (different nodes, and the same node for different times), so a
/// node type holds no per-node state: that is the scratch slot's.
pub trait NodeType: Send + Sync {
    /// Its id (`kinogaki.mesh.extrude`).
    fn id(&self) -> NodeTypeId;

    /// Its label, ports and parameters, read once when its package
    /// registers.
    fn description(&self) -> Description;

    /// What invalidates a node with `parameters` besides its inputs and
    /// parameters; nothing by default.
    fn reads(&self, parameters: &Parameters) -> Reads {
        let _ = parameters;
        Reads::default()
    }

    /// Computes the node's outputs from `inputs`: the values wired into
    /// it, in port order, a multi-input port's wires one after another
    /// ([`CookCx::port`] says which are whose). Long cooks poll
    /// cancellation and split work with [`CookCx::parallel_for`].
    fn cook(&self, cx: &mut CookCx<'_>, inputs: &[Data]) -> Result<Outputs, CookError>;

    /// Its incremental cooking, when it has any (an interactive drag of a
    /// parameter then reuses prepared work).
    fn incremental(&self) -> Option<&dyn Incremental> {
        None
    }
}
