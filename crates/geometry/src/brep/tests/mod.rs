//! `kinogaki.brep`: a second kernel on the SDK alone. A small kernel
//! written here builds plain bodies (a box, a cylinder, a cone with its
//! apex a pole, a whole sphere, a trimmed NURBS patch), a reader measures
//! them by the documented conventions alone, and edits make new bodies
//! from old ones. Then each rule the builder checks is broken once.
use crate::brep::*;
use crate::{Attributes, Column, Curves, Frame};
use std::f64::consts::{FRAC_PI_2, PI, TAU};
use std::sync::Arc;
use vectors::{close, cross, dot, unit};

mod reading;
mod refusals;
mod vectors;

const P: f64 = 0.01;

fn name(index: u32) -> Name {
    Name::created(OperationId(1), Role::Local { index })
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn coedge(edge: EdgeId, sense: Sense, pcurve: PCurveId) -> CoedgeSpec {
    CoedgeSpec {
        edge,
        sense,
        pcurve: Some(pcurve),
        precision: None,
    }
}

fn face(surface: SurfaceId, sense: Sense) -> FaceSpec {
    FaceSpec {
        surface,
        sense,
        precision: None,
    }
}

fn edge(curve: CurveId, interval: [f64; 2], vertices: [VertexId; 2]) -> EdgeSpec {
    EdgeSpec {
        curve: Some(curve),
        interval,
        vertices,
        precision: None,
    }
}

// ---------------------------------------------------------------------------
// The second kernel.

/// A polyhedron of `points` and `faces` (each a cycle of point indices,
/// counter-clockwise seen from outside): planes, lines and chart lines,
/// its entities named from `first`.
fn polyhedron(points: &[[f64; 3]], faces: &[&[usize]], first: u32) -> BrepBuilder {
    let mut b = BrepBuilder::new(name(1000), P);
    let mut names = first..;
    let vertices: Vec<VertexId> = points
        .iter()
        .map(|&p| b.vertex(p, None, name(names.next().unwrap())))
        .collect();
    let mut edges: Vec<((usize, usize), EdgeId, CurveId)> = Vec::new();
    let mut ids = Vec::new();
    for corners in faces {
        let (a, bb, c) = (points[corners[0]], points[corners[1]], points[corners[2]]);
        let z = unit(cross(sub(bb, a), sub(c, a)));
        let x = unit(sub(bb, a));
        let frame = Frame::new(a, x, cross(z, x), z).unwrap();
        let plane = b.surface(Surface::Plane(Plane::new(frame)));
        let chart = |p: [f64; 3]| [dot(sub(p, a), x), dot(sub(p, a), cross(z, x))];
        let mut loop_ = Vec::new();
        for k in 0..corners.len() {
            let (i, j) = (corners[k], corners[(k + 1) % corners.len()]);
            let key = (i.min(j), i.max(j));
            let found = edges
                .iter()
                .find(|(e, _, _)| *e == key)
                .map(|&(_, e, c)| (e, c));
            let (e, _) = found.unwrap_or_else(|| {
                let (from, to) = (points[key.0], points[key.1]);
                let curve = b.curve(Curve::Line(Line::new(from, unit(sub(to, from))).unwrap()));
                let length = dot(sub(to, from), sub(to, from)).sqrt();
                let e = b.edge(
                    edge(curve, [0.0, length], [vertices[key.0], vertices[key.1]]),
                    name(names.next().unwrap()),
                );
                edges.push((key, e, curve));
                (e, curve)
            });
            let (from, to) = (points[key.0], points[key.1]);
            let (o, end) = (chart(from), chart(to));
            let length = dot(sub(to, from), sub(to, from)).sqrt();
            let line = Line2::new(o, [(end[0] - o[0]) / length, (end[1] - o[1]) / length]);
            let p = b.pcurve(PCurve::Line(line.unwrap()));
            let sense = if i == key.0 {
                Sense::Same
            } else {
                Sense::Reversed
            };
            loop_.push(coedge(e, sense, p));
        }
        ids.push(b.face(
            face(plane, Sense::Same),
            vec![loop_],
            name(names.next().unwrap()),
        ));
    }
    let shell = b.shell(ids);
    b.lump(vec![shell]);
    b
}

fn cuboid(lo: [f64; 3], hi: [f64; 3]) -> BrepBuilder {
    cuboid_faces(lo, hi, &BOX_FACES, 0)
}

/// A box's six faces over its corners (bit 0 x, bit 1 y, bit 2 z).
const BOX_FACES: [&[usize]; 6] = [
    &[0, 2, 3, 1],
    &[4, 5, 7, 6],
    &[0, 1, 5, 4],
    &[2, 6, 7, 3],
    &[0, 4, 6, 2],
    &[1, 3, 7, 5],
];

/// A box from `lo` to `hi` bounded by `faces` over its corners, its
/// entities named from `first`.
fn cuboid_faces(lo: [f64; 3], hi: [f64; 3], faces: &[&[usize]], first: u32) -> BrepBuilder {
    let p = |i: usize| {
        [
            if i & 1 == 0 { lo[0] } else { hi[0] },
            if i & 2 == 0 { lo[1] } else { hi[1] },
            if i & 4 == 0 { lo[2] } else { hi[2] },
        ]
    };
    let points: Vec<[f64; 3]> = (0..8).map(p).collect();
    polyhedron(&points, faces, first)
}

/// A closed cylinder of `radius` and `height` on the world z axis: a side
/// with no seam (two loops wrapping θ) and two discs.
fn cylinder(radius: f64, height: f64) -> BrepBuilder {
    let mut b = BrepBuilder::new(name(2000), P);
    let raised = Frame::new(
        [0.0, 0.0, height],
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
    );
    let raised = raised.unwrap();
    let side = b.surface(Surface::Cylinder(
        Cylinder::new(Frame::WORLD, radius).unwrap(),
    ));
    let bottom = b.surface(Surface::Plane(Plane::new(Frame::WORLD)));
    let top = b.surface(Surface::Plane(Plane::new(raised)));
    let rims =
        [Frame::WORLD, raised].map(|f| b.curve(Curve::Circle(Circle::new(f, radius).unwrap())));
    let v = [0.0, height].map(|z| b.vertex([radius, 0.0, z], None, name(z as u32)));
    let e = [0, 1].map(|k| b.edge(edge(rims[k], [0.0, TAU], [v[k], v[k]]), name(10 + k as u32)));
    let along =
        [0.0, height].map(|h| b.pcurve(PCurve::Line(Line2::new([0.0, h], [1.0, 0.0]).unwrap())));
    let disc = PCurve::Circle(Circle2::new([0.0; 2], [1.0, 0.0], radius, true).unwrap());
    let discs = [b.pcurve(disc.clone()), b.pcurve(disc)];
    let s = b.face(
        face(side, Sense::Same),
        vec![
            vec![coedge(e[0], Sense::Same, along[0])],
            vec![coedge(e[1], Sense::Reversed, along[1])],
        ],
        name(20),
    );
    let lo = b.face(
        face(bottom, Sense::Reversed),
        vec![vec![coedge(e[0], Sense::Reversed, discs[0])]],
        name(21),
    );
    let hi = b.face(
        face(top, Sense::Same),
        vec![vec![coedge(e[1], Sense::Same, discs[1])]],
        name(22),
    );
    let shell = b.shell(vec![s, lo, hi]);
    b.lump(vec![shell]);
    b
}

/// A cone with its apex at the origin (a pole edge), opening up by
/// `half_angle` to a disc at `height`.
fn cone(half_angle: f64, height: f64) -> BrepBuilder {
    let mut b = BrepBuilder::new(name(3000), P);
    let radius = height * half_angle.tan();
    let raised = Frame::new(
        [0.0, 0.0, height],
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
    );
    let raised = raised.unwrap();
    let side = b.surface(Surface::Cone(
        Cone::new(Frame::WORLD, 0.0, half_angle).unwrap(),
    ));
    let top = b.surface(Surface::Plane(Plane::new(raised)));
    let rim = b.curve(Curve::Circle(Circle::new(raised, radius).unwrap()));
    let apex = b.vertex([0.0; 3], None, name(0));
    let v = b.vertex([radius, 0.0, height], None, name(1));
    let pole = b.edge(
        EdgeSpec {
            curve: None,
            interval: [0.0, TAU],
            vertices: [apex, apex],
            precision: None,
        },
        name(2),
    );
    let circle = b.edge(edge(rim, [0.0, TAU], [v, v]), name(3));
    let line = |h: f64| PCurve::Line(Line2::new([0.0, h], [1.0, 0.0]).unwrap());
    let (at_apex, at_rim) = (b.pcurve(line(0.0)), b.pcurve(line(height)));
    let disc = b.pcurve(PCurve::Circle(
        Circle2::new([0.0; 2], [1.0, 0.0], radius, true).unwrap(),
    ));
    let s = b.face(
        face(side, Sense::Same),
        vec![
            vec![coedge(pole, Sense::Same, at_apex)],
            vec![coedge(circle, Sense::Reversed, at_rim)],
        ],
        name(4),
    );
    let t = b.face(
        face(top, Sense::Same),
        vec![vec![coedge(circle, Sense::Same, disc)]],
        name(5),
    );
    let shell = b.shell(vec![s, t]);
    b.lump(vec![shell]);
    b
}

/// A whole sphere: one face without loops, on a closed surface.
fn sphere(radius: f64) -> BrepBuilder {
    let mut b = BrepBuilder::new(name(4000), P);
    let s = b.surface(Surface::Sphere(Sphere::new(Frame::WORLD, radius).unwrap()));
    let f = b.face(face(s, Sense::Same), Vec::new(), name(0));
    let shell = b.shell(vec![f]);
    b.lump(vec![shell]);
    b
}

/// The 3 × 3 control grid of a biquadratic Bézier patch over the unit
/// square, bulging by `lift` in its middle.
fn grid(lift: f64) -> Vec<[f64; 3]> {
    let mut points = Vec::new();
    for i in 0..3 {
        for j in 0..3 {
            let z = if i == 1 && j == 1 { lift } else { 0.0 };
            points.push([i as f64 / 2.0, j as f64 / 2.0, z]);
        }
    }
    points
}

fn bezier_knots() -> Column<f64> {
    vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0].into()
}

/// The isoparametric curve of `grid` at `u = c` (or `v = c`): its control
/// points are the rows blended by the quadratic Bernstein weights at `c`.
fn iso(grid: &[[f64; 3]], along_v: bool, c: f64) -> Curve {
    let bernstein = [(1.0 - c) * (1.0 - c), 2.0 * c * (1.0 - c), c * c];
    let points: Vec<[f64; 3]> = (0..3)
        .map(|k| {
            let mut p = [0.0; 3];
            for (m, w) in bernstein.iter().enumerate() {
                let q = if along_v {
                    grid[m * 3 + k]
                } else {
                    grid[k * 3 + m]
                };
                for d in 0..3 {
                    p[d] += w * q[d];
                }
            }
            p
        })
        .collect();
    Curve::Nurbs(Arc::new(
        NurbsCurve::new(2, bezier_knots(), points, None).unwrap(),
    ))
}

/// A biquadratic patch trimmed to `[a, b]²` of its chart: one face, four
/// edges on its isoparametric curves.
fn patch(lift: f64, [a, b]: [f64; 2]) -> BrepBuilder {
    let g = grid(lift);
    let mut k = BrepBuilder::new(name(5000), P);
    let surface = NurbsSurface::new(
        [2, 2],
        [bezier_knots(), bezier_knots()],
        g.clone(),
        None,
        [None, None],
    );
    let surface = Surface::Nurbs(Arc::new(surface.unwrap()));
    let s = k.surface(surface.clone());
    let corner = |u: f64, v: f64| surface.point([u, v]).unwrap();
    let v = [(a, a), (b, a), (b, b), (a, b)].map(|(u, w)| corner(u, w));
    let vs: Vec<VertexId> = v
        .iter()
        .enumerate()
        .map(|(i, &p)| k.vertex(p, None, name(i as u32)))
        .collect();
    // Edges along u at v = a and v = b, along v at u = a and u = b, each
    // on its iso curve over [a, b], from the lower parameter up.
    let sides = [
        (false, a, [vs[0], vs[1]], [0.0, a], [1.0, 0.0]),
        (true, b, [vs[1], vs[2]], [b, 0.0], [0.0, 1.0]),
        (false, b, [vs[3], vs[2]], [0.0, b], [1.0, 0.0]),
        (true, a, [vs[0], vs[3]], [a, 0.0], [0.0, 1.0]),
    ];
    let mut loop_ = Vec::new();
    for (i, (along_v, c, ends, o, d)) in sides.into_iter().enumerate() {
        let curve = k.curve(iso(&g, along_v, c));
        let e = k.edge(edge(curve, [a, b], ends), name(10 + i as u32));
        let p = k.pcurve(PCurve::Line(Line2::new(o, d).unwrap()));
        let sense = if i < 2 { Sense::Same } else { Sense::Reversed };
        loop_.push(coedge(e, sense, p));
    }
    let f = k.face(face(s, Sense::Same), vec![loop_], name(20));
    let shell = k.shell(vec![f]);
    k.lump(vec![shell]);
    k
}

/// The kernel's own edit: the same body moved by `by`, read entity by
/// entity and built again.
fn translated(body: &Body, by: [f64; 3]) -> Body {
    let t = body.topology();
    let g = body.geometry();
    let names = body.names();
    let moved = |p: [f64; 3]| [p[0] + by[0], p[1] + by[1], p[2] + by[2]];
    let frame = |f: &Frame| Frame::new(moved(f.origin()), f.x(), f.y(), f.z()).unwrap();
    let mut b = BrepBuilder::new(body.name().clone(), body.tolerance());
    for s in g.surfaces().iter() {
        b.surface(match s {
            Surface::Plane(p) => Surface::Plane(Plane::new(frame(p.frame()))),
            Surface::Cylinder(c) => {
                Surface::Cylinder(Cylinder::new(frame(c.frame()), c.radius()).unwrap())
            }
            other => panic!("the kernel doesn't move {other:?}"),
        });
    }
    for c in g.curves().iter() {
        b.curve(match c {
            Curve::Line(l) => Curve::Line(Line::new(moved(l.origin()), l.direction()).unwrap()),
            Curve::Circle(c) => Curve::Circle(Circle::new(frame(c.frame()), c.radius()).unwrap()),
            other => panic!("the kernel doesn't move {other:?}"),
        });
    }
    for p in g.pcurves().iter() {
        b.pcurve(p.clone());
    }
    for v in t.vertex_ids() {
        b.vertex(
            moved(g.point(v)),
            t.vertex(v).precision(),
            names.vertices[v.index()].clone(),
        );
    }
    for e in t.edge_ids() {
        let edge = t.edge(e);
        let spec = EdgeSpec {
            curve: edge.curve(),
            interval: edge.interval(),
            vertices: edge.vertices(),
            precision: edge.precision(),
        };
        b.edge(spec, names.edge(e).clone());
    }
    for f in t.face_ids() {
        let fc = t.face(f);
        let loops = t
            .face_loops(f)
            .map(|l| {
                t.loop_coedges(l)
                    .map(|c| {
                        let c = t.coedge(c);
                        CoedgeSpec {
                            edge: c.edge(),
                            sense: c.sense(),
                            pcurve: c.pcurve(),
                            precision: c.precision(),
                        }
                    })
                    .collect()
            })
            .collect();
        let spec = FaceSpec {
            surface: fc.surface(),
            sense: fc.sense(),
            precision: fc.precision(),
        };
        b.face(spec, loops, names.faces[f.index()].clone());
    }
    for s in t.shell_ids() {
        b.shell(t.shell_faces(s).to_vec());
    }
    for l in t.lump_ids() {
        b.lump(t.lump_shells(l).to_vec());
    }
    b.build((**body.attributes()).clone()).unwrap()
}

// ---------------------------------------------------------------------------
// A reader: what the conventions alone say about a body.

/// The volume of a polyhedral solid, from its faces' outward normals
/// (surface normal by sense) and their boundaries' vertices in loop order.
fn polyhedral_volume(body: &Body) -> f64 {
    let (t, g) = (body.topology(), body.geometry());
    let mut volume = 0.0;
    for f in t.face_ids() {
        let face = t.face(f);
        let Surface::Plane(plane) = g.surface(face.surface()) else {
            panic!("not a polyhedron")
        };
        let sign = if face.sense() == Sense::Same {
            1.0
        } else {
            -1.0
        };
        let n = plane.frame().z().map(|c| c * sign);
        for l in t.face_loops(f) {
            let corners: Vec<[f64; 3]> = t
                .loop_coedges(l)
                .map(|c| g.point(t.coedge_start(c)))
                .collect();
            // Area vector of the loop, by its direction: material on the
            // left seen along the outward normal makes it point along it.
            let mut area = [0.0; 3];
            for k in 0..corners.len() {
                let c = cross(corners[k], corners[(k + 1) % corners.len()]);
                for d in 0..3 {
                    area[d] += c[d] / 2.0;
                }
            }
            assert!(
                dot(area, n) > 0.0,
                "a loop of face {} winds against its sense",
                f.index()
            );
            volume += dot(corners[0], n) * dot(area, n) / 3.0;
        }
    }
    volume
}

/// The Euler–Poincaré characteristic `V − E + F − (L − F)`.
fn euler(body: &Body) -> i64 {
    let c = body.topology().counts();
    c.vertices as i64 - c.edges as i64 + 2 * c.faces as i64 - c.loops as i64
}
