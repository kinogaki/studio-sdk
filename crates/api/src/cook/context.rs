//! [`CookCx`]: everything one cook may read besides its inputs.
use crate::cancel::CancelToken;
use crate::components::Components;
use crate::cook::{Error, Inside, Iteration, Project, Resolved, Written};
use crate::data::Data;
use crate::id::ContextId;
use crate::params::{Parameters, ParametersExt, Value};
use crate::refs::NodeRef;
use kinogaki_geometry::{Rate, Time, Xform};
use std::any::Any;
use std::ops::Range;

/// One cook's context, built by core for each cook of a node.
///
/// A host builds it ([`builder`](Self::builder)); a node only reads it. What a node didn't declare in its
/// [`Reads`](crate::cook::Reads) is withheld: a node that doesn't read
/// time sees [`Time::ZERO`] at 24 frames a second, one that doesn't read input names sees
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
    time: Time,
    rate: Rate,
    node: NodeRef,
    resolved: &'a [(String, Resolved)],
    project: Option<&'a Project<'a>>,
    context: Option<ContextId>,
    inside: Option<Inside<'a>>,
    iteration: Option<&'a Iteration>,
    current: Option<&'a Data>,
    scratch: &'a mut Option<Box<dyn Any + Send>>,
}

impl<'a> CookCx<'a> {
    /// A context to build, for a host or a test: `parameters`, polling
    /// `cancel`, lending the node's scratch slot `scratch`; one wire on each
    /// input port, no names, no selection, the default time.
    ///
    /// Core keeps one scratch slot per node and lends it to one cook of
    /// the node at a time; a cook that runs beside it (the same node at
    /// another time) gets a fresh slot that is dropped afterwards. Core
    /// drops a node's slot with the node.
    pub fn builder(
        parameters: &'a Parameters,
        cancel: &'a CancelToken,
        scratch: &'a mut Option<Box<dyn Any + Send>>,
    ) -> CookCxBuilder<'a> {
        CookCxBuilder(Self {
            parameters,
            wires: &[],
            names: &[],
            wanted: &[],
            selection: None,
            incoming: None,
            cancel,
            time: Time::ZERO,
            rate: Rate::FILM,
            node: NodeRef(0),
            resolved: &[],
            project: None,
            context: None,
            inside: None,
            iteration: None,
            current: None,
            scratch,
        })
    }

    /// What the network the node holds gave, when the node reads its inside
    /// ([`Reads::inside`](crate::cook::Reads::inside)); `None` otherwise.
    pub fn inside(&self) -> Option<Inside<'a>> {
        self.inside
    }

    /// Which pass of a loop it cooks in, when it reads that and there is a
    /// loop around it.
    pub fn iteration(&self) -> Option<&'a Iteration> {
        self.iteration
    }

    /// What the pass of a loop it cooks in works on, when it reads that and
    /// there is a loop around it.
    pub fn current(&self) -> Option<&'a Data> {
        self.current
    }

    /// The context of the node's network; `None` when the host didn't say
    /// (a package's own tests).
    pub fn context(&self) -> Option<ContextId> {
        self.context
    }

    /// What path parameter `name` names (an object's result, a node's),
    /// when the node reads objects and core resolved it.
    pub fn object(&self, name: &str) -> Option<&Data> {
        self.resolved.iter().find_map(|(n, r)| match r {
            Resolved::Object(data, _) if n == name => Some(data),
            _ => None,
        })
    }

    /// Where the object path parameter `name` names (or the object holding
    /// the node it names) is placed in the space of the object the node
    /// cooks in, at the cook's time, when the node reads objects and core
    /// resolved it: what places a camera's lens, or brings another object's
    /// geometry into this one's space ([`Resolved::Object`]).
    pub fn placement(&self, name: &str) -> Option<Xform> {
        self.resolved.iter().find_map(|(n, r)| match r {
            Resolved::Object(_, placement) if n == name => Some(*placement),
            _ => None,
        })
    }

    /// Path parameter `name` as core resolved it (absolute), when the node
    /// reads objects and core resolved it to a path: blank when it names
    /// nothing, which core warns about on the node (what isn't installed,
    /// or that nothing is there), so the node doesn't warn about it itself.
    pub fn path(&self, name: &str) -> Option<&str> {
        self.resolved.iter().find_map(|(n, r)| match r {
            Resolved::Path(path) if n == name => Some(path.as_str()),
            _ => None,
        })
    }

    /// What row `name`, fitting [`Fits::Written`](crate::params::Fits::Written),
    /// names; `None` when it names nothing, which core warns about on the node.
    pub fn written(&self, name: &str) -> Option<&Written> {
        self.resolved.iter().find_map(|(n, r)| match r {
            Resolved::Written(written) if n == name => Some(written),
            _ => None,
        })
    }

    /// The project the node cooks in, when it reads the project.
    pub fn project(&self) -> Option<&Project<'a>> {
        self.project
    }

    /// The host's color management, in the project's working space, when
    /// the node reads the project.
    pub fn colors(&self) -> Option<&'a dyn crate::color::ColorService> {
        self.project.map(|p| p.colors)
    }

    /// The host's id for the node cooking: stable while the node exists
    /// and unique in its network, so what the cook names after it (a CAD
    /// node's faces) keeps its names across cooks. 0 outside a network.
    pub fn node(&self) -> NodeRef {
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
    pub fn number(&self, name: &str) -> Result<f64, Error> {
        self.parameters.number(name)
    }

    /// Parameter `name` as a toggle's state, or a failure naming it.
    pub fn toggle(&self, name: &str) -> Result<bool, Error> {
        self.parameters.toggle(name)
    }

    /// Parameter `name` as a whole number, or a failure naming it.
    pub fn integer(&self, name: &str) -> Result<i64, Error> {
        self.parameters.integer(name)
    }

    /// Parameter `name` as a choice's key, or a failure naming it.
    pub fn choice(&self, name: &str) -> Result<&str, Error> {
        self.parameters.choice(name)
    }

    /// Parameter `name` as text, or a failure naming it.
    pub fn text(&self, name: &str) -> Result<&str, Error> {
        self.parameters.text(name)
    }

    /// Data parameter `name`'s value, or a failure naming it.
    pub fn data(&self, name: &str) -> Result<&Data, Error> {
        self.parameters.data(name)
    }

    /// The name of the node behind input `wire` (an index into the inputs),
    /// when the node reads them (`Reads::input_names`); blank when it has
    /// none.
    pub fn input_name(&self, wire: usize) -> &'a str {
        self.names.get(wire).map_or("", String::as_str)
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

    /// `Err(Error::cancelled())` once the cook is cancelled, for `?`.
    pub fn check(&self) -> Result<(), Error> {
        if self.cancel.is_cancelled() {
            Err(Error::cancelled())
        } else {
            Ok(())
        }
    }

    /// The scene time the cook runs at.
    pub fn time(&self) -> Time {
        self.time
    }

    /// The document's frames a second: what [`time`](Self::time) is in
    /// frames ([`Time::frame`]).
    pub fn rate(&self) -> Rate {
        self.rate
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
}

/// A [`CookCx`] being assembled: what a host (or a test standing in for one)
/// lends a cook before it runs. A node never builds one.
pub struct CookCxBuilder<'a>(CookCx<'a>);

impl<'a> CookCxBuilder<'a> {
    /// With what the node's held network gave
    /// ([`Reads::inside`](crate::cook::Reads::inside)): core sets it when the
    /// node reads its inside.
    pub fn with_inside(mut self, inside: Inside<'a>) -> Self {
        self.0.inside = Some(inside);
        self
    }

    /// In pass `iteration` of a loop
    /// ([`Reads::iteration`](crate::cook::Reads::iteration)).
    pub fn with_iteration(mut self, iteration: &'a Iteration) -> Self {
        self.0.iteration = Some(iteration);
        self
    }

    /// Working on `current`, a loop's pass's
    /// ([`Reads::current`](crate::cook::Reads::current)).
    pub fn with_current(mut self, current: &'a Data) -> Self {
        self.0.current = Some(current);
        self
    }

    /// In a network of `context`.
    pub fn with_context(mut self, context: ContextId) -> Self {
        self.0.context = Some(context);
        self
    }

    /// With `wires[p]` wires on input port `p` (a multi-input port may
    /// have none or many). Without it, each input is its own port.
    pub fn with_ports(mut self, wires: &'a [usize]) -> Self {
        self.0.wires = wires;
        self
    }

    /// With the names of the nodes behind the wires, one per input.
    pub fn with_input_names(mut self, names: &'a [String]) -> Self {
        self.0.names = names;
        self
    }

    /// With `wanted[k]` saying whether anything reads output `k`. Without
    /// it every output is wanted.
    pub fn with_wanted(mut self, wanted: &'a [bool]) -> Self {
        self.0.wanted = wanted;
        self
    }

    /// With `selection` as the components the node acts on.
    pub fn with_selection(mut self, selection: &'a Components) -> Self {
        self.0.selection = Some(selection);
        self
    }

    /// With `incoming` as the selection input 0 carries.
    pub fn with_incoming(mut self, incoming: &'a Components) -> Self {
        self.0.incoming = Some(incoming);
        self
    }

    /// At `time`.
    pub fn with_time(mut self, time: Time) -> Self {
        self.0.time = time;
        self
    }

    /// In a document showing `rate` frames a second.
    pub fn with_rate(mut self, rate: Rate) -> Self {
        self.0.rate = rate;
        self
    }

    /// In `project` ([`Reads::project`](crate::cook::Reads::project)).
    pub fn with_project(mut self, project: &'a Project<'a>) -> Self {
        self.0.project = Some(project);
        self
    }

    /// Cooking the node the host knows as `node`.
    pub fn with_node(mut self, node: NodeRef) -> Self {
        self.0.node = node;
        self
    }

    /// With what core resolved the node's path parameters to, by parameter
    /// name ([`Reads::objects`](crate::cook::Reads::objects)).
    pub fn with_resolved(mut self, resolved: &'a [(String, Resolved)]) -> Self {
        self.0.resolved = resolved;
        self
    }

    /// The context, ready to lend.
    pub fn build(self) -> CookCx<'a> {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::Typed;
    use crate::id::DataTypeId;

    /// A node sees what core resolved its path parameters to, by name: an
    /// object's result and where it is placed, or a path.
    #[test]
    fn resolved_paths_are_read_by_parameter_name() {
        let number = Typed::<f64>::new(DataTypeId::declared("test.cx.number"));
        let placed = Xform::translation(kinogaki_geometry::Vec3::new(0.0, 0.0, 5.0));
        let resolved = [
            (
                "source".to_owned(),
                Resolved::Object(number.wrap(2.0), placed),
            ),
            (
                "material".to_owned(),
                Resolved::Path("/Materials/Red".into()),
            ),
        ];
        let parameters = Parameters::new();
        let cancel = CancelToken::default();
        let mut scratch = None;
        let cx = CookCx::builder(&parameters, &cancel, &mut scratch)
            .with_resolved(&resolved)
            .build();
        assert_eq!(cx.object("source").and_then(|d| number.get(d)), Some(&2.0));
        assert_eq!(cx.placement("source"), Some(placed));
        assert_eq!(cx.path("material"), Some("/Materials/Red"));
        assert!(cx.object("material").is_none() && cx.path("source").is_none());
        assert!(cx.placement("material").is_none());
        assert!(cx.project().is_none());
    }

    /// A holder sees what its inside gave only when core sets it.
    #[test]
    fn a_holder_reads_its_inside_when_core_sets_it() {
        let number = Typed::<f64>::new(DataTypeId::declared("test.cx.number"));
        let outputs = crate::cook::Outputs::one(number.wrap(3.0));
        let parameters = Parameters::new();
        let cancel = CancelToken::default();
        let (mut a, mut b) = (None, None);
        let plain = CookCx::builder(&parameters, &cancel, &mut a).build();
        assert!(plain.inside().is_none());
        let holder = CookCx::builder(&parameters, &cancel, &mut b)
            .with_inside(Inside::Walked(&outputs))
            .build();
        match holder.inside() {
            Some(Inside::Walked(o)) => assert_eq!(number.get(&o.data[0]), Some(&3.0)),
            other => panic!("the walked inside, not {other:?}"),
        }
    }
}
