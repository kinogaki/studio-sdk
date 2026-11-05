//! What lies next to a selection: [`grow`], [`shrink`] and a face
//! selection's outline ([`boundary`]).
use kinogaki_geometry::{Domain, Elements, Error, Geometry, Selection};

/// `s` with what lies next to it: a point's neighbours, the edges and
/// primitives sharing a point with an edge or primitive of it, and the
/// vertices on a selected vertex's point and on its neighbours. Costs what
/// it touches. A selection that doesn't belong to `on`, or of a domain
/// without neighbours (Detail), stays as it is.
///
/// ```
/// use kinogaki_edit::select;
/// use kinogaki_geometry::{Attributes, Domain, Elements, Geometry, Primitives, Selection};
///
/// // A strip of three quads.
/// let t = Primitives::of_polygons(8, vec![0, 4, 8, 12].into(), vec![0, 1, 5, 4, 1, 2, 6, 5, 2, 3, 7, 6].into()).unwrap();
/// let strip = Geometry::new(vec![[0.0; 3]; 8].into(), t, Attributes::new()).unwrap();
/// let first = Elements::from_indices(3, [0]).unwrap();
/// let first = Selection::on(&strip, Domain::Primitive, first, None).unwrap();
/// let two = select::grow(&first, &strip);
/// assert_eq!(two.members().iter().collect::<Vec<_>>(), [0, 1]);
/// assert_eq!(select::shrink(&two, &strip).members().iter().collect::<Vec<_>>(), [0]);
/// ```
pub fn grow(s: &Selection, on: &Geometry) -> Selection {
    if s.validate(on).is_err() || !has_neighbours(s.domain()) || s.members().is_all() {
        return s.clone();
    }
    let mut grown: Vec<u32> = s.members().iter().collect();
    for i in s.members().iter() {
        around(on, s.domain(), i, &mut |near| grown.push(near));
    }
    let count = on.count(s.domain()).unwrap_or(0);
    let members = Elements::from_indices(count, grown).expect("the value's own elements");
    Selection::on(on, s.domain(), members, s.active()).expect("the active element stays")
}

/// `s` without what [`grow`] of the rest would take: the elements with
/// everything next to them selected. So a whole surface stays whole.
/// Stays as it is when [`grow`] would.
pub fn shrink(s: &Selection, on: &Geometry) -> Selection {
    if s.validate(on).is_err() || !has_neighbours(s.domain()) || s.members().is_all() {
        return s.clone();
    }
    let kept = s.members().iter().filter(|&i| {
        let mut inside = true;
        around(on, s.domain(), i, &mut |near| {
            inside &= s.contains(near);
        });
        inside
    });
    let count = on.count(s.domain()).unwrap_or(0);
    let members = Elements::from_indices(count, kept).expect("the value's own elements");
    let active = s.active().filter(|&a| members.contains(a));
    Selection::on(on, s.domain(), members, active).expect("the active element stays")
}

fn has_neighbours(domain: Domain) -> bool {
    matches!(
        domain,
        Domain::Point | Domain::Vertex | Domain::Edge | Domain::Primitive
    )
}

/// Calls `near` with what lies next to element `i` of `domain`: the one
/// relation [`grow`] and [`shrink`] share, and a symmetric one, so
/// shrinking is growing the rest.
fn around(on: &Geometry, domain: Domain, i: u32, near: &mut dyn FnMut(u32)) {
    let table = on.primitives();
    let index = table.index();
    match domain {
        Domain::Point => index.point_neighbours(i as usize).for_each(near),
        Domain::Edge => {
            for p in table.edge(i as usize).points {
                index.point_every_edge(p as usize).for_each(&mut *near);
            }
        }
        Domain::Primitive => {
            for &p in table.primitive(i as usize) {
                index.point_primitives(p as usize).for_each(&mut *near);
            }
        }
        _ => {
            let point = table.vertices()[i as usize];
            let mut on_point = |p: u32| {
                for prim in index.point_primitives(p as usize) {
                    for v in table.primitive_vertices(prim as usize) {
                        if table.vertices()[v] == p {
                            near(v as u32);
                        }
                    }
                }
            };
            on_point(point);
            index.point_neighbours(point as usize).for_each(on_point);
        }
    }
}

/// The edges of a Primitive selection's outline: those with exactly one
/// selected polygon among their polygons (of every polygon, the edges the
/// surface ends at, [`Index::boundary_edges`](kinogaki_geometry::Index::boundary_edges)).
/// Costs what it touches.
///
/// ```
/// use kinogaki_edit::select;
/// use kinogaki_geometry::{Attributes, Domain, Elements, Geometry, Primitives, Selection};
///
/// // Two quads side by side, sharing the edge 1–4.
/// let t = Primitives::of_polygons(6, vec![0, 4, 8].into(), vec![0, 1, 4, 3, 1, 2, 5, 4].into()).unwrap();
/// let quads = Geometry::new(vec![[0.0; 3]; 6].into(), t, Attributes::new()).unwrap();
/// let both = Selection::on(&quads, Domain::Primitive, Elements::all(2), None).unwrap();
/// let around = select::boundary(&both, &quads).unwrap();
/// assert_eq!(around.len(), 6);
/// assert!(!around.contains(quads.primitives().edge_of(1, 4).unwrap()));
/// ```
///
/// # Errors
/// As [`Selection::validate`], or a selection of another domain.
pub fn boundary(s: &Selection, on: &Geometry) -> Result<Selection, Error> {
    s.validate(on)?;
    if s.domain() != Domain::Primitive {
        return Err(Error::invalid(format!(
            "a boundary is of primitives, not of the {} domain",
            s.domain().name()
        )));
    }
    let table = on.primitives();
    let index = table.index();
    let count = table.edge_count();
    let outline = |edges: Vec<u32>| Elements::from_indices(count, edges);
    let members = if s.members().is_all() {
        outline(index.boundary_edges().to_vec())?
    } else {
        let mut edges = Vec::new();
        for p in s.members().iter() {
            for e in index.primitive_edges(p as usize) {
                let polygons = index.edge_faces(e as usize);
                if polygons.iter().filter(|&&f| s.contains(f)).count() == 1 {
                    edges.push(e);
                }
            }
        }
        outline(edges)?
    };
    Selection::on(on, Domain::Edge, members, None)
}
