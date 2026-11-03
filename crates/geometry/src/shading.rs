//! [`ShadingNormals`]: the one rule polygons are shaded by, everywhere.
use crate::attributes::{Attribute, Domain};
use crate::column::Column;
use crate::elements::Elements;
use crate::geometry::Geometry;
use crate::kept::Kept;
use crate::names;
use crate::parallel;
use crate::primitives::Primitives;
use crate::vector::{self, V3};
use std::sync::Arc;

/// Points (and polygons) per parallel block.
const BLOCK: usize = 8192;

/// How close two polygons' normals must be for a vertex to smooth across
/// them: within 60° (the cusp angle).
const CUSP_COSINE: f64 = 0.5;

/// A polygons value's shading normals, unit length (f32).
///
/// **The rule.** An authored [`N`](names::N) (read from the first domain
/// that has one: Vertex, Point, Primitive)
/// gives each polygon vertex its normal. Otherwise each vertex averages
/// the normals of the polygons around its point, weighted by each
/// polygon's angle there: the polygons reached from its own across edges
/// with exactly two polygons, never across a [`sharp`](names::SHARP)
/// edge, always across a [`smooth`](names::SMOOTH) one (not also sharp),
/// and across any other edge when the polygon reached is within 60° (the
/// cusp angle) of the vertex's own. One rule, with or without the groups:
/// an empty group changes nothing. A vertex with nothing to average (or an
/// authored normal of zero length) takes its polygon's normal.
///
/// It is derived from the topology, the positions, `N`, `sharp` and
/// `smooth`, and kept on the positions column.
#[derive(Clone, Debug, PartialEq)]
pub struct ShadingNormals {
    vertices: Column<[f32; 3]>,
    primitives: Column<[f32; 3]>,
    /// The polygon normals at full precision, which a refresh reads for
    /// the polygons it doesn't recompute (so it matches a whole build).
    faces: Column<V3>,
}

impl ShadingNormals {
    /// Each vertex's normal (zero on the vertices of primitives that
    /// aren't polygons).
    pub fn vertices(&self) -> &Column<[f32; 3]> {
        &self.vertices
    }

    /// Each primitive's own (flat) normal, from its area (zero for one
    /// that isn't a polygon).
    pub fn primitives(&self) -> &Column<[f32; 3]> {
        &self.primitives
    }
}

/// What normals are derived from besides the positions: each held, so it
/// compares by identity while kept.
struct Inputs {
    topology: Arc<Primitives>,
    authored: Option<(Domain, Arc<Attribute>)>,
    sharp: Option<Arc<Elements>>,
    smooth: Option<Arc<Elements>>,
}

impl Inputs {
    fn of(value: &Geometry) -> Self {
        let attributes = value.attributes();
        let authored = [Domain::Vertex, Domain::Point, Domain::Primitive]
            .into_iter()
            .find_map(|d| Some((d, attributes.get_shared(d, names::N)?.clone())));
        let group = |name| attributes.group_shared(Domain::Edge, name).cloned();
        Self {
            topology: value.primitives().clone(),
            authored,
            sharp: group(names::SHARP),
            smooth: group(names::SMOOTH),
        }
    }

    fn same(&self, other: &Inputs) -> bool {
        Arc::ptr_eq(&self.topology, &other.topology)
            && match (&self.authored, &other.authored) {
                (Some((a, x)), Some((b, y))) => a == b && Arc::ptr_eq(x, y),
                (None, None) => true,
                _ => false,
            }
            && same_group(&self.sharp, &other.sharp)
            && same_group(&self.smooth, &other.smooth)
    }
}

fn same_group(a: &Option<Arc<Elements>>, b: &Option<Arc<Elements>>) -> bool {
    match (a, b) {
        (Some(x), Some(y)) => Arc::ptr_eq(x, y),
        (None, None) => true,
        _ => false,
    }
}

type Normals = Kept<Inputs, ShadingNormals>;

impl Geometry {
    /// The [`ShadingNormals`]: built once per positions, topology, `N`,
    /// `sharp` and `smooth`.
    pub fn shading_normals(&self) -> Arc<ShadingNormals> {
        let inputs = Inputs::of(self);
        let kept = self.positions().derived(|_| Normals::default());
        if let Some(n) = kept.find(|k| k.same(&inputs)) {
            return n;
        }
        let built = Arc::new(build(self, &inputs));
        kept.keep(inputs, built.clone());
        built
    }

    /// The same as [`shading_normals`](Self::shading_normals), made from
    /// `before`'s (same topology, `N`, `sharp` and `smooth`, already built) by
    /// recomputing only the polygons through `moved` and the vertices
    /// around their points; `moved` must include every point whose
    /// position differs. Otherwise built whole.
    pub fn shading_normals_after(&self, before: &Geometry, moved: &[u32]) -> Arc<ShadingNormals> {
        let inputs = Inputs::of(self);
        let kept = self.positions().derived(|_| Normals::default());
        if let Some(n) = kept.find(|k| k.same(&inputs)) {
            return n;
        }
        let previous = before
            .positions()
            .derived(|_| Normals::default())
            .find(|k| k.same(&inputs));
        let built = Arc::new(match previous {
            Some(previous) => refreshed(self, &inputs, &previous, moved),
            None => build(self, &inputs),
        });
        kept.keep(inputs, built.clone());
        built
    }
}

/// Polygon `f`'s normal from its area, unit length (zero when it has
/// none).
fn face_normal(topology: &Primitives, positions: &Column<V3>, f: usize) -> V3 {
    let ids = topology.primitive(f);
    let origin = positions[ids[0] as usize];
    let area = ids
        .iter()
        .zip(ids.iter().cycle().skip(1))
        .take(ids.len())
        .map(|(&a, &b)| {
            vector::cross(
                vector::sub(positions[a as usize], origin),
                vector::sub(positions[b as usize], origin),
            )
        })
        .fold([0.0; 3], vector::add);
    vector::normalize_or_zero(area)
}

fn build(value: &Geometry, inputs: &Inputs) -> ShadingNormals {
    let topology = value.primitives();
    let positions = value.positions();
    let faces: Vec<V3> = parallel::map(topology.len(), BLOCK, |r| {
        r.map(|f| match topology.kind(f).is_polygon() {
            true => face_normal(topology, positions, f),
            false => [0.0; 3],
        })
        .collect::<Vec<_>>()
    })
    .concat();
    let fans = parallel::map(topology.points(), BLOCK, |r| {
        let mut fan = Fan::default();
        let mut out = Vec::new();
        for p in r {
            out.extend_from_slice(fan.at(value, inputs, p, &faces));
        }
        out
    });
    let mut vertices = vec![[0.0f32; 3]; topology.vertex_count()];
    for &(v, n) in fans.iter().flatten() {
        vertices[v as usize] = n;
    }
    finished(vertices, faces)
}

fn finished(vertices: Vec<[f32; 3]>, faces: Vec<V3>) -> ShadingNormals {
    ShadingNormals {
        vertices: vertices.into(),
        primitives: faces.iter().copied().map(vector::to_f32).collect(),
        faces: faces.into(),
    }
}

/// `previous` with the polygons through `moved` and the vertices around
/// their points recomputed.
fn refreshed(
    value: &Geometry,
    inputs: &Inputs,
    previous: &ShadingNormals,
    moved: &[u32],
) -> ShadingNormals {
    let topology = value.primitives();
    let positions = value.positions();
    let point_faces = topology.index().point_faces();
    let mut changed: Vec<u32> = moved
        .iter()
        .filter(|&&p| (p as usize) < topology.points())
        .flat_map(|&p| point_faces[p as usize].iter().copied())
        .collect();
    changed.sort_unstable();
    changed.dedup();
    let mut faces = previous.faces.to_vec();
    for &f in &changed {
        faces[f as usize] = face_normal(topology, positions, f as usize);
    }
    let mut points: Vec<u32> = changed
        .iter()
        .flat_map(|&f| topology.primitive(f as usize).iter().copied())
        .collect();
    points.sort_unstable();
    points.dedup();
    let mut vertices = previous.vertices.to_vec();
    let mut fan = Fan::default();
    for p in points {
        for &(v, n) in fan.at(value, inputs, p as usize, &faces) {
            vertices[v as usize] = n;
        }
    }
    finished(vertices, faces)
}

/// The vertex normals around one point, with buffers reused point to
/// point.
#[derive(Default)]
struct Fan {
    /// Per polygon through the point: the polygon, the point's place in
    /// it, its normal and its angle there.
    local: Vec<(u32, usize, V3, f64)>,
    /// Per polygon through the point: the polygons a flood may reach from
    /// it across its two edges at the point (their places in `local`),
    /// each with whether the edge is smooth; none across a sharp edge or
    /// an edge without exactly two polygons.
    links: Vec<[Option<(usize, bool)>; 2]>,
    visited: Vec<bool>,
    stack: Vec<usize>,
    out: Vec<(u32, [f32; 3])>,
}

impl Fan {
    fn at(
        &mut self,
        value: &Geometry,
        inputs: &Inputs,
        point: usize,
        faces: &[V3],
    ) -> &[(u32, [f32; 3])] {
        let Self {
            local,
            links,
            visited,
            stack,
            out,
        } = self;
        local.clear();
        out.clear();
        let topology = value.primitives();
        let positions = value.positions();
        let index = topology.index();
        let incident = &index.point_faces()[point];
        let here = positions[point];
        for &f in incident {
            let ids = topology.primitive(f as usize);
            let at = ids
                .iter()
                .position(|&p| p as usize == point)
                .expect("a polygon through a point holds it");
            let prev = vector::sub(
                positions[ids[(at + ids.len() - 1) % ids.len()] as usize],
                here,
            );
            let next = vector::sub(positions[ids[(at + 1) % ids.len()] as usize], here);
            let lengths = vector::length(prev) * vector::length(next);
            let angle = if lengths > 1e-30 {
                (vector::dot(prev, next) / lengths).clamp(-1.0, 1.0).acos()
            } else {
                0.0
            };
            local.push((f, at, faces[f as usize], angle));
        }
        let set = |group: &Option<Arc<Elements>>, e: usize| {
            group.as_ref().is_some_and(|g| g.contains(e as u32))
        };
        links.clear();
        let flooded = inputs.authored.is_none().then_some(local.iter());
        links.extend(flooded.into_iter().flatten().map(|&(f, at, _, _)| {
            let edges = index.primitive_edges(f as usize);
            [edges[at], edges[(at + edges.len() - 1) % edges.len()]].map(|edge| {
                let faces = &index.edge_faces()[edge as usize];
                if set(&inputs.sharp, edge as usize) || faces.len() != 2 {
                    return None;
                }
                let other = if faces[0] == f { faces[1] } else { faces[0] };
                let next = incident.iter().position(|&g| g == other)?;
                Some((next, set(&inputs.smooth, edge as usize)))
            })
        }));
        visited.clear();
        visited.resize(incident.len(), false);
        for (i, &(face, at, own, _)) in local.iter().enumerate() {
            let vertex = topology.offsets()[face as usize] + at as u32;
            if let Some((domain, authored)) = &inputs.authored {
                let at = match domain {
                    Domain::Vertex => vertex as usize,
                    Domain::Point => point,
                    _ => face as usize,
                };
                // A standard N is f32 × 3 (the names table).
                let values: &[[f32; 3]] = authored.as_tuples().unwrap_or_default();
                let n = vector::normalize_or_zero(values[at].map(f64::from));
                out.push((
                    vertex,
                    vector::to_f32(if vector::dot(n, n) > 0.0 { n } else { own }),
                ));
                continue;
            }
            visited.fill(false);
            stack.clear();
            stack.push(i);
            visited[i] = true;
            let mut sum = [0.0; 3];
            while let Some(k) = stack.pop() {
                let (_, _, normal, angle) = local[k];
                sum = vector::add(sum, vector::scale(normal, angle));
                for &(next, soft) in links[k].iter().flatten() {
                    if !visited[next] && (soft || vector::dot(own, local[next].2) >= CUSP_COSINE) {
                        visited[next] = true;
                        stack.push(next);
                    }
                }
            }
            let length = vector::length(sum);
            let n = if length > 1e-20 {
                sum.map(|c| c / length)
            } else {
                own
            };
            out.push((vertex, vector::to_f32(n)));
        }
        out
    }
}
