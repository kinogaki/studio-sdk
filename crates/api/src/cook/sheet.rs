//! [`SchemaCx`]: what a node's rows may depend on besides its parameters.
use crate::data::Data;

/// What [`NodeType::schema`](super::NodeType::schema) may shape a node's
/// rows by: which of its input ports are wired, and what each held when the
/// node last cooked (an attribute menu from the input's attributes).
#[derive(Clone, Copy, Debug, Default)]
pub struct SchemaCx<'a> {
    wired: &'a [bool],
    inputs: &'a [Option<Data>],
    kept: Option<&'a [f64]>,
}

impl<'a> SchemaCx<'a> {
    /// Ports `wired` (one a port), and the value each input held at the
    /// node's last cook (`None`: not cooked yet, or nothing wired).
    pub fn new(wired: &'a [bool], inputs: &'a [Option<Data>]) -> Self {
        Self {
            wired,
            inputs,
            kept: None,
        }
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
        assert_eq!(cx.kept(), None);
        let palette = [0.1, 0.2, 0.3];
        assert_eq!(cx.with_kept(Some(&palette)).kept(), Some(&palette[..]));
    }
}
