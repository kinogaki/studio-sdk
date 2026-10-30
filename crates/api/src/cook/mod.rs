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
//! ports, parameters and the viewport [`Handle`]s bound to them), which the
//! environment reads once when the package registers.
mod binding;
mod context;
mod description;
mod frame;
mod handle;
mod incremental;
mod sheet;

pub use binding::{Scalar, Vector};
pub use context::CookCx;
pub use description::{Description, NodeRole, Port, Ports};
pub use frame::Frame;
pub use handle::{Constraint, Handle, HandleKind, Space};
pub use incremental::{CookPlan, Incremental, Prepared};
pub use sheet::SchemaCx;

use crate::components::Components;
use crate::data::Data;
use crate::data_type::Stamp;
use crate::error::Error;
use crate::id::{DataTypeId, NodeTypeId};
use crate::params::{Diagnostic, Parameters, Schema};
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
    /// What its path parameters name ([`CookCx::object`],
    /// [`CookCx::path`]): core resolves each of its
    /// [`Path`](crate::params::ParameterKind::Path) and
    /// [`Reference`](crate::params::ParameterKind::Reference) parameters,
    /// cooks what they name first, and recooks the node when what they
    /// resolve to changes.
    pub objects: bool,
    /// The project it cooks in ([`CookCx::project`]: its units, its
    /// directory and its packages' stored values); otherwise it sees none
    /// and a change of the project doesn't recook it.
    pub project: bool,
}

/// What core resolved one of a node's path parameters to
/// ([`Reads::objects`]).
#[derive(Clone, Debug)]
pub enum Resolved {
    /// What the path names: an object's result, or a node's.
    Object(Data),
    /// The path itself, as core resolved it from the node (absolute), for
    /// a path the node writes rather than reads (a Material's).
    Path(String),
}

/// The project a cook reads ([`Reads::project`]).
#[derive(Clone, Copy, Debug)]
pub struct Project<'a> {
    /// The project's units.
    pub units: crate::math::Units,
    /// The directory relative paths resolve against; `None` for a project
    /// not saved yet.
    pub directory: Option<&'a std::path::Path>,
    /// The project's stored values ([`crate::Stored`]).
    pub stored: &'a crate::StoredValues,
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

    /// Its label, ports, parameters and handles, read once when its package
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

    /// Whether a cook with `parameters` reads wire `wire` of input `port`,
    /// which holds `wires` wires: a Switch reads its chosen wire only.
    /// Core leaves the wires it doesn't read out of the cook (nothing
    /// upstream of them cooks for it) and out of the required checks.
    /// Every wire by default. Core asks only types whose
    /// [`partial_reads`](Self::partial_reads) says so.
    fn reads_wire(&self, parameters: &Parameters, port: usize, wire: usize, wires: usize) -> bool {
        let _ = (parameters, port, wire, wires);
        true
    }

    /// Whether [`reads_wire`](Self::reads_wire) ever leaves a wire out:
    /// core asks it again whenever such a node's parameters change.
    fn partial_reads(&self) -> bool {
        false
    }

    /// Its ports when they follow its parameters (a render output's
    /// outputs follow its engine's); `None`: its description's. Core asks
    /// again whenever the node's parameters change.
    fn ports(&self, parameters: &Parameters) -> Option<Ports> {
        let _ = parameters;
        None
    }

    /// The parameters a node with `parameters` holds beyond its
    /// description's (a code node's, declared by its code), each at its
    /// declared default as a description's are: a spare the node holds no
    /// value for yet starts at it, and a reset (a promoted row's too)
    /// returns to it. The inspector shows them after the declared ones,
    /// and their values live in the node's parameters under their own
    /// names. Core asks again whenever the node's parameters change.
    fn spares(&self, parameters: &Parameters) -> Option<Schema> {
        let _ = parameters;
        None
    }

    /// The rows a node with `parameters` shows, shaped from `rows` (its
    /// description's parameters and its spares, at their values) by what
    /// `cx` says: rows enabled or disabled (with the reason) by what is
    /// wired, choices filled from what an input held (its attributes, its
    /// groups). It shapes rows only: it never changes a parameter, and the
    /// same parameters and `cx` give the same rows. Core asks only while
    /// the node's rows are shown, so it must be quick. `rows` unchanged by
    /// default.
    fn schema(&self, rows: Schema, parameters: &Parameters, cx: &SchemaCx<'_>) -> Schema {
        let _ = (parameters, cx);
        rows
    }

    /// The data type of the delta it holds when it is a held edit (an Edit
    /// Mesh holds `kinogaki.mesh.action`); `None` otherwise. Core applies
    /// the delta through the [`HeldEdit`](crate::HeldEdit) registered for
    /// that type, over the input it captured once.
    fn held(&self) -> Option<DataTypeId> {
        None
    }

    /// Checks of `parameters` beyond what its description and its spares
    /// declare: core checks each declared value first with
    /// [`Parameter::check`](crate::params::Parameter::check) (of its kind,
    /// finite, within a number's or an integer's limits).
    fn validate(&self, parameters: &Parameters) -> Result<(), Diagnostic> {
        let _ = parameters;
        Ok(())
    }

    /// A stamp of what a cook with `parameters` reads outside the network
    /// and the project (a file's size and modification time): core asks
    /// it before reusing the node's last cook and recooks when it differs.
    /// It must be quick. `None` by default: nothing outside.
    fn external(&self, parameters: &Parameters, project: Option<&Project<'_>>) -> Option<Stamp> {
        let _ = (parameters, project);
        None
    }
}
