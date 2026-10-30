//! [`SchemaCx`]: what a node's rows may depend on besides its parameters.
use crate::data::Data;

/// What [`NodeType::schema`](super::NodeType::schema) may shape a node's
/// rows by: which of its input ports are wired, and what each held when the
/// node last cooked (an attribute menu from the input's attributes).
#[derive(Clone, Copy, Debug, Default)]
pub struct SchemaCx<'a> {
    wired: &'a [bool],
    inputs: &'a [Option<Data>],
}

impl<'a> SchemaCx<'a> {
    /// Ports `wired` (one a port), and the value each input held at the
    /// node's last cook (`None`: not cooked yet, or nothing wired).
    pub fn new(wired: &'a [bool], inputs: &'a [Option<Data>]) -> Self {
        Self { wired, inputs }
    }

    /// Whether input `port` has a wire.
    pub fn wired(&self, port: usize) -> bool {
        self.wired.get(port).copied().unwrap_or(false)
    }

    /// What input `port` held at the node's last cook, if it cooked.
    pub fn input(&self, port: usize) -> Option<&'a Data> {
        self.inputs.get(port)?.as_ref()
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
    }
}
