//! Scripts: code a node runs over a standard value, on the host's engine.
//!
//! Core owns the values, so it owns what a script sees of them: a value's
//! elements and typed attributes as fields, its groups, its detail and the
//! queries over them. A code node is rows and a call: its Code row's text and
//! which elements run ([`Over`]) go to [`ScriptEngine::run`], and its other
//! answers follow from the same engine (its spares and whether it reads the
//! time from [`ScriptEngine::facts`], its code field's
//! [`language`](ScriptEngine::language)). A new standard value gains its
//! binding with it, and every node over that value can run scripts.
//!
//! The language is documented in the host's scripting reference. A script
//! reads the input and writes the output; a run is deterministic at any
//! number of threads; a fault fails the run at its line, column and lowest
//! failing element (an [`Error`] whose [`Diagnostic`](crate::Diagnostic) says
//! so), and nothing partial is kept.
use crate::Error;
use crate::cancel::CancelToken;
use crate::cook::Outputs;
use crate::data::Data;
use crate::params::code::CodeLanguage;
use crate::params::{Parameters, Schema};
use kinogaki_geometry::{Domain, Elements, Rate, Time};
use std::fmt::Debug;
use std::sync::{Arc, RwLock};

/// What a script runs once for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Over {
    /// Each element of a domain ([`Domain::Detail`]: once).
    Elements(Domain),
    /// `count` times, its element a number.
    Numbers(usize),
}

/// What code says of itself, whatever it runs over.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CodeFacts {
    /// The rows its declarations make, at their defaults, named for
    /// [`NodeType::spares`](crate::NodeType::spares).
    pub rows: Schema,
    /// It reads the scene time.
    pub reads_time: bool,
}

/// One run of a script.
#[derive(Clone, Copy, Debug)]
pub struct ScriptRun<'a> {
    /// The code.
    pub code: &'a str,
    /// What it runs once for.
    pub over: Over,
    /// Which elements of `over` run; `None`: all.
    pub on: Option<&'a Elements>,
    /// The inputs: the first is the value it changes, the others it may read.
    pub inputs: &'a [Option<Data>],
    /// The node's parameters: its spares' values.
    pub parameters: &'a Parameters,
    /// The scene time, when it reads it.
    pub time: Time,
    /// Frames a second.
    pub rate: Rate,
    /// Its cancellation.
    pub cancel: &'a CancelToken,
}

/// The host's script engine. Every method may be called from any thread.
pub trait ScriptEngine: Send + Sync + Debug {
    /// What `code` says of itself. Quick (the host asks whenever parameters
    /// change), and it reads even code with errors: a line that doesn't parse
    /// says nothing, and the others stand.
    fn facts(&self, code: &str) -> Arc<CodeFacts>;

    /// Runs a script: its result, and warnings in place of errors.
    ///
    /// # Errors
    /// Code that doesn't check or a fault, at its place; a value it has no
    /// binding for; cancelled.
    fn run(&self, run: ScriptRun<'_>) -> Result<Outputs, Error>;

    /// The language of code running `over` on `inputs` (as
    /// [`ScriptRun::inputs`]), for its code field.
    fn language(&self, over: Over, inputs: &[Option<Data>]) -> Arc<dyn CodeLanguage>;
}

/// The process's engine. Packages install once per process and are never
/// unloaded.
static ENGINE: RwLock<Option<Arc<dyn ScriptEngine>>> = RwLock::new(None);

/// Offers `engine` to the whole process: the host calls it for the package
/// that brings one. The latest answers.
pub fn offer(engine: Arc<dyn ScriptEngine>) {
    *ENGINE.write().unwrap_or_else(|e| e.into_inner()) = Some(engine);
}

/// The process's engine.
///
/// # Errors
/// [`Missing`](crate::ErrorKind::Missing) while none is offered.
pub fn engine() -> Result<Arc<dyn ScriptEngine>, Error> {
    let engine = ENGINE.read().unwrap_or_else(|e| e.into_inner()).clone();
    engine.ok_or_else(|| Error::missing("no script engine is installed"))
}
