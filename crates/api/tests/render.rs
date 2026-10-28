//! The render contract through a toy source: a triangle whose snapshots
//! share their topology and say which points moved, as a drag's do.
use kinogaki_studio_api::render::{Look, MeshAttribute, Ranges};
use kinogaki_studio_api::*;
use std::sync::Arc;

/// One snapshot of a triangle.
struct Snapshot {
    identity: Arc<()>,
    points: Arc<[[f32; 3]]>,
    triangles: Arc<[[u32; 3]]>,
    /// The snapshot it was dragged from, and what moved since.
    parent: Option<(Arc<()>, Ranges)>,
}

impl Snapshot {
    fn first() -> Self {
        Self {
            identity: Arc::new(()),
            points: Arc::from([[0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]]),
            triangles: Arc::from([[0, 1, 2]]),
            parent: None,
        }
    }

    /// Point `point` moved to `to`.
    fn dragged(&self, point: usize, to: [f32; 3]) -> Self {
        let mut points = self.points.to_vec();
        points[point] = to;
        Self {
            identity: Arc::new(()),
            points: points.into(),
            triangles: self.triangles.clone(),
            parent: Some((
                self.identity.clone(),
                Ranges::of(point as u32..point as u32 + 1),
            )),
        }
    }
}

impl TriangleSource for Snapshot {
    fn identity(&self) -> &Arc<()> {
        &self.identity
    }
    fn topology(&self) -> u64 {
        Arc::as_ptr(&self.triangles) as *const () as u64
    }
    fn triangle_count(&self) -> usize {
        self.triangles.len()
    }
    fn triangles(&self) -> Arc<[[u32; 3]]> {
        self.triangles.clone()
    }
    fn points(&self) -> Arc<[[f32; 3]]> {
        self.points.clone()
    }
    fn corner_normals(&self) -> Option<Arc<[[f32; 3]]>> {
        None
    }
    fn corner_uvs(&self) -> Option<Arc<[[f32; 2]]>> {
        None
    }
    fn looks(&self) -> (Arc<[u16]>, Vec<Look>) {
        (Arc::from([u16::MAX]), Vec::new())
    }
    fn columns(&self) -> Vec<u64> {
        Vec::new()
    }
    fn attributes(&self, _: &[Arc<str>], _: &dyn Fn([f32; 3]) -> [f32; 3]) -> Vec<MeshAttribute> {
        Vec::new()
    }
    fn attribute_identity(&self, names: &[Arc<str>]) -> Vec<u64> {
        vec![0; names.len()]
    }
    fn revision(&self) -> u64 {
        0
    }
    fn moved_since(&self, previous: &Arc<()>) -> Option<Ranges> {
        self.parent
            .as_ref()
            .filter(|(parent, _)| Arc::ptr_eq(parent, previous))
            .map(|(_, moved)| moved.clone())
    }
}

#[test]
fn a_drag_is_a_points_update_of_the_same_topology() {
    let first = Snapshot::first();
    let next = first.dragged(1, [2.0, 0.0, 0.0]);
    assert!(!Arc::ptr_eq(first.identity(), next.identity()));
    assert_eq!(first.topology(), next.topology());
    assert_eq!(next.moved_since(first.identity()), Some(Ranges::of(1..2)));
    // Unknown history: the host takes every point as moved.
    let later = next.dragged(0, [0.0, 0.0, 1.0]);
    assert_eq!(later.moved_since(first.identity()), None);
}

#[test]
fn an_item_is_placed_once_by_default() {
    let item = RenderItem::triangles(3, Arc::new(Snapshot::first()));
    let RenderItem::Triangles {
        part,
        placements,
        label,
        ..
    } = &item;
    assert_eq!(
        (*part, &placements[..], label),
        (3, &[Mat4::IDENTITY][..], &None)
    );
    assert_eq!(format!("{item:?}"), "Triangles(part 3, 1 × 1, None)");
}
