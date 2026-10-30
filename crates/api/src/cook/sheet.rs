//! [`SchemaCx`]: what a node's rows may depend on besides its parameters;
//! [`NewCx`]: what a new node's parameters may.
use crate::data::Data;

/// What [`NodeType::schema`](super::NodeType::schema) may shape a node's
/// rows by: which of its input ports are wired, and what each held when the
/// node last cooked (an attribute menu from the input's attributes).
#[derive(Clone, Copy, Debug, Default)]
pub struct SchemaCx<'a> {
    wired: &'a [bool],
    inputs: &'a [Option<Data>],
    names: &'a [Vec<String>],
    kept: Option<&'a [f64]>,
}

impl<'a> SchemaCx<'a> {
    /// Ports `wired` (one a port), and the value each input held at the
    /// node's last cook (`None`: not cooked yet, or nothing wired).
    pub fn new(wired: &'a [bool], inputs: &'a [Option<Data>]) -> Self {
        Self {
            wired,
            inputs,
            names: &[],
            kept: None,
        }
    }

    /// With the names of the nodes wired into each port, in wire order.
    pub fn with_input_names(mut self, names: &'a [Vec<String>]) -> Self {
        self.names = names;
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
}

/// What [`NodeType::created`](super::NodeType::created) may start a new
/// node's parameters from: what the host knows of the network it is added
/// to.
#[derive(Clone, Copy, Debug, Default)]
pub struct NewCx<'a> {
    outputs: &'a [String],
}

impl<'a> NewCx<'a> {
    /// A node added where the render writes `outputs` (its output ids, the
    /// beauty first; none outside a render's network).
    pub fn new(outputs: &'a [String]) -> Self {
        Self { outputs }
    }

    /// The outputs the network's render writes, by id, the beauty first
    /// (what [`CompileCx::outputs`](crate::network::CompileCx::outputs)
    /// says when it compiles).
    pub fn outputs(&self) -> &'a [String] {
        self.outputs
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
        assert!(!SchemaCx::default().wired(0));
        assert_eq!(cx.kept(), None);
        assert!(cx.input_names(0).is_empty());
        let names = [vec!["a".to_owned(), "b".to_owned()]];
        let named = SchemaCx::new(&[true], &[None]).with_input_names(&names);
        assert_eq!(named.input_names(0), ["a", "b"]);
        assert!(named.input_names(1).is_empty());
        let palette = [0.1, 0.2, 0.3];
        assert_eq!(cx.with_kept(Some(&palette)).kept(), Some(&palette[..]));
    }

    #[test]
    fn a_new_node_reads_the_render_outputs() {
        let outputs = ["beauty".to_owned(), "depth".to_owned()];
        assert_eq!(NewCx::new(&outputs).outputs(), outputs);
        assert!(NewCx::default().outputs().is_empty());
    }
}
