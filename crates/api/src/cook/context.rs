//! [`CookCx`]: everything one cook may read besides its inputs.
use crate::cancel::CancelToken;
use crate::components::Components;
use crate::cook::{CookError, Project, Resolved};
use crate::data::Data;
use crate::parallel::{self, Workers};
use crate::params::{Parameters, Value};
use crate::time::Time;
use std::any::Any;
use std::ops::Range;
use std::panic::Location;

/// One cook's context, built by core for each cook of a node.
///
/// Core builds it with [`new`](Self::new) and the `with_*` methods; a node
/// reads it. What a node didn't declare in its
/// [`Reads`](crate::cook::Reads) is withheld: a node that doesn't read
/// time sees [`Time::default`], one that doesn't read input names sees
/// none, one that doesn't read the selection sees none.
pub struct CookCx<'a> {
    parameters: &'a Parameters,
    /// Wires per input port, in port order.
    wires: &'a [usize],
    /// The names of the nodes behind the wires, flat like the inputs.
    names: &'a [String],
    /// Whether anything reads each output; empty: every one.
    wanted: &'a [bool],
    selection: Option<&'a Components>,
    incoming: Option<&'a Components>,
    cancel: &'a CancelToken,
    workers: &'a dyn Workers,
    time: Time,
    node: u64,
    resolved: &'a [(String, Resolved)],
    project: Option<&'a Project<'a>>,
    scratch: &'a mut Option<Box<dyn Any + Send>>,
}

impl<'a> CookCx<'a> {
    /// A context with `parameters`, polling `cancel`, running parallel
    /// work on `workers`, lending the node's scratch slot `scratch`: one
    /// wire on each input port, no names, no selection, the default time.
    /// A package's tests pass [`Serial`](crate::parallel::Serial) workers.
    ///
    /// Core keeps one scratch slot per node and lends it to one cook of
    /// the node at a time; a cook that runs beside it (the same node at
    /// another time) gets a fresh slot that is dropped afterwards. Core
    /// drops a node's slot with the node.
    pub fn new(
        parameters: &'a Parameters,
        cancel: &'a CancelToken,
        workers: &'a dyn Workers,
        scratch: &'a mut Option<Box<dyn Any + Send>>,
    ) -> Self {
        Self {
            parameters,
            wires: &[],
            names: &[],
            wanted: &[],
            selection: None,
            incoming: None,
            cancel,
            workers,
            time: Time::default(),
            node: 0,
            resolved: &[],
            project: None,
            scratch,
        }
    }

    /// With `wires[p]` wires on input port `p` (a multi-input port may
    /// have none or many). Without it, each input is its own port.
    pub fn with_ports(mut self, wires: &'a [usize]) -> Self {
        self.wires = wires;
        self
    }

    /// With the names of the nodes behind the wires, one per input.
    pub fn with_input_names(mut self, names: &'a [String]) -> Self {
        self.names = names;
        self
    }

    /// With `wanted[k]` saying whether anything reads output `k`. Without
    /// it every output is wanted.
    pub fn with_wanted(mut self, wanted: &'a [bool]) -> Self {
        self.wanted = wanted;
        self
    }

    /// With `selection` as the components the node acts on.
    pub fn with_selection(mut self, selection: &'a Components) -> Self {
        self.selection = Some(selection);
        self
    }

    /// With `incoming` as the selection input 0 carries.
    pub fn with_incoming(mut self, incoming: &'a Components) -> Self {
        self.incoming = Some(incoming);
        self
    }

    /// At `time`.
    pub fn with_time(mut self, time: Time) -> Self {
        self.time = time;
        self
    }

    /// With what core resolved the node's path parameters to, by parameter
    /// name ([`Reads::objects`](crate::cook::Reads::objects)).
    pub fn with_resolved(mut self, resolved: &'a [(String, Resolved)]) -> Self {
        self.resolved = resolved;
        self
    }

    /// In `project` ([`Reads::project`](crate::cook::Reads::project)).
    pub fn with_project(mut self, project: &'a Project<'a>) -> Self {
        self.project = Some(project);
        self
    }

    /// What path parameter `name` names (an object's result, a node's),
    /// when the node reads objects and core resolved it.
    pub fn object(&self, name: &str) -> Option<&Data> {
        self.resolved.iter().find_map(|(n, r)| match r {
            Resolved::Object(data) if n == name => Some(data),
            _ => None,
        })
    }

    /// Path parameter `name` as core resolved it (absolute), when the node
    /// reads objects and core resolved it to a path.
    pub fn path(&self, name: &str) -> Option<&str> {
        self.resolved.iter().find_map(|(n, r)| match r {
            Resolved::Path(path) if n == name => Some(path.as_str()),
            _ => None,
        })
    }

    /// The project the node cooks in, when it reads the project.
    pub fn project(&self) -> Option<&Project<'a>> {
        self.project
    }

    /// Cooking the node the host knows as `node`.
    pub fn with_node(mut self, node: u64) -> Self {
        self.node = node;
        self
    }

    /// The host's id for the node cooking: stable while the node exists
    /// and unique in its network, so what the cook names after it (a CAD
    /// node's faces) keeps its names across cooks. 0 outside a network.
    pub fn node(&self) -> u64 {
        self.node
    }

    /// Every parameter the node holds, by name.
    pub fn parameters(&self) -> &'a Parameters {
        self.parameters
    }

    /// Parameter `name`'s value; `None` when the node has no such
    /// parameter.
    pub fn parameter(&self, name: &str) -> Option<&Value> {
        self.parameters.get(name)
    }

    /// Parameter `name` as a number, or a failure naming it.
    pub fn number(&self, name: &str) -> Result<f64, CookError> {
        self.typed(name, "a number", Value::number)
    }

    /// Parameter `name` as a toggle's state, or a failure naming it.
    pub fn toggle(&self, name: &str) -> Result<bool, CookError> {
        self.typed(name, "a toggle", Value::bool)
    }

    /// Parameter `name` as a choice's index, or a failure naming it.
    pub fn choice(&self, name: &str) -> Result<usize, CookError> {
        self.typed(name, "a choice", Value::index)
    }

    /// Parameter `name` as text, or a failure naming it.
    pub fn text(&self, name: &str) -> Result<&str, CookError> {
        self.typed(name, "text", Value::text)
    }

    fn typed<'v, T>(
        &'v self,
        name: &str,
        what: &str,
        read: impl Fn(&'v Value) -> Option<T>,
    ) -> Result<T, CookError> {
        self.parameter(name)
            .and_then(read)
            .ok_or_else(|| CookError::message(format!("parameter {name:?} is not {what}")))
    }

    /// The range of `inputs` wired into port `port` (empty when nothing
    /// is).
    pub fn port(&self, port: usize) -> Range<usize> {
        if self.wires.is_empty() {
            return port..port + 1;
        }
        let start: usize = self.wires.iter().take(port).sum();
        start..start + self.wires.get(port).copied().unwrap_or(0)
    }

    /// The name of the node behind input `input`, when the node reads
    /// input names and core gave one.
    pub fn input_name(&self, input: usize) -> Option<&str> {
        self.names
            .get(input)
            .map(String::as_str)
            .filter(|n| !n.is_empty())
    }

    /// Whether anything reads output `output`: a node may skip making an
    /// output nobody reads (see [`Outputs::data`](crate::Outputs::data)).
    /// Output 0, the node's result, is always wanted.
    pub fn wants(&self, output: usize) -> bool {
        output == 0 || self.wanted.get(output).copied().unwrap_or(true)
    }

    /// The components the node acts on (its group, or a held edit's
    /// picks), when it reads the selection and there is one.
    pub fn selection(&self) -> Option<&Components> {
        self.selection
    }

    /// The selection input 0 carries from upstream.
    pub fn incoming(&self) -> Option<&Components> {
        self.incoming
    }

    /// The cook's cancellation.
    pub fn cancel(&self) -> &CancelToken {
        self.cancel
    }

    /// `Err(CookError::Cancelled)` once the cook is cancelled, for `?`.
    pub fn check(&self) -> Result<(), CookError> {
        if self.cancel.is_cancelled() {
            Err(CookError::Cancelled)
        } else {
            Ok(())
        }
    }

    /// The scene time the cook runs at.
    pub fn time(&self) -> Time {
        self.time
    }

    /// The node's scratch value of type `T`, made with `T::default()` when
    /// the slot is empty or holds another type. It keeps per-node state
    /// across cooks (a tessellation cache, an index of the last input);
    /// core may empty the slot at any time (memory pressure, a recook from
    /// scratch), so it must only ever speed a cook up, never change its
    /// result.
    pub fn scratch<T: Any + Send + Default>(&mut self) -> &mut T {
        if !self.scratch.as_ref().is_some_and(|s| s.is::<T>()) {
            *self.scratch = Some(Box::new(T::default()));
        }
        self.scratch
            .as_mut()
            .and_then(|s| s.downcast_mut())
            .expect("the slot holds a T: it was just checked or filled")
    }

    /// Empties the node's scratch slot.
    pub fn clear_scratch(&mut self) {
        *self.scratch = None;
    }

    /// `work(first_index, block)` over disjoint blocks of `values`, at
    /// least `minimum` items each, on the environment's shared workers
    /// (packages never start their own threads). The first error stops
    /// handing out blocks and is returned. The host sizes the run by what
    /// it learnt of this call site.
    #[track_caller]
    pub fn parallel_for<T: Send, E: Send>(
        &self,
        values: &mut [T],
        minimum: usize,
        work: impl Fn(usize, &mut [T]) -> Result<(), E> + Sync,
    ) -> Result<(), E> {
        parallel::for_each_block(self.workers, Location::caller(), values, minimum, work)
    }

    /// `work` over consecutive ranges of `0..len`, `block` items each, on
    /// the shared workers; results in range order.
    #[track_caller]
    pub fn parallel_map<T: Send, E: Send>(
        &self,
        len: usize,
        block: usize,
        work: impl Fn(Range<usize>) -> Result<T, E> + Sync,
    ) -> Result<Vec<T>, E> {
        parallel::map_blocks(self.workers, Location::caller(), len, block, work)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::Typed;
    use crate::id::DataTypeId;
    use crate::parallel::Serial;

    /// A node sees what core resolved its path parameters to, by name: an
    /// object's result, or a path.
    #[test]
    fn resolved_paths_are_read_by_parameter_name() {
        let number = Typed::<f64>::new(DataTypeId::declared("test.cx.number"));
        let resolved = [
            ("source".to_owned(), Resolved::Object(number.wrap(2.0))),
            (
                "material".to_owned(),
                Resolved::Path("/Materials/Red".into()),
            ),
        ];
        let parameters = Parameters::new();
        let cancel = CancelToken::default();
        let mut scratch = None;
        let cx = CookCx::new(&parameters, &cancel, &Serial, &mut scratch).with_resolved(&resolved);
        assert_eq!(cx.object("source").and_then(|d| number.get(d)), Some(&2.0));
        assert_eq!(cx.path("material"), Some("/Materials/Red"));
        assert!(cx.object("material").is_none() && cx.path("source").is_none());
        assert!(cx.project().is_none());
    }
}
