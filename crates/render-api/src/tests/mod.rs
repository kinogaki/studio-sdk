//! The interface's contracts: coalescing, ids, one-shot results, networks.
mod batches;
mod declared_prims;
mod descriptors;

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::*;

pub(super) fn mesh(points: usize) -> Prim {
    Prim::Mesh(Mesh {
        points: Arc::from(vec![[0.0f32; 3]; points]),
        triangles: Arc::from(vec![[0u32, 1, 2]]),
        normals: None,
        uvs: None,
        material_slots: Arc::from(vec![0u16]),
        attributes: Arc::from([]),
        materials: Arc::from(Vec::new()),
        transform: IDENTITY,
        motion: Motion::none(),
        visible: true,
        id: 1,
        path: Arc::from("/mesh"),
    })
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
