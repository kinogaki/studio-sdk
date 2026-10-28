//! [`Incremental`] cooking: a node prepares reusable work once and updates
//! from it while only light parameters change (an Extrude's distance
//! during a drag). This is lib-mesh's `cook_prepared` and `ToolPlan` as a
//! node capability.
use crate::cook::{CookCx, CookError, Outputs};
use crate::data::Data;
use std::sync::Arc;

/// A full cook's outputs and, when the node can update from it, its plan.
#[derive(Clone)]
pub struct Prepared {
    /// What the cook produced.
    pub outputs: Outputs,
    /// The reusable work; `None`: the next cook is a full one.
    pub plan: Option<Arc<dyn CookPlan>>,
}

/// A node type's incremental cooking.
pub trait Incremental: Send + Sync {
    /// Cooks in full, keeping what later updates can reuse. Core calls
    /// this instead of [`NodeType::cook`](crate::NodeType::cook) when it
    /// expects updates (the user started dragging a parameter).
    fn prepare(&self, cx: &mut CookCx<'_>, inputs: &[Data]) -> Result<Prepared, CookError>;
}

/// Prepared, immutable work. Plans belong to core's per-node cache, never
/// to the project or the undo history; core drops one when the node's
/// inputs, selection or structural parameters change, or when memory runs
/// short.
pub trait CookPlan: Send + Sync {
    /// The outputs for the current parameters, from the plan; `Ok(None)`
    /// when the plan no longer applies (core then rebuilds it, through
    /// [`reprepare`](Self::reprepare) or a new
    /// [`Incremental::prepare`]).
    fn update(&self, cx: &mut CookCx<'_>, inputs: &[Data]) -> Result<Option<Outputs>, CookError>;

    /// A new plan after a structural change, keeping what of this one still
    /// holds (an unchanged spatial index); `Ok(None)`: prepare anew.
    fn reprepare(
        &self,
        cx: &mut CookCx<'_>,
        inputs: &[Data],
    ) -> Result<Option<Prepared>, CookError> {
        let _ = (cx, inputs);
        Ok(None)
    }

    /// Bytes it holds, for core's cache budget.
    fn memory(&self) -> usize;
}
