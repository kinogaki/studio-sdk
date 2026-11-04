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
//! A verb runs as a cook does: it reads its rows through a [`CookCx`] (an unset
//! row at its default, a row it doesn't have refused), polls the caller's
//! cancellation, and runs its parallel steps on the
//! [process's workers](kinogaki_geometry::parallel). Nothing of a run is cached
//! or kept: the caller's cook is.
//!
//! ```
//! use kinogaki_studio_api::kernel::{self, Verb};
//! use kinogaki_studio_api::params::{Number, Parameter, Parameters, Schema, Value, ValueKind};
//! use kinogaki_studio_api::{CancelToken, Outputs, VerbId, standard};
//!
//! // The kernel's side: a verb, offered as its package is installed.
//! let scale = VerbId::declared("acme.points.scale");
//! let rows = Schema::default()
//!     .parameter(Parameter::new("by", "By", ValueKind::Number(Number::length()), 2.0));
//! kernel::offer(&[Verb::new(scale, rows, |cx, inputs| {
//!     let geometry = standard::geometry();
//!     let value = geometry.get_arc(&inputs[0]).unwrap();
//!     let by = cx.number("by")?;
//!     let moved = value.positions().iter().map(|p| p.map(|v| v * by)).collect::<Vec<_>>();
//!     Ok(Outputs::one(geometry.wrap(value.with_positions(moved.into())?)))
//! })]);
//!
//! // The consumer's side: the id, its rows and the standard values alone.
//! let geometry = standard::geometry();
//! let input = geometry.wrap(kinogaki_studio_api::geometry::Geometry::empty());
//! let done = kernel::run(scale, &Parameters::new(), &[input], &CancelToken::default()).unwrap();
//! assert!(geometry.get(&done.data[0]).is_some());
//! let absent = VerbId::declared("acme.points.absent");
//! assert!(kernel::find(absent).is_none());
//! ```
use crate::Error;
use crate::cancel::CancelToken;
use crate::cook::{CookCx, Outputs};
use crate::data::Data;
use crate::id::VerbId;
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
    run: Arc<Run>,
}

impl Verb {
    /// Verb `id`, with `rows` at their defaults, running `run`.
    pub fn new(
        id: VerbId,
        rows: Schema,
        run: impl Fn(&mut CookCx<'_>, &[Data]) -> Result<Outputs, Error> + Send + Sync + 'static,
    ) -> Self {
        Self {
            id,
            rows: Arc::new(rows),
            run: Arc::new(run),
        }
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
    /// their defaults, polling `cancel`.
    ///
    /// # Errors
    /// A row it doesn't have, cancelled, or what the verb refuses.
    pub fn run(
        &self,
        parameters: &Parameters,
        inputs: &[Data],
        cancel: &CancelToken,
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
        let mut scratch = None;
        let mut cx = CookCx::new(&rows, cancel, &mut scratch);
        (self.run)(&mut cx, inputs)
    }
}

impl fmt::Debug for Verb {
    /// The verb by its id.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Verb").field(&self.id).finish()
    }
}

/// The process's verbs. Packages install once per process and are never
/// unloaded, as the process's [workers](kinogaki_geometry::parallel) are one.
static OFFERED: LazyLock<RwLock<BTreeMap<VerbId, Verb>>> = LazyLock::new(RwLock::default);

/// Offers `verbs` to the whole process: the host calls it for a package it
/// admitted ([`Registry`](crate::Registry) refuses duplicate ids), and so does
/// a package's own test. The latest of one id answers.
pub fn offer(verbs: &[Verb]) {
    let mut table = OFFERED.write().unwrap_or_else(|e| e.into_inner());
    table.extend(verbs.iter().map(|verb| (verb.id, verb.clone())));
}

/// Verb `id`; `None` while no loaded package offers it.
pub fn find(id: VerbId) -> Option<Verb> {
    let table = OFFERED.read().unwrap_or_else(|e| e.into_inner());
    table.get(&id).cloned()
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
) -> Result<Outputs, Error> {
    let verb = find(id).ok_or_else(|| {
        let package = id.package();
        Error::missing(format_args!("{id}: the kernel {package} isn't loaded"))
    })?;
    verb.run(parameters, inputs, cancel)
}
