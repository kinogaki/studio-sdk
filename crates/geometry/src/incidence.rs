//! [`Incidence`]: which elements of a value touch which, the one structure
//! selection rules read (`kinogaki-edit`'s conversions), implemented by
//! every standard value with elements: [`Geometry`] and [`Brep`].
use crate::attributes::{Domain, Values, names};
use crate::brep::Brep;
use crate::column::Column;
use crate::geometry::Geometry;

/// A value's elements per domain and how they touch: a polygon its
/// points, vertices and edges; an edge its points; a B-rep body its faces,
/// edges and vertices; and the elements' stable ids where the value keeps
/// them. Structure and identity: no positions, no other attributes.
pub trait Incidence {
    /// How many elements `domain` has; `None` for a domain the value has
    /// none of.
    fn count(&self, domain: Domain) -> Option<usize>;

    /// Calls `pair(a, b)` for each element `a` of `from` that directly
    /// touches an element `b` of `to` (a pair may come more than once).
    /// Nothing for two domains that don't touch, or one it doesn't have;
    /// every element touches the one Detail.
    fn incident(&self, from: Domain, to: Domain, pair: &mut dyn FnMut(u32, u32));

    /// The fingerprint a selection made on it keeps: equal fingerprints
    /// number the same elements alike ([`Selection::validate`](crate::Selection::validate)).
    fn fingerprint(&self) -> [u8; 32];

    /// The stable ids of `domain`'s elements (the standard `id`
    /// attribute, unique within the domain), when the value has them.
    fn ids(&self, domain: Domain) -> Option<&Column<i64>>;
}

impl Incidence for Geometry {
    fn count(&self, domain: Domain) -> Option<usize> {
        Geometry::count(self, domain)
    }

    /// Points and edges by the edges' ends; every other pair through the
    /// primitives' corners (a corner's point, its primitive, its vertex,
    /// the edge it starts; an open primitive's last corner starts none).
    fn incident(&self, from: Domain, to: Domain, pair: &mut dyn FnMut(u32, u32)) {
        let topology = self.primitives();
        let has = |d| Geometry::count(self, d).is_some();
        if from == to || !has(from) || !has(to) {
            return;
        }
        if matches!(
            (from, to),
            (Domain::Point, Domain::Edge) | (Domain::Edge, Domain::Point)
        ) {
            for (edge, e) in topology.edges().enumerate() {
                for point in e.points {
                    match from {
                        Domain::Point => pair(point, edge as u32),
                        _ => pair(edge as u32, point),
                    }
                }
            }
            return;
        }
        let index = (from == Domain::Edge || to == Domain::Edge).then(|| topology.index());
        for p in 0..topology.len() {
            let start = topology.offsets()[p] as usize;
            let edges: Vec<u32> = index.map_or(Vec::new(), |i| i.primitive_edges(p).collect());
            for (corner, &point) in topology.primitive(p).iter().enumerate() {
                let element = |d: Domain| match d {
                    Domain::Point => Some(point),
                    Domain::Primitive => Some(p as u32),
                    Domain::Vertex => Some((start + corner) as u32),
                    Domain::Detail => Some(0),
                    Domain::Edge => edges.get(corner).copied(),
                    _ => None,
                };
                if let (Some(a), Some(b)) = (element(from), element(to)) {
                    pair(a, b);
                }
            }
        }
    }

    fn fingerprint(&self) -> [u8; 32] {
        self.primitives().fingerprint()
    }

    fn ids(&self, domain: Domain) -> Option<&Column<i64>> {
        match self.attributes().get(domain, names::ID)?.values() {
            Values::I64(ids) => Some(ids),
            _ => None,
        }
    }
}

impl Incidence for Brep {
    fn count(&self, domain: Domain) -> Option<usize> {
        Brep::count(self, domain)
    }

    /// Within each body: the body (Object) touches its faces, edges and
    /// vertices; a face the edges its coedges run along and their
    /// vertices; an edge its two vertices. Datums and free curves touch
    /// only the Detail.
    fn incident(&self, from: Domain, to: Domain, pair: &mut dyn FnMut(u32, u32)) {
        let has = |d| Brep::count(self, d).is_some();
        if from == to || !has(from) || !has(to) {
            return;
        }
        if from == Domain::Detail || to == Domain::Detail {
            let other = if from == Domain::Detail { to } else { from };
            for i in 0..Brep::count(self, other).unwrap_or(0) as u32 {
                match from {
                    Domain::Detail => pair(0, i),
                    _ => pair(i, 0),
                }
            }
            return;
        }
        // Down from the higher domain, each pair turned to `from`, `to`.
        let rank = |d| match d {
            Domain::Object => Some(3),
            Domain::Primitive => Some(2),
            Domain::Edge => Some(1),
            Domain::Point => Some(0),
            _ => None,
        };
        let (Some(rank_from), Some(rank_to)) = (rank(from), rank(to)) else {
            return;
        };
        let down = rank_from > rank_to;
        let (high, low) = if down { (from, to) } else { (to, from) };
        let mut emit = |h: u32, l: u32| match down {
            true => pair(h, l),
            false => pair(l, h),
        };
        // Each body's first face, edge and vertex in the value's numbering.
        let mut base = [0u32; 3];
        for (index, body) in self.bodies().iter().enumerate() {
            let t = body.topology();
            let c = t.counts();
            let counts = [c.vertices as u32, c.edges as u32, c.faces as u32];
            let low_id = |i: usize| base[rank(low).expect("a body domain")] + i as u32;
            match high {
                Domain::Object => (0..counts[rank(low).expect("a body domain")])
                    .for_each(|i| emit(index as u32, low_id(i as usize))),
                // A face touches the edges its coedges run along, and their
                // vertices; an edge its two vertices.
                _ => {
                    for e in t.edge_ids() {
                        let vertices = t.edge(e).vertices();
                        let mut touch = |h: u32| match low {
                            Domain::Edge => emit(h, low_id(e.index())),
                            _ => vertices.iter().for_each(|v| emit(h, low_id(v.index()))),
                        };
                        match high {
                            Domain::Primitive => {
                                let faces = t.edge_coedges(e).map(|c| t.coedge_face(c));
                                faces.for_each(|f| touch(base[2] + f.index() as u32));
                            }
                            _ => touch(base[1] + e.index() as u32),
                        }
                    }
                }
            }
            base = [0, 1, 2].map(|k| base[k] + counts[k]);
        }
    }

    fn fingerprint(&self) -> [u8; 32] {
        Brep::fingerprint(self)
    }

    /// A B-rep keeps its elements by persistent name, not by id.
    fn ids(&self, _: Domain) -> Option<&Column<i64>> {
        None
    }
}
