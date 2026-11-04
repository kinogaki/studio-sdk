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
mod handle;
mod incremental;
mod placement;
mod sheet;

pub use binding::{Scalar, Vector};
pub use context::CookCx;
pub use description::{
    Arity, Category, Description, Listing, NodeRole, Port, Ports, Reach, Unwired,
};
pub use handle::{Constraint, Handle, HandleKind, Space};
pub use incremental::{CookPlan, Incremental, Prepared};
pub use placement::Placement;
pub use sheet::{NewCx, PortsCx, RowsFor, SchemaCx};

/// The prefix of the names a node's spares take ([`NodeType::spares`]): no
/// node type's description declares a parameter so named (the
/// [`Registry`](crate::Registry) refuses one that does).
pub const SPARE: &str = "spare-";

/// Core's loop node types, For Each and Loop Input, which a context offers
/// with [`NetworkContext::with_loops`](crate::network::NetworkContext::with_loops).
pub const LOOPS: [&str; 2] = ["kinogaki.core.for_each", "kinogaki.core.loop_input"];

use crate::Diagnostic;
use crate::Error;
use crate::components::Components;
use crate::data::Data;
use crate::data_type::Stamp;
use crate::id::{DataTypeId, NodeTypeId};
use crate::params::{Parameters, Schema, Value};
use std::any::Any;

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
    /// The names of the nodes wired into it (`CookCx::input_name`);
    /// otherwise it sees none.
    pub input_names: bool,
    /// The component selection it acts on ([`CookCx::selection`]);
    /// otherwise it sees none and a selection change doesn't recook it.
    pub selection: bool,
    /// What its path parameters name ([`CookCx::object`],
    /// [`CookCx::path`]): core resolves each of its
    /// [`Path`](crate::params::ValueKind::Path) and
    /// [`Reference`](crate::params::ValueKind::Reference) parameters,
    /// cooks what they name first, and recooks the node when what they
    /// resolve to changes.
    pub objects: bool,
    /// The project it cooks in ([`CookCx::project`]: its units, its
    /// directory, its packages' stored values and the user's); otherwise it sees none
    /// and a change of the project doesn't recook it.
    pub project: bool,
    /// The network it holds ([`CookCx::inside`],
    /// [`Description::holding`](crate::cook::Description::holding)): core
    /// cooks the inside first and recooks the node when it changes;
    /// otherwise the node neither waits for its inside nor recooks with it.
    pub inside: bool,
    /// With [`inside`](Self::inside): core doesn't cook the network first,
    /// and gives the node [`Inside::Passes`] to cook it once per
    /// [`Pass`], as often as it asks (a loop).
    pub passes: bool,
    /// Which pass of a loop it cooks in ([`CookCx::iteration`]): another
    /// number, count or value recooks it; otherwise it sees none.
    pub iteration: bool,
    /// What the pass it cooks in works on ([`CookCx::current`]): another
    /// value recooks it; otherwise it sees none.
    pub current: bool,
}

/// What a holder's cook reads of the network it holds ([`Reads::inside`]).
#[non_exhaustive]
#[derive(Clone, Copy)]
pub enum Inside<'a> {
    /// A walked context's displayed node's outputs (empty when it displays
    /// nothing).
    Walked(&'a Outputs),
    /// A compiled context's plan for its displayed node.
    Compiled(&'a crate::network::Compiled),
    /// The network, to cook once per pass ([`Reads::passes`]).
    Passes(&'a dyn Passes),
}

impl std::fmt::Debug for Inside<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Inside::Walked(outputs) => f.debug_tuple("Walked").field(outputs).finish(),
            Inside::Compiled(plan) => f.debug_tuple("Compiled").field(plan).finish(),
            Inside::Passes(_) => f.write_str("Passes"),
        }
    }
}

/// Which pass of a loop a node cooks in.
#[derive(Clone, Debug, PartialEq)]
pub struct Iteration {
    /// Its number, from 0.
    pub index: usize,
    /// How many passes the loop makes.
    pub count: usize,
    /// What it stands for, unique in the loop: a piece's attribute value, an
    /// element's number. Core keeps each pass's cooks by it, so a pass
    /// whose value and input stay the same doesn't cook when others do.
    pub value: Value,
}

/// One cook of a holder's network: which pass, and what it works on.
#[derive(Clone, Debug)]
pub struct Pass {
    /// Which pass it is.
    pub iteration: Iteration,
    /// What it works on: what a node reading [`Reads::current`] sees.
    pub current: Data,
}

/// A holder's network, to cook once per pass (a loop's body). Core keeps
/// each pass's cooks, polls the holder's cancellation and fails the holder,
/// naming the inner node, when a pass fails.
pub trait Passes: Sync {
    /// What the network yields for each of `passes`, in their order, which
    /// cook at once as workers allow. Passes that depend on one another (a
    /// feedback) come one call at a time.
    ///
    /// # Errors
    /// The first pass failing, or cancellation.
    fn run(&self, passes: Vec<Pass>) -> Result<Vec<std::sync::Arc<Outputs>>, Error>;
}

/// What core resolved one of a node's path parameters to
/// ([`Reads::objects`]). A parameter that names nothing is core's to
/// report: core warns about it on the node (what isn't installed, or that
/// nothing is there), and the node sees no object, or a blank path, and
/// says nothing about it itself.
#[derive(Clone, Debug)]
pub enum Resolved {
    /// What the path names: an object's result, or a node's.
    Object(Data),
    /// The path itself, as core resolved it from the node (absolute), for
    /// a path the node writes rather than reads (a Material's).
    Path(String),
}

/// The project a cook reads ([`Reads::project`]).
#[derive(Clone, Copy)]
pub struct Project<'a> {
    /// The project's units.
    pub units: kinogaki_geometry::Units,
    /// The host's color management, in the project's working space (a
    /// cook reading it cooks again when its revision moves).
    pub colors: &'a dyn crate::color::ColorService,
    /// The directory relative paths resolve against; `None` for a project
    /// not saved yet.
    pub directory: Option<&'a std::path::Path>,
    /// The project's stored values ([`crate::Stored`]).
    pub stored: &'a crate::StoredValues,
    /// The user's stored values ([`Scope::User`](crate::Scope::User)).
    pub user: &'a crate::StoredValues,
    /// The host's image files: a cook waits for what it asks.
    pub files: &'a dyn crate::image_files::ImageFiles,
}

impl std::fmt::Debug for Project<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Project")
            .field("units", &self.units)
            .field("directory", &self.directory)
            .finish_non_exhaustive()
    }
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
    /// Handles placed from this result (an extrusion's distance arrow on
    /// its profile, a blend's radius at its first edge): places fixed, in
    /// the node's output space, numbers bound by name to the node's own
    /// parameters. Studio shows them with the type's declared handles
    /// while the node is the active one. Empty: none.
    pub handles: Vec<Handle>,
}

impl Outputs {
    /// `result` as the only output, carrying no selection, without
    /// warnings or handles.
    pub fn one(result: Data) -> Self {
        Self {
            data: vec![result],
            ..Self::default()
        }
    }

    /// With `handles` placed from the result.
    pub fn with_handles(mut self, handles: Vec<Handle>) -> Self {
        self.handles = handles;
        self
    }

    /// Drops each handle that doesn't [`check`](Handle::check) against
    /// `parameters` (its node type's), with a warning naming it: what a
    /// host does to every cook's handles, as it would refuse their writes.
    pub fn check_handles(&mut self, parameters: &Schema) {
        let warnings = &mut self.warnings;
        self.handles
            .retain(|handle| match handle.check(parameters) {
                Ok(()) => true,
                Err(why) => {
                    let message = format!("The {} handle is left out: {why}", handle.label);
                    warnings.push(Diagnostic::warning(message));
                    false
                }
            });
    }
}

/// What one node type computes. Cooks run on core's workers, several at
/// once (different nodes, and the same node for different times), so a
/// node type holds no per-node state: that is the scratch slot's.
///
/// A node type is [`Any`]: whoever holds a `&dyn NodeType` (a compiler,
/// through [`CompileCx::node_type`](crate::network::CompileCx::node_type))
/// may upcast it to `&dyn Any` and downcast it to its own concrete type,
/// for what only that type knows.
pub trait NodeType: Any + Send + Sync {
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
    /// cancellation and split work with
    /// [`geometry::parallel`](crate::geometry::parallel).
    fn cook(&self, cx: &mut CookCx<'_>, inputs: &[Data]) -> Result<Outputs, Error>;

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

    /// Its ports when they follow its parameters or the network it is in
    /// (a render output's outputs follow its engine's; a File node's
    /// output, what its context's networks carry); `None`: its
    /// description's. Core asks again whenever the node's parameters
    /// change, and when it is pasted into another network.
    fn ports(&self, parameters: &Parameters, cx: &PortsCx) -> Option<Ports> {
        let _ = (parameters, cx);
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
    /// groups). Where they show is `cx.purpose()`: a tool's card
    /// ([`RowsFor::Card`]) may leave out the rows its run isn't adjusted
    /// by, and offers More while it shows fewer than the inspector does.
    /// It shapes rows only: it never changes a parameter, and the same
    /// parameters and `cx` give the same rows. Core asks only while the
    /// node's rows are shown, so it must be quick. `rows` unchanged by
    /// default, for every purpose.
    fn schema(&self, rows: Schema, parameters: &Parameters, cx: &SchemaCx<'_>) -> Schema {
        let _ = (parameters, cx);
        rows
    }

    /// The language of a node's code rows, for the code field that shows
    /// them, from its `parameters` and what `cx` says it reads (a script's
    /// names follow what runs and the input's attributes); plain text by
    /// default. Core asks while the node's rows are built, and keeps the
    /// answer with them.
    fn language(
        &self,
        parameters: &Parameters,
        cx: &SchemaCx<'_>,
    ) -> Option<std::sync::Arc<dyn crate::params::code::CodeLanguage>> {
        let _ = (parameters, cx);
        None
    }

    /// The parameters an edit from `before` to `after` leaves, when the
    /// type answers it; `after` by default. It must be pure and
    /// deterministic: core calls it inside the edit's own transaction
    /// (the same undo step), on every path an edit takes (the inspector,
    /// `NetworkEdit`, paste), so undo, redo and remote edits agree.
    fn edited(&self, before: &Parameters, after: Parameters, cx: &SchemaCx<'_>) -> Parameters {
        let _ = (before, cx);
        after
    }

    /// The parameters a new node starts with, from its description's
    /// defaults (`parameters`) and what `cx` says of the network it is
    /// added to (a Render Out lists the outputs its render writes);
    /// `parameters` by default. Like [`edited`](Self::edited) it must be
    /// pure and deterministic: core calls it inside the creation's own
    /// transaction (the same undo step), on every path that creates a
    /// node (Tab, menus, `NetworkEdit`'s add), never on paste or load,
    /// which keep the parameters they bring.
    fn created(&self, parameters: Parameters, cx: &NewCx<'_>) -> Parameters {
        let _ = cx;
        parameters
    }

    /// What a new node's network starts with
    /// ([`Description::holding`]): core plants it wherever a node is added;
    /// paste and load keep what they bring. Nothing by default.
    fn seed(&self) -> crate::network::NetworkSeed {
        crate::network::NetworkSeed::default()
    }

    /// The parameters pressing button `button` of its rows leaves (Fit to
    /// Group reads the input `cx` gives and writes the bounds it found);
    /// `None`: nothing changes. It must be pure and deterministic, like
    /// [`edited`](Self::edited): core applies the answer as one edit
    /// through the inspector's own path, so `edited` answers it too.
    fn pressed(
        &self,
        button: &str,
        parameters: &Parameters,
        cx: &SchemaCx<'_>,
    ) -> Option<Parameters> {
        let _ = (button, parameters, cx);
        None
    }

    /// Where pressing button `button` of its rows writes the node's value as
    /// a picture (a File Out: its Canvas facet's pieces composited at the
    /// picture's size, in the file's format, through the color service's
    /// pinned sRGB ACES 2.0 SDR view), as the row holds it; `None`: it writes
    /// nothing. Core runs the write off the UI thread and says on the status
    /// line how it went.
    fn writes(&self, button: &str, parameters: &Parameters) -> Option<String> {
        let _ = (button, parameters);
        None
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
    fn validate(&self, parameters: &Parameters) -> Result<(), Error> {
        let _ = parameters;
        Ok(())
    }

    /// A stamp of what a cook with `parameters` reads outside the network
    /// and the project (a file's size and modification time); the node
    /// recooks when it differs. Core doesn't ask it on every cook: it asks
    /// when the parameters or the project change, when the node is told to
    /// reload, and when the outside may have changed unseen (a project
    /// opens, the window is focused again). `None` by default: nothing
    /// outside.
    fn external(&self, parameters: &Parameters, project: Option<&Project<'_>>) -> Option<Stamp> {
        let _ = (parameters, project);
        None
    }
}
