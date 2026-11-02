//! [`SchemaCx`]: what a node's rows may depend on besides its parameters,
//! and [`RowsFor`] where they show; [`NewCx`]: what a new node's
//! parameters may; [`PortsCx`]: what its ports may.
use crate::data::Data;
use crate::id::ContextId;

/// Where a node's rows show: what [`SchemaCx::purpose`] says.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum RowsFor {
    /// The inspector: every row.
    #[default]
    Inspector,
    /// A tool's card over the views: the rows a tool's run is adjusted by
    /// (a type may leave out the rest); `more`: the user asked the card
    /// for more.
    Card {
        /// The card's More is on.
        more: bool,
    },
}

/// What [`NodeType::schema`](super::NodeType::schema) may shape a node's
/// rows by: which of its input ports are wired, what each held when the
/// node last cooked (an attribute menu from the input's attributes), what
/// the node itself made then (what an Auto choice resolved to), and where
/// the rows show.
#[derive(Clone, Copy, Debug, Default)]
pub struct SchemaCx<'a> {
    wired: &'a [bool],
    inputs: &'a [Option<Data>],
    outputs: &'a [Data],
    names: &'a [Vec<String>],
    kept: Option<&'a [f64]>,
    purpose: RowsFor,
}

impl<'a> SchemaCx<'a> {
    /// Ports `wired` (one a port), and the value each input held at the
    /// node's last cook (`None`: not cooked yet, or nothing wired); for the
    /// inspector.
    pub fn new(wired: &'a [bool], inputs: &'a [Option<Data>]) -> Self {
        Self {
            wired,
            inputs,
            outputs: &[],
            names: &[],
            kept: None,
            purpose: RowsFor::Inspector,
        }
    }

    /// For rows shown where `purpose` says.
    pub fn with_purpose(mut self, purpose: RowsFor) -> Self {
        self.purpose = purpose;
        self
    }

    /// Where the rows show.
    pub fn purpose(&self) -> RowsFor {
        self.purpose
    }

    /// With the names of the nodes wired into each port, in wire order.
    pub fn with_input_names(mut self, names: &'a [Vec<String>]) -> Self {
        self.names = names;
        self
    }

    /// With the node's own `outputs` at its last cook, one a port, while
    /// its stamp (its parameters and what it reads) is still the one it
    /// cooked at; empty when it hasn't cooked at that stamp.
    pub fn with_outputs(mut self, outputs: &'a [Data]) -> Self {
        self.outputs = outputs;
        self
    }

    /// With what the node's compiled context's runner keeps for it
    /// (`display::Runner::kept`: a palette it read back).
    pub fn with_kept(mut self, kept: Option<&'a [f64]>) -> Self {
        self.kept = kept;
        self
    }

    /// What the node's runner keeps for it, if anything.
    pub fn kept(&self) -> Option<&'a [f64]> {
        self.kept
    }

    /// Whether input `port` has a wire.
    pub fn wired(&self, port: usize) -> bool {
        self.wired.get(port).copied().unwrap_or(false)
    }

    /// The names of the nodes wired into input `port`, in wire order
    /// (empty when the host didn't say).
    pub fn input_names(&self, port: usize) -> &'a [String] {
        self.names.get(port).map_or(&[], Vec::as_slice)
    }

    /// What input `port` held at the node's last cook, if it cooked.
    pub fn input(&self, port: usize) -> Option<&'a Data> {
        self.inputs.get(port)?.as_ref()
    }

    /// What the node's own output `port` held at its last cook, while its
    /// stamp holds (see [`with_outputs`](Self::with_outputs)). An output
    /// nothing read then may hold an empty value of its type
    /// ([`Outputs::data`](super::Outputs::data)).
    pub fn output(&self, port: usize) -> Option<&'a Data> {
        self.outputs.get(port)
    }
}

/// What [`NodeType::created`](super::NodeType::created) may start a new
/// node's parameters from: what the host knows of the network it is added
/// to.
#[derive(Clone, Copy, Debug)]
pub struct NewCx<'a> {
    context: ContextId,
    outputs: &'a [String],
}

impl<'a> NewCx<'a> {
    /// A node added to a network of `context` whose render writes
    /// `outputs` (its output ids, the beauty first; none outside a
    /// render's network).
    pub fn new(context: ContextId, outputs: &'a [String]) -> Self {
        Self { context, outputs }
    }

    /// The context of the network the node is added to.
    pub fn context(&self) -> ContextId {
        self.context
    }

    /// The outputs the network's render writes, by id, the beauty first
    /// (what [`CompileCx::outputs`](crate::network::CompileCx::outputs)
    /// says when it compiles).
    pub fn outputs(&self) -> &'a [String] {
        self.outputs
    }
}

/// What [`NodeType::ports`](super::NodeType::ports) may shape a node's
/// ports by besides its parameters: the network it is in.
#[derive(Clone, Copy, Debug)]
pub struct PortsCx {
    context: ContextId,
}

impl PortsCx {
    /// A node in a network of `context`.
    pub fn new(context: ContextId) -> Self {
        Self { context }
    }

    /// The context of the node's network.
    pub fn context(&self) -> ContextId {
        self.context
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ports_past_the_ends_are_unwired_and_uncooked() {
        let cx = SchemaCx::new(&[true, false], &[None, None]);
        assert!(cx.wired(0) && !cx.wired(1) && !cx.wired(5));
        assert!(cx.input(0).is_none() && cx.input(9).is_none());
        assert!(cx.output(0).is_none());
        let made =
            [crate::Typed::<f64>::new(crate::DataTypeId::declared("acme.trees.count")).wrap(2.0)];
        let cooked = cx.with_outputs(&made);
        assert!(cooked.output(0).is_some() && cooked.output(1).is_none());
        assert!(!SchemaCx::default().wired(0));
        assert_eq!(cx.kept(), None);
        assert!(cx.input_names(0).is_empty());
        let names = [vec!["a".to_owned(), "b".to_owned()]];
        let named = SchemaCx::new(&[true], &[None]).with_input_names(&names);
        assert_eq!(named.input_names(0), ["a", "b"]);
        assert!(named.input_names(1).is_empty());
        assert_eq!(cx.purpose(), RowsFor::Inspector);
        let card = RowsFor::Card { more: false };
        assert_eq!(cx.with_purpose(card).purpose(), card);
        let palette = [0.1, 0.2, 0.3];
        assert_eq!(cx.with_kept(Some(&palette)).kept(), Some(&palette[..]));
    }

    #[test]
    fn a_new_node_reads_the_render_outputs() {
        let outputs = ["beauty".to_owned(), "depth".to_owned()];
        let cx = NewCx::new(ContextId::declared("acme.trees.network"), &outputs);
        assert_eq!(
            (cx.outputs(), cx.context().as_str()),
            (&outputs[..], "acme.trees.network")
        );
        assert!(
            NewCx::new(ContextId::declared("acme.trees.network"), &[])
                .outputs()
                .is_empty()
        );
    }
}
