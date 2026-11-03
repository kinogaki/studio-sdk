//! The geometric half of a body's rules: consistency at the points the
//! data names (ends and joins), within P. Nothing along an edge's length,
//! no winding, no intersection: those need sampling densities, which are
//! a kernel's policy.
use super::geometry::{Geometry, Surface};
use super::topology::{BrepTopology, Sense};
use super::{CoedgeId, Entity, SurfaceId};
use crate::error::GeometryError;
use crate::vec3::Vec3;

/// The tolerance of a decision involving entities with these precisions:
/// the body's P, or the largest precision when larger (within P, two
/// points are one; an entity with a precision is tolerant within it).
fn within(tolerance: f64, precisions: &[Option<f64>]) -> f64 {
    precisions
        .iter()
        .flatten()
        .fold(tolerance, |t, &p| t.max(p))
}

fn apart(a: Vec3, b: Vec3) -> f64 {
    (a - b).length()
}

/// Whether `a` and `b` are one point at `tolerance` (never, when either
/// isn't finite).
fn one_point(a: Vec3, b: Vec3, tolerance: f64) -> bool {
    apart(a, b) <= tolerance
}

/// Checks every rule of `topology` over `geometry` at `tolerance` that
/// needs evaluation.
pub(super) fn check(
    topology: &BrepTopology,
    geometry: &Geometry,
    tolerance: f64,
) -> Result<(), GeometryError> {
    let refuse = |entity, phrase, detail: &str| {
        Err(GeometryError::brep(Some(entity), phrase, detail.into()))
    };
    let counts = [
        geometry.points().len(),
        geometry.surfaces().len(),
        geometry.curves().len(),
        geometry.pcurves().len(),
    ];
    let uses_missing = counts[0] != topology.vertices.len()
        || topology
            .faces
            .iter()
            .any(|f| f.surface.index() >= counts[1])
        || topology
            .edges
            .iter()
            .any(|e| e.curve.is_some_and(|c| c.index() >= counts[2]))
        || topology
            .coedges
            .iter()
            .any(|c| c.pcurve.is_some_and(|p| p.index() >= counts[3]));
    if uses_missing {
        return Err(GeometryError::brep(
            None,
            "dangling reference",
            "the geometry has a point for each vertex and every carrier the topology names".into(),
        ));
    }

    // Each edge's carrier ends at its vertices.
    for e in topology.edge_ids() {
        let edge = topology.edge(e);
        let Some(curve) = edge.curve else { continue };
        let curve = geometry.curve(curve);
        for (k, t) in edge.interval.into_iter().enumerate() {
            let v = topology.vertex(edge.vertices[k]);
            let tol = within(tolerance, &[edge.precision, v.precision]);
            if !one_point(curve.point(t), geometry.point(edge.vertices[k]), tol) {
                return refuse(
                    Entity::Edge(e),
                    "vertex off its edge",
                    "an edge's carrier at the end of its interval is not at that end's vertex",
                );
            }
        }
    }

    // Each coedge's p-curve maps onto its edge at the edge's ends: onto the
    // carrier there, or onto the vertex of a pole edge.
    for c in topology.coedge_ids() {
        let coedge = topology.coedge(c);
        let Some(pcurve) = coedge.pcurve else {
            continue;
        };
        let (pcurve, edge) = (geometry.pcurve(pcurve), topology.edge(coedge.edge));
        let face = topology.face(topology.coedge_face(c));
        let surface = geometry.surface(face.surface);
        for (k, t) in edge.interval.into_iter().enumerate() {
            let vertex = topology.vertex(edge.vertices[k]);
            let target = match edge.curve {
                Some(curve) => geometry.curve(curve).point(t),
                None => geometry.point(edge.vertices[k]),
            };
            let tol = within(
                tolerance,
                &[
                    coedge.precision,
                    edge.precision,
                    face.precision,
                    vertex.precision,
                ],
            );
            let Some(on_surface) = surface.point(pcurve.point(t)) else {
                return refuse(
                    Entity::Coedge(c),
                    "surface undefined",
                    "a face's surface cannot be evaluated where a p-curve ends",
                );
            };
            if !one_point(on_surface, target, tol) {
                return refuse(
                    Entity::Coedge(c),
                    "p-curve off its edge",
                    "a coedge's p-curve at its edge's end does not map onto the edge there",
                );
            }
        }
    }

    // Consecutive p-curves of a loop meet, in one lift but at the step
    // closing the loop.
    for l in 0..topology.loops.len() {
        let coedges: Vec<CoedgeId> = topology.loop_coedges(super::LoopId::new(l)).collect();
        let face = topology.face(topology.loop_(super::LoopId::new(l)).face);
        let surface = geometry.surface(face.surface);
        for (i, &c) in coedges.iter().enumerate() {
            let next = coedges[(i + 1) % coedges.len()];
            let (Some(a), Some(b)) = (run(topology, geometry, c), run(topology, geometry, next))
            else {
                continue;
            };
            let (end, start) = (a.1, b.0);
            if end == start {
                continue;
            }
            let vertex = topology.vertex(topology.coedge_end(c));
            let (ca, cb) = (topology.coedge(c), topology.coedge(next));
            let tol = within(
                tolerance,
                &[
                    ca.precision,
                    cb.precision,
                    topology.edge(ca.edge).precision,
                    topology.edge(cb.edge).precision,
                    vertex.precision,
                    face.precision,
                ],
            );
            let closing = i + 1 == coedges.len();
            let lifted = surface
                .periods()
                .into_iter()
                .zip([start[0] - end[0], start[1] - end[1]])
                .any(|(period, step)| {
                    period.is_some_and(|p| {
                        let rest = if closing { reduce(step, p) } else { step };
                        rest.abs() > p / 2.0
                    })
                });
            let meets = match (surface.point(end), surface.point(start)) {
                (Some(p), Some(q)) => one_point(p, q, tol),
                _ => false,
            };
            if lifted || !meets {
                return refuse(
                    Entity::Coedge(c),
                    "loop gap",
                    "consecutive p-curves of a loop do not meet in the face's chart",
                );
            }
        }
    }

    // Closed NURBS rows coincide.
    for (s, surface) in geometry.surfaces().iter().enumerate() {
        let id = SurfaceId::new(s);
        let tol = within(
            tolerance,
            &topology
                .faces
                .iter()
                .filter(|f| f.surface == id)
                .map(|f| f.precision)
                .collect::<Vec<_>>(),
        );
        if !closed_rows_meet(surface, tol) {
            return refuse(
                Entity::Surface(id),
                "closed rows apart",
                "a NURBS surface declared closed has first and last control rows that do not coincide",
            );
        }
    }
    Ok(())
}

/// Where coedge `c` starts and ends in its face's chart, in loop
/// direction; `None` without a p-curve.
fn run(topology: &BrepTopology, geometry: &Geometry, c: CoedgeId) -> Option<([f64; 2], [f64; 2])> {
    let coedge = topology.coedge(c);
    let pcurve = geometry.pcurve(coedge.pcurve?);
    let [t0, t1] = topology.edge(coedge.edge).interval;
    let (a, b) = (pcurve.point(t0), pcurve.point(t1));
    Some(match coedge.sense {
        Sense::Same => (a, b),
        Sense::Reversed => (b, a),
    })
}

/// `step` less the whole periods nearest it.
fn reduce(step: f64, period: f64) -> f64 {
    step - (step / period).round() * period
}

/// Whether each closed direction's first and last control rows coincide
/// within `tolerance` (an offset's base's included).
fn closed_rows_meet(surface: &Surface, tolerance: f64) -> bool {
    match surface {
        Surface::Nurbs(n) => {
            let [nu, nv] = n.counts();
            let rows_meet = |k: usize| {
                n.closed()[k].is_none()
                    || match k {
                        0 => (0..nv).all(|j| {
                            apart(n.point_at(0, j).into(), n.point_at(nu - 1, j).into())
                                <= tolerance
                        }),
                        _ => (0..nu).all(|i| {
                            apart(n.point_at(i, 0).into(), n.point_at(i, nv - 1).into())
                                <= tolerance
                        }),
                    }
            };
            rows_meet(0) && rows_meet(1)
        }
        Surface::Offset(o) => closed_rows_meet(o.base(), tolerance),
        _ => true,
    }
}
