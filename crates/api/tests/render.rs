//! The render contract through a toy source: a triangle whose snapshots
//! share their topology and say which points moved, as a drag's do.
use kinogaki_studio_api::geometry::{Attributes, Column};
use kinogaki_studio_api::render::{Ranges, Topology};
use kinogaki_studio_api::*;
use std::sync::Arc;

/// One snapshot of a triangle.
struct Snapshot {
    identity: Arc<()>,
    points: Column<[f32; 3]>,
    triangles: Column<[u32; 3]>,
    /// The snapshot it was dragged from, and what moved since.
    parent: Option<(Arc<()>, Ranges)>,
}

impl Snapshot {
    fn first() -> Self {
        Self {
            identity: Arc::new(()),
            points: vec![[0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]].into(),
            triangles: vec![[0, 1, 2]].into(),
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

impl ShapeSource for Snapshot {
    fn identity(&self) -> &Arc<()> {
        &self.identity
    }
    fn topology(&self) -> Topology {
        Topology::Triangles(self.triangles.clone())
    }
    fn points(&self) -> Column<[f32; 3]> {
        self.points.clone()
    }
    fn primvars(&self, _: &[Arc<str>]) -> Attributes {
        Attributes::new()
    }
    fn subsets(&self) -> (Column<u16>, Vec<Arc<str>>) {
        (Column::default(), Vec::new())
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
    assert_eq!(first.topology().key(), next.topology().key());
    assert_eq!(next.moved_since(first.identity()), Some(Ranges::of(1..2)));
    // Unknown history: the host takes every point as moved.
    let later = next.dragged(0, [0.0, 0.0, 1.0]);
    assert_eq!(later.moved_since(first.identity()), None);
}

#[test]
fn an_item_names_its_part_and_says_what_it_is() {
    let item = RenderItem::shape(3, Arc::new(Snapshot::first()));
    assert_eq!(item.part(), 3);
    assert_eq!(format!("{item:?}"), "Shape(part 3, 1, None)");
    let instances = RenderItem::Instances {
        part: 4,
        prototypes: Arc::from([Arc::from([item])]),
        which: None,
        xforms: vec![Xform::IDENTITY; 2].into(),
        primvars: Arc::default(),
        label: None,
    };
    assert_eq!(instances.part(), 4);
    assert_eq!(format!("{instances:?}"), "Instances(part 4, 1 × 2, None)");
}
