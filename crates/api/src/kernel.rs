//! Kernels: a heavy, shared algorithm library one plugin offers every other,
//! which a consumer reaches through core while it is loaded, knowing only the
//! kernel's documentation.
//!
//! No trait or type of a kernel enters the SDK and no consumer links a
//! kernel's crate. A kernel is a package of [`Verb`]s: functions by id, over
//! [`Data`] (the standard values) and [`Parameters`], returning [`Outputs`]. A
//! kernel documents its verbs' ids, inputs, rows and what they read and write;
//! competing kernels offer their verbs under their own vendors.
//!
//! A verb declares its inputs and outputs as a node type does ([`Port`]s): the
//! registry admits it only when their data types are registered, and a run
//! refuses inputs that don't fit them. It runs as a cook does: it reads its
//! rows through a [`CookCx`] (an unset row at its default, a row it doesn't
//! have refused), polls the caller's cancellation, and runs its parallel steps
//! on the [process's workers](kinogaki_geometry::parallel). Nothing of a run is
//! cached or kept: the caller's cook is.
//!
//! The registry is the one table of verbs: registering a package offers its
//! verbs to the process, and refuses one another package offers already.
//!
//! ```
//! use kinogaki_studio_api::kernel::{self, Verb};
//! use kinogaki_studio_api::cook::Port;
//! use kinogaki_studio_api::params::{Number, Parameter, Parameters, Schema, ValueKind};
//! use kinogaki_studio_api::{CancelToken, Outputs, Package, PackageId, VerbId, standard};
//!
//! // The kernel's side: a verb over one Geometry, offered as its package is
//! // registered (a host registers the standard types first).
//! let scale = VerbId::declared("acme.points.scale");
//! let rows = Schema::default()
//!     .parameter(Parameter::new("by", "By", ValueKind::Number(Number::length()), 2.0));
//! let geometry = standard::geometry();
//! let verb = Verb::new(scale, rows, |cx, inputs| {
//!     let value = standard::geometry().get_arc(&inputs[0]).unwrap();
//!     let by = cx.number("by")?;
//!     let moved = value.positions().iter().map(|p| p.map(|v| v * by)).collect::<Vec<_>>();
//!     Ok(Outputs::one(standard::geometry().wrap(value.with_positions(moved.into())?)))
//! })
//! .input(Port::new("geometry", "Geometry", geometry.id()))
//! .output(Port::new("geometry", "Geometry", geometry.id()));
//! assert_eq!(verb.inputs().len(), 1);
//! # let _ = Package::new(PackageId::declared("acme.points"), "Points").with_verb(verb);
//!
//! // The consumer's side: the id, its rows and the standard values alone.
//! let absent = VerbId::declared("acme.points.absent");
//! assert!(kernel::find(absent).is_none());
//! let input = geometry.wrap(kinogaki_studio_api::geometry::Geometry::empty());
//! let missing = kernel::run(absent, &Parameters::new(), &[input], &CancelToken::default(), None);
//! assert!(missing.is_err());
//! ```
use crate::Error;
use crate::cancel::CancelToken;
use crate::cook::{CookCx, Outputs, Port, Project};
use crate::data::Data;
use crate::id::{PackageId, VerbId};
use crate::params::{Parameters, Schema};
use std::collections::BTreeMap;
use std::fmt;
use std::sync::{Arc, LazyLock, RwLock};

/// What a verb runs: its rows in `cx`, its inputs in the slice.
type Run = dyn Fn(&mut CookCx<'_>, &[Data]) -> Result<Outputs, Error> + Send + Sync;

/// One function a kernel offers ([the module](self) says how).
#[derive(Clone)]
pub struct Verb {
    id: VerbId,
    rows: Arc<Schema>,
    inputs: Vec<Port>,
    outputs: Vec<Port>,
    run: Arc<Run>,
}

impl Verb {
    /// Verb `id`, with `rows` at their defaults, running `run`; no inputs or
    /// outputs until [`input`](Self::input) and [`output`](Self::output) add
    /// them.
    pub fn new(
        id: VerbId,
        rows: Schema,
        run: impl Fn(&mut CookCx<'_>, &[Data]) -> Result<Outputs, Error> + Send + Sync + 'static,
    ) -> Self {
        Self {
            id,
            rows: Arc::new(rows),
            inputs: Vec::new(),
            outputs: Vec::new(),
            run: Arc::new(run),
        }
    }

    /// With input `port` after the others (an [`optional`](Port::optional)
    /// one may be left out, as may every one after it).
    pub fn input(mut self, port: Port) -> Self {
        self.inputs.push(port);
        self
    }

    /// With output `port` after the others.
    pub fn output(mut self, port: Port) -> Self {
        self.outputs.push(port);
        self
    }

    /// The inputs it takes, in order.
    pub fn inputs(&self) -> &[Port] {
        &self.inputs
    }

    /// The outputs it gives, in order.
    pub fn outputs(&self) -> &[Port] {
        &self.outputs
    }

    /// Its id.
    pub fn id(&self) -> VerbId {
        self.id
    }

    /// The rows it reads, at their defaults.
    pub fn rows(&self) -> &Schema {
        &self.rows
    }

    /// Runs it on `inputs`, its rows as `parameters` say and otherwise at
    /// their defaults, polling `cancel`, the caller's `project` its
    /// [`CookCx::project`] (whose stored values hold the project's
    /// [`Tolerances`](kinogaki_geometry::Tolerances)).
    ///
    /// # Errors
    /// Inputs that don't fit its ports, a row it doesn't have, cancelled,
    /// or what the verb refuses.
    pub fn run(
        &self,
        parameters: &Parameters,
        inputs: &[Data],
        cancel: &CancelToken,
        project: Option<&Project<'_>>,
    ) -> Result<Outputs, Error> {
        let mut rows = self.rows.values();
        for (name, value) in parameters {
            match rows.get_mut(name) {
                Some(row) => *row = value.clone(),
                None => {
                    let id = self.id;
                    return Err(Error::invalid(format_args!("{id} has no row {name:?}")));
                }
            }
        }
        self.check_inputs(inputs, &rows)?;
        let mut scratch = None;
        let mut cx = CookCx::builder(&rows, cancel, &mut scratch);
        if let Some(project) = project {
            cx = cx.with_project(project);
        }
        let mut cx = cx.build();
        (self.run)(&mut cx, inputs)
    }

    /// Whether `inputs` fit its ports with `rows`: each of a type its port
    /// takes, none past them, none left out that is required.
    fn check_inputs(&self, inputs: &[Data], rows: &Parameters) -> Result<(), Error> {
        let id = self.id;
        if inputs.len() > self.inputs.len() {
            return Err(Error::invalid(format_args!(
                "{id} takes {} inputs, not {}",
                self.inputs.len(),
                inputs.len()
            )));
        }
        if let Some(port) = self.inputs[inputs.len()..]
            .iter()
            .find(|p| p.required(rows))
        {
            return Err(Error::invalid(format_args!(
                "{id} needs input {}",
                port.name
            )));
        }
        for (data, port) in inputs.iter().zip(&self.inputs) {
            if !port.types().any(|ty| ty == data.ty()) {
                let (name, ty) = (&port.name, data.ty());
                return Err(Error::invalid(format_args!(
                    "{id}'s input {name} takes no {ty}"
                )));
            }
        }
        Ok(())
    }
}

impl fmt::Debug for Verb {
    /// The verb by its id.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Verb").field(&self.id).finish()
    }
}

/// The process's verbs, each with the package offering it: the
/// [`Registry`](crate::Registry) writes it as it registers a package, and
/// nothing else does. Packages install once per process and are never
/// unloaded, as the process's [workers](kinogaki_geometry::parallel) are one.
static OFFERED: LazyLock<RwLock<BTreeMap<VerbId, (PackageId, Verb)>>> =
    LazyLock::new(RwLock::default);

/// The package offering verb `id`, when one does.
pub(crate) fn offered_by(id: VerbId) -> Option<PackageId> {
    let table = OFFERED.read().unwrap_or_else(|e| e.into_inner());
    table.get(&id).map(|(package, _)| *package)
}

/// Offers `package`'s `verbs` to the whole process, once the registry
/// admitted it (no other package offers one of them).
pub(crate) fn offer(package: PackageId, verbs: &[Verb]) {
    let mut table = OFFERED.write().unwrap_or_else(|e| e.into_inner());
    table.extend(verbs.iter().map(|verb| (verb.id, (package, verb.clone()))));
}

/// Verb `id`; `None` while no loaded package offers it.
pub fn find(id: VerbId) -> Option<Verb> {
    let table = OFFERED.read().unwrap_or_else(|e| e.into_inner());
    table.get(&id).map(|(_, verb)| verb.clone())
}

/// Runs verb `id` ([`Verb::run`]).
///
/// # Errors
/// [`Missing`](crate::ErrorKind::Missing), naming the verb's package, while no
/// loaded package offers it; otherwise the verb's.
pub fn run(
    id: VerbId,
    parameters: &Parameters,
    inputs: &[Data],
    cancel: &CancelToken,
    project: Option<&Project<'_>>,
) -> Result<Outputs, Error> {
    let verb = find(id).ok_or_else(|| {
        let package = id.package();
        Error::missing(format_args!("{id}: the kernel {package} isn't loaded"))
    })?;
    verb.run(parameters, inputs, cancel, project)
}
