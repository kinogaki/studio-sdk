//! The interface's contracts: coalescing, ids, one-shot results, networks.
mod batches;
mod declared_prims;
mod descriptors;

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::*;

pub(super) fn mesh(points: usize) -> Prim {
    let at = vec![[0.0f32; 3]; points].into();
    let triangles = vec![[0u32, 1, 2]].into();
    let shape = Shape::builder(at, Topology::Triangles(triangles))
        .path("/mesh")
        .build()
        .expect("a triangle");
    Prim::Shape(shape)
}

pub(super) fn id(slot: u32) -> PrimId {
    PrimId {
        slot,
        generation: 0,
    }
}

pub(super) fn counter() -> (Wake, Arc<AtomicUsize>) {
    let count = Arc::new(AtomicUsize::new(0));
    let held = count.clone();
    (
        Arc::new(move || {
            held.fetch_add(1, Ordering::SeqCst);
        }),
        count,
    )
}
