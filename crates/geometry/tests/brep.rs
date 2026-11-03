//! `kinogaki.brep`: a second kernel on the SDK alone. A small kernel
//! written here builds plain bodies (a box, a cylinder, a cone with its
//! apex a pole, a whole sphere, a trimmed NURBS patch), a reader measures
//! them by the documented conventions alone, and edits make new bodies
//! from old ones. Then each rule the builder checks is broken once.
use kinogaki_geometry::brep::*;
use kinogaki_geometry::{Attribute, Attributes, Class, Column, Curves, ErrorKind, Frame, TypeInfo};
use std::f64::consts::{FRAC_PI_2, FRAC_PI_4, PI, TAU};
use std::sync::Arc;
mod vectors;
use vectors::{close, cross, dot, unit};

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
            names.vertex(v).clone(),
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
        b.face(spec, loops, names.face(f).clone());
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

// ---------------------------------------------------------------------------
// The kernel's bodies.

#[test]
fn a_box_reads_back_by_the_conventions() {
    let body = cuboid([0.0; 3], [2.0, 3.0, 4.0])
        .build(Attributes::new())
        .unwrap();
    let t = body.topology();
    assert_eq!(body.kind(), BodyKind::Solid);
    assert_eq!(
        t.counts(),
        Counts {
            vertices: 8,
            edges: 12,
            coedges: 24,
            loops: 6,
            faces: 6,
            shells: 1,
            lumps: 1
        }
    );
    assert_eq!(euler(&body), 2);
    assert!((polyhedral_volume(&body) - 24.0).abs() < 1e-12);
    // Every edge's ring: two coedges, opposite ways, on two faces.
    for e in t.edge_ids() {
        let ring: Vec<CoedgeId> = t.edge_coedges(e).collect();
        assert_eq!(ring.len(), 2);
        assert_ne!(t.coedge(ring[0]).sense(), t.coedge(ring[1]).sense());
        assert_ne!(t.coedge_face(ring[0]), t.coedge_face(ring[1]));
        assert_eq!(t.coedge(ring[0]).radial(), ring[1]);
    }
    // Loops close: each coedge ends where the next starts.
    for c in t.coedge_ids() {
        assert_eq!(t.coedge_end(c), t.coedge_start(t.next(c)));
        assert_eq!(t.previous(t.next(c)), c);
    }
    // Ids are checked against the body.
    assert_eq!(FaceId::from_index(&body, 5).map(FaceId::index), Some(5));
    assert_eq!(FaceId::from_index(&body, 6), None);
    assert_eq!(
        VertexId::from_index(&body, 7).map(|v| body.geometry().point(v)),
        Some([2.0, 3.0, 4.0])
    );
}

#[test]
fn a_cylinder_has_no_seam_and_its_loops_wrap() {
    let body = cylinder(1.5, 4.0).build(Attributes::new()).unwrap();
    let t = body.topology();
    assert_eq!(body.kind(), BodyKind::Solid);
    let c = t.counts();
    assert_eq!((c.vertices, c.edges, c.faces, c.loops), (2, 2, 3, 4));
    assert_eq!(euler(&body), 2);
    // The side's loops run its chart's period: θ from 0 to 2π.
    let side = FaceId::from_index(&body, 0).unwrap();
    let surface = body.geometry().surface(t.face(side).surface());
    assert_eq!(surface.periods(), [Some(TAU), None]);
    for l in t.face_loops(side) {
        let [c] = t.loop_coedges(l).collect::<Vec<_>>()[..] else {
            panic!()
        };
        let p = body.geometry().pcurve(t.coedge(c).pcurve().unwrap());
        let [t0, t1] = t.edge(t.coedge(c).edge()).interval();
        assert_eq!(p.point(t1)[0] - p.point(t0)[0], TAU);
    }
}

#[test]
fn a_cone_closes_at_its_apex_with_a_pole_edge() {
    let body = cone(FRAC_PI_4, 2.0).build(Attributes::new()).unwrap();
    let t = body.topology();
    // The pole is used once and still the shell is closed.
    assert_eq!(body.kind(), BodyKind::Solid);
    let pole = EdgeId::from_index(&body, 0).unwrap();
    assert_eq!(t.edge(pole).curve(), None);
    assert_eq!(t.edge_coedges(pole).count(), 1);
    assert_eq!(body.geometry().point(t.edge(pole).vertices()[0]), [0.0; 3]);
}

#[test]
fn a_whole_sphere_needs_no_loops() {
    let body = sphere(2.0).build(Attributes::new()).unwrap();
    assert_eq!(body.kind(), BodyKind::Solid);
    let c = body.topology().counts();
    assert_eq!((c.faces, c.loops, c.edges, c.vertices), (1, 0, 0, 0));
}

#[test]
fn a_trimmed_patch_is_a_sheet_on_its_surface() {
    let body = patch(0.5, [0.25, 0.75]).build(Attributes::new()).unwrap();
    assert_eq!(body.kind(), BodyKind::Sheet);
    let (t, g) = (body.topology(), body.geometry());
    // Each edge lies on the surface along its whole length here (the
    // builder checks the ends only; this kernel's curves are exact).
    for c in t.coedge_ids() {
        let coedge = t.coedge(c);
        let e = t.edge(coedge.edge());
        let (curve, p) = (
            g.curve(e.curve().unwrap()),
            g.pcurve(coedge.pcurve().unwrap()),
        );
        let surface = g.surface(t.face(t.coedge_face(c)).surface());
        for k in 0..=8 {
            let s = e.interval()[0] + (e.interval()[1] - e.interval()[0]) * k as f64 / 8.0;
            assert!(close(
                surface.point(p.point(s)).unwrap(),
                curve.point(s),
                1e-12
            ));
        }
    }
}

#[test]
fn an_edit_reads_one_body_and_builds_another() {
    for body in [cuboid([0.0; 3], [1.0; 3]), cylinder(1.0, 2.0)]
        .map(|b| b.build(Attributes::new()).unwrap())
    {
        let moved = translated(&body, [10.0, -5.0, 2.0]);
        assert_eq!(moved.topology(), body.topology());
        assert_eq!(moved.names(), body.names());
        assert_ne!(moved.geometry(), body.geometry());
        assert_eq!(translated(&moved, [-10.0, 5.0, -2.0]), body);
    }
}

#[test]
fn a_cv_drag_keeps_the_topology_and_is_checked_at_the_ends() {
    let body = patch(0.5, [0.25, 0.75]).build(Attributes::new()).unwrap();
    let g = body.geometry();
    let Surface::Nurbs(n) = g.surface(SurfaceId::from_index(&body, 0).unwrap()) else {
        panic!()
    };
    // Drag the middle CV up: the trim edges and corners move with it.
    let lifted = grid(1.5);
    let surface = Surface::Nurbs(Arc::new(n.with_points(lifted.clone()).unwrap()));
    let curves: Vec<Curve> = [(false, 0.25), (true, 0.75), (false, 0.75), (true, 0.25)]
        .map(|(along_v, c)| iso(&lifted, along_v, c))
        .into();
    let corners: Vec<[f64; 3]> = [(0.25, 0.25), (0.75, 0.25), (0.75, 0.75), (0.25, 0.75)]
        .map(|(u, v)| surface.point([u, v]).unwrap())
        .into();
    let dragged = Geometry::new(
        corners.clone(),
        vec![surface.clone()],
        curves.clone(),
        g.pcurves().clone(),
    )
    .unwrap();
    let moved = body.with_geometry(dragged).unwrap();
    assert!(Arc::ptr_eq(moved.topology(), body.topology()));
    // The surface alone, without its edges: a p-curve's end no longer maps
    // onto its edge.
    let stale = Geometry::new(
        corners,
        vec![surface],
        g.curves().clone(),
        g.pcurves().clone(),
    )
    .unwrap();
    let e = body.with_geometry(stale).unwrap_err();
    assert_eq!(
        (e.kind(), e.message()),
        (ErrorKind::Brep, "vertex off its edge")
    );
    // Tables of another size.
    let short = Geometry::new(
        Vec::<[f64; 3]>::new(),
        g.surfaces().clone(),
        curves,
        g.pcurves().clone(),
    );
    assert_eq!(
        body.with_geometry(short.unwrap()).unwrap_err().message(),
        "geometry resized"
    );
}

#[test]
fn caches_live_on_the_shared_parts() {
    struct Degrees(Vec<usize>);
    let body = cuboid([0.0; 3], [1.0; 3]).build(Attributes::new()).unwrap();
    let degrees = |b: &Body| {
        b.topology().derived(|t| {
            let mut d = vec![0; t.counts().vertices];
            for e in t.edge_ids() {
                for v in t.edge(e).vertices() {
                    d[v.index()] += 1;
                }
            }
            Degrees(d)
        })
    };
    assert_eq!(degrees(&body).0, vec![3; 8]);
    let moved = body
        .with_geometry(
            Geometry::new(
                body.geometry().points().clone(),
                body.geometry().surfaces().clone(),
                body.geometry().curves().clone(),
                body.geometry().pcurves().clone(),
            )
            .unwrap(),
        )
        .unwrap();
    assert!(Arc::ptr_eq(&degrees(&body), &degrees(&moved)));
    // The fingerprint follows the topology, not the geometry.
    assert_eq!(
        body.topology().fingerprint(),
        translated(&body, [1.0; 3]).topology().fingerprint()
    );
    let other = cylinder(1.0, 1.0).build(Attributes::new()).unwrap();
    assert_ne!(
        body.topology().fingerprint(),
        other.topology().fingerprint()
    );
}

/// The fingerprints say whether ids still mean the same elements: a
/// resize (a box's sizes, a cylinder's radius: other edge
/// parameter ranges over the same structure) keeps them, a split face,
/// another structure or other names change them.
#[test]
fn fingerprints_follow_structure_and_names_never_geometry() {
    let one = |b: BrepBuilder| {
        let body = b.build(Attributes::new()).unwrap();
        let value = Brep::new(
            vec![body.clone()],
            Curves::empty(),
            Vec::new(),
            Attributes::new(),
            History::default(),
        )
        .unwrap();
        (body, value)
    };
    let (unit, unit_value) = one(cuboid([0.0; 3], [1.0; 3]));
    let (resized, resized_value) = one(cuboid([0.0; 3], [2.0, 3.0, 4.0]));
    assert_ne!(
        unit.topology()
            .edge(EdgeId::from_index(&unit, 0).unwrap())
            .interval(),
        resized
            .topology()
            .edge(EdgeId::from_index(&resized, 0).unwrap())
            .interval()
    );
    assert_eq!(
        unit.topology().fingerprint(),
        resized.topology().fingerprint()
    );
    assert_eq!(unit_value.fingerprint(), resized_value.fingerprint());
    let (thin, thin_value) = one(cylinder(1.0, 1.0));
    let (wide, wide_value) = one(cylinder(2.5, 1.0));
    assert_eq!(thin.topology().fingerprint(), wide.topology().fingerprint());
    assert_eq!(thin_value.fingerprint(), wide_value.fingerprint());
    // The top split into two triangles: a face and an edge more.
    let mut split: Vec<&[usize]> = BOX_FACES.to_vec();
    split[1] = &[4, 5, 7];
    split.push(&[4, 7, 6]);
    let (halves, halves_value) = one(cuboid_faces([0.0; 3], [1.0; 3], &split, 0));
    assert_ne!(
        unit.topology().fingerprint(),
        halves.topology().fingerprint()
    );
    assert_ne!(unit_value.fingerprint(), halves_value.fingerprint());
    assert_ne!(unit_value.fingerprint(), thin_value.fingerprint());
    // The same structure under other names.
    let (renamed, renamed_value) = one(cuboid_faces([0.0; 3], [1.0; 3], &BOX_FACES, 100));
    assert_eq!(
        unit.topology().fingerprint(),
        renamed.topology().fingerprint()
    );
    assert_ne!(unit_value.fingerprint(), renamed_value.fingerprint());
}

/// Datums and free curves are numbered too, so the fingerprint names
/// them in order: swapping two point datums or two free curves changes
/// what an id selects and the fingerprint; moving them changes neither.
#[test]
fn fingerprints_name_datums_and_free_curves_in_order() {
    let point = |index: u32, at: [f64; 3]| Datum {
        name: name(index),
        kind: DatumKind::Point(at),
        hint: DatumHint {
            center: at,
            size: 1.0,
        },
    };
    let curves = |names: [&str; 2], lift: f64| {
        let points = vec![
            [0.0, 0.0, lift],
            [1.0, 0.0, lift],
            [0.0, 1.0, lift],
            [1.0, 1.0, lift],
        ];
        let named = Attributes::new()
            .with(
                Class::Primitive,
                kinogaki_geometry::names::NAME,
                Attribute::text(kinogaki_geometry::Text::from_strs(names)),
            )
            .unwrap();
        Curves::builder(
            kinogaki_geometry::Basis::Linear,
            points.into(),
            vec![0, 2, 4].into(),
        )
        .attributes(named)
        .build()
        .unwrap()
    };
    let value = |datums: Vec<Datum>, curves: Curves| {
        Brep::new(
            Vec::new(),
            curves,
            datums,
            Attributes::new(),
            History::default(),
        )
        .unwrap()
        .fingerprint()
    };
    let (a, b) = (point(1, [0.0; 3]), point(2, [1.0, 0.0, 0.0]));
    let first = value(vec![a.clone(), b.clone()], curves(["left", "right"], 0.0));
    assert_ne!(
        first,
        value(vec![b.clone(), a.clone()], curves(["left", "right"], 0.0)),
        "two point datums swapped"
    );
    assert_ne!(
        first,
        value(vec![a.clone(), b.clone()], curves(["right", "left"], 0.0)),
        "two free curves swapped"
    );
    let moved = vec![point(1, [5.0, 5.0, 5.0]), point(2, [-1.0, 2.0, 0.0])];
    assert_eq!(
        first,
        value(moved, curves(["left", "right"], 3.0)),
        "datums and curves moved"
    );
}

#[test]
fn attributes_follow_the_body_classes() {
    let b = cuboid([0.0; 3], [1.0; 3]);
    let faces = Attributes::new()
        .with(
            Class::Primitive,
            "Cd",
            Attribute::vector(vec![[1.0, 0.0, 0.0]; 6], TypeInfo::Color),
        )
        .unwrap()
        .with(Class::Edge, "cad:weight", Attribute::float(vec![1.0; 12]))
        .unwrap()
        .with(Class::Point, "cad:weight", Attribute::float(vec![1.0; 8]))
        .unwrap()
        .with(Class::Object, "cad:output_by", Attribute::integer(vec![7]))
        .unwrap();
    let body = b.clone().build(faces.clone()).unwrap();
    assert_eq!(body.count(Class::Instance), Some(1));
    assert_eq!(body.count(Class::Vertex), None);
    let short = faces.with(
        Class::Primitive,
        "Cd",
        Attribute::vector(vec![[1.0; 3]; 5], TypeInfo::Color),
    );
    assert_eq!(
        b.clone().build(short.unwrap()).unwrap_err().kind(),
        ErrorKind::Attribute
    );
    let corner = Attributes::new().with(Class::Vertex, "w", Attribute::float(vec![0.0; 24]));
    assert_eq!(
        b.clone().build(corner.unwrap()).unwrap_err().kind(),
        ErrorKind::Attribute
    );
    // Instances: one Instance element a placement.
    let names = Attributes::new()
        .with(Class::Instance, "instance", Attribute::integer(vec![0, 1]))
        .unwrap();
    let mut placed = b.clone();
    let up = Placement::new([
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 3.0],
    ])
    .unwrap();
    placed.placement(Placement::IDENTITY, vec![up]);
    let body = placed.build(names.clone()).unwrap();
    assert_eq!(body.count(Class::Instance), Some(2));
    assert!(body.with_placement(up, Column::default()).is_err());
    assert_eq!(
        body.with_placement(up, vec![up].into())
            .unwrap()
            .placement(),
        &up
    );
}

#[test]
fn a_value_holds_bodies_curves_and_datums() {
    let body = cuboid([0.0; 3], [1.0; 3]).build(Attributes::new()).unwrap();
    let datum = Datum {
        name: name(1),
        kind: DatumKind::Axis {
            origin: [0.0; 3],
            direction: [0.0, 0.0, 1.0],
        },
        hint: DatumHint {
            center: [0.0; 3],
            size: 2.0,
        },
    };
    let history = History {
        generated: vec![body.name().clone()],
        ..History::default()
    };
    let value = Brep::new(
        vec![body.clone()],
        Curves::empty(),
        vec![datum.clone()],
        Attributes::new(),
        history,
    )
    .unwrap();
    assert_eq!((value.bodies().len(), value.datums().len()), (1, 1));
    assert_eq!(value.history().generated, vec![body.name().clone()]);
    let skew = Datum {
        kind: DatumKind::Axis {
            origin: [0.0; 3],
            direction: [0.0, 0.0, 2.0],
        },
        ..datum
    };
    let e = Brep::new(
        Vec::new(),
        Curves::empty(),
        vec![skew],
        Attributes::new(),
        History::default(),
    )
    .unwrap_err();
    assert_eq!(e.entity(), Some(Entity::Datum(0)));
    let detail = Attributes::new()
        .with(Class::Primitive, "w", Attribute::float(vec![0.0]))
        .unwrap();
    assert!(value.with_detail(detail).is_err());
    assert_eq!(Brep::empty(), Brep::default());
}

// ---------------------------------------------------------------------------
// Each rule broken once.

/// The refusal's short phrase, and the entity it blames as a kind and an
/// index. Its text names no id and no number.
fn refused(
    result: Result<Body, kinogaki_geometry::GeometryError>,
) -> (String, Option<(&'static str, usize)>) {
    let e = result.unwrap_err();
    assert_eq!(e.kind(), ErrorKind::Brep);
    for text in [format!("{e}"), format!("{e:#}")] {
        assert!(!text.chars().any(|c| c.is_ascii_digit()), "{text}");
    }
    assert_ne!(format!("{e}"), format!("{e:#}"));
    let entity = e.entity().map(|entity| {
        let kind = match entity {
            Entity::Vertex(_) => "vertex",
            Entity::Edge(_) => "edge",
            Entity::Coedge(_) => "coedge",
            Entity::Loop(_) => "loop",
            Entity::Face(_) => "face",
            Entity::Surface(_) => "surface",
            _ => "other",
        };
        (kind, entity.index())
    });
    assert_eq!(e.element(), entity.map(|(_, i)| i));
    (e.message().to_string(), entity)
}

#[test]
fn structure_is_checked() {
    let disc = || {
        let mut b = BrepBuilder::new(name(0), P);
        let plane = b.surface(Surface::Plane(Plane::new(Frame::WORLD)));
        let circle = b.curve(Curve::Circle(Circle::new(Frame::WORLD, 1.0).unwrap()));
        let p = b.pcurve(PCurve::Circle(
            Circle2::new([0.0; 2], [1.0, 0.0], 1.0, true).unwrap(),
        ));
        let v = b.vertex([1.0, 0.0, 0.0], None, name(1));
        (b, plane, circle, p, v)
    };
    // An open loop.
    let (mut b, plane, circle, p, v) = disc();
    let w = b.vertex([0.0, 1.0, 0.0], None, name(2));
    let e = b.edge(edge(circle, [0.0, FRAC_PI_2], [v, w]), name(3));
    let f = b.face(
        face(plane, Sense::Same),
        vec![vec![coedge(e, Sense::Same, p)]],
        name(4),
    );
    let s = b.shell(vec![f]);
    b.lump(vec![s]);
    assert_eq!(
        refused(b.build(Attributes::new())),
        ("open loop".into(), Some(("loop", 0)))
    );
    // An interval running backwards.
    let (mut b, plane, circle, p, v) = disc();
    let e = b.edge(edge(circle, [TAU, 0.0], [v, v]), name(3));
    let f = b.face(
        face(plane, Sense::Same),
        vec![vec![coedge(e, Sense::Same, p)]],
        name(4),
    );
    let s = b.shell(vec![f]);
    b.lump(vec![s]);
    assert_eq!(refused(b.build(Attributes::new())).0, "empty edge interval");
    // A pole edge between two vertices.
    let (mut b, plane, _, p, v) = disc();
    let w = b.vertex([0.0, 1.0, 0.0], None, name(2));
    let e = b.edge(
        EdgeSpec {
            curve: None,
            interval: [0.0, 1.0],
            vertices: [v, w],
            precision: None,
        },
        name(3),
    );
    let f = b.face(
        face(plane, Sense::Same),
        vec![vec![coedge(e, Sense::Same, p)]],
        name(4),
    );
    let s = b.shell(vec![f]);
    b.lump(vec![s]);
    assert_eq!(refused(b.build(Attributes::new())).0, "bad pole edge");
    // A face in no shell; a shell in no lump; an edge no coedge uses.
    let (mut b, plane, circle, p, v) = disc();
    let e = b.edge(edge(circle, [0.0, TAU], [v, v]), name(3));
    let f = b.face(
        face(plane, Sense::Same),
        vec![vec![coedge(e, Sense::Same, p)]],
        name(4),
    );
    assert_eq!(
        refused(b.clone().build(Attributes::new())),
        ("face in no shell".into(), Some(("face", 0)))
    );
    let s = b.shell(vec![f]);
    assert_eq!(
        refused(b.clone().build(Attributes::new())).0,
        "shell in no lump"
    );
    b.lump(vec![s]);
    b.edge(edge(circle, [0.0, TAU], [v, v]), name(5));
    assert_eq!(
        refused(b.build(Attributes::new())),
        ("edge bounds no face".into(), Some(("edge", 1)))
    );
    // Two coedges of one edge running the same way.
    let (mut b, plane, circle, p, v) = disc();
    let e = b.edge(edge(circle, [0.0, TAU], [v, v]), name(3));
    let f = b.face(
        face(plane, Sense::Same),
        vec![
            vec![coedge(e, Sense::Same, p)],
            vec![coedge(e, Sense::Same, p)],
        ],
        name(4),
    );
    let s = b.shell(vec![f]);
    b.lump(vec![s]);
    assert_eq!(
        refused(b.build(Attributes::new())),
        ("non-manifold edge".into(), Some(("edge", 0)))
    );
    // Two faces of one name.
    let (mut b, plane, circle, p, v) = disc();
    let e = b.edge(edge(circle, [0.0, TAU], [v, v]), name(3));
    let f = b.face(
        face(plane, Sense::Same),
        vec![vec![coedge(e, Sense::Same, p)]],
        name(1),
    );
    let g = b.face(
        face(plane, Sense::Reversed),
        vec![vec![coedge(e, Sense::Reversed, p)]],
        name(1),
    );
    let s = b.shell(vec![f, g]);
    b.lump(vec![s]);
    assert_eq!(
        refused(b.build(Attributes::new())),
        ("duplicate name".into(), Some(("face", 1)))
    );
    // A dangling id, from another body's builder.
    let (mut b, plane, _, p, v) = disc();
    let elsewhere = cuboid([0.0; 3], [1.0; 3]).build(Attributes::new()).unwrap();
    let far = CurveId::from_index(&elsewhere, 11).unwrap();
    let e = b.edge(edge(far, [0.0, TAU], [v, v]), name(3));
    let f = b.face(
        face(plane, Sense::Same),
        vec![vec![coedge(e, Sense::Same, p)]],
        name(4),
    );
    let s = b.shell(vec![f]);
    b.lump(vec![s]);
    assert_eq!(refused(b.build(Attributes::new())).0, "dangling reference");
    // A negative precision; a tolerance that isn't positive.
    let (mut b, ..) = disc();
    b.vertex([0.0; 3], Some(-1.0), name(9));
    assert_eq!(refused(b.build(Attributes::new())).0, "precision negative");
    assert_eq!(
        refused(BrepBuilder::new(name(0), 0.0).build(Attributes::new())).0,
        "tolerance not positive"
    );
    assert!(
        Placement::new([
            [2.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0]
        ])
        .is_err()
    );
}

#[test]
fn consistency_is_checked_at_ends_and_joins() {
    // A vertex beyond P from its edge's end; within its own precision it
    // is one with it.
    let off = |precision: Option<f64>| {
        let mut b = BrepBuilder::new(name(0), P);
        let plane = b.surface(Surface::Plane(Plane::new(Frame::WORLD)));
        let circle = b.curve(Curve::Circle(Circle::new(Frame::WORLD, 1.0).unwrap()));
        let p = b.pcurve(PCurve::Circle(
            Circle2::new([0.0; 2], [1.0, 0.0], 1.0, true).unwrap(),
        ));
        let v = b.vertex([1.05, 0.0, 0.0], precision, name(1));
        let e = b.edge(edge(circle, [0.0, TAU], [v, v]), name(2));
        let f = b.face(
            face(plane, Sense::Same),
            vec![vec![coedge(e, Sense::Same, p)]],
            name(3),
        );
        let s = b.shell(vec![f]);
        b.lump(vec![s]);
        b.build(Attributes::new())
    };
    assert_eq!(refused(off(None)).0, "vertex off its edge");
    assert!(off(Some(0.06)).is_ok());
    // A p-curve whose end maps elsewhere.
    let mut b = BrepBuilder::new(name(0), P);
    let plane = b.surface(Surface::Plane(Plane::new(Frame::WORLD)));
    let circle = b.curve(Curve::Circle(Circle::new(Frame::WORLD, 1.0).unwrap()));
    let p = b.pcurve(PCurve::Circle(
        Circle2::new([0.5, 0.0], [1.0, 0.0], 1.0, true).unwrap(),
    ));
    let v = b.vertex([1.0, 0.0, 0.0], None, name(1));
    let e = b.edge(edge(circle, [0.0, TAU], [v, v]), name(2));
    let f = b.face(
        face(plane, Sense::Same),
        vec![vec![coedge(e, Sense::Same, p)]],
        name(3),
    );
    let s = b.shell(vec![f]);
    b.lump(vec![s]);
    assert_eq!(
        refused(b.build(Attributes::new())),
        ("p-curve off its edge".into(), Some(("coedge", 0)))
    );
    // Consecutive p-curves a period apart on the side of a cylinder: the
    // loop jumps lifts mid-way.
    let mut b = cylinder(1.0, 2.0);
    let side = b.surface(Surface::Cylinder(Cylinder::new(Frame::WORLD, 1.0).unwrap()));
    let a = b.vertex([1.0, 0.0, 5.0], None, name(100));
    let c = b.vertex([-1.0, 0.0, 5.0], None, name(101));
    let raised = Frame::new(
        [0.0, 0.0, 5.0],
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
    )
    .unwrap();
    let rim = b.curve(Curve::Circle(Circle::new(raised, 1.0).unwrap()));
    let half = b.edge(edge(rim, [0.0, PI], [a, c]), name(102));
    let back = b.edge(edge(rim, [PI, TAU], [c, a]), name(103));
    let first = b.pcurve(PCurve::Line(Line2::new([0.0, 5.0], [1.0, 0.0]).unwrap()));
    let lifted = b.pcurve(PCurve::Line(Line2::new([TAU, 5.0], [1.0, 0.0]).unwrap()));
    let f = b.face(
        face(side, Sense::Same),
        vec![vec![
            coedge(half, Sense::Same, first),
            coedge(back, Sense::Same, lifted),
        ]],
        name(104),
    );
    let s = b.shell(vec![f]);
    b.lump(vec![s]);
    assert_eq!(refused(b.build(Attributes::new())).0, "loop gap");
    // Closed NURBS rows apart.
    let points: Vec<[f64; 3]> = [[1.0, 0.0], [0.0, 1.0], [-1.0, 0.0], [0.9, 0.0]]
        .iter()
        .flat_map(|&[x, y]| [[x, y, 0.0], [x, y, 1.0]])
        .collect();
    let u: Column<f64> = vec![0.0, 0.0, 1.0, 2.0, 3.0, 3.0].into();
    let v: Column<f64> = vec![0.0, 0.0, 1.0, 1.0].into();
    let open = NurbsSurface::new(
        [1, 1],
        [u, v],
        points,
        None,
        [Some(SeamContinuity::Position), None],
    )
    .unwrap();
    let mut b = BrepBuilder::new(name(0), P);
    b.surface(Surface::Nurbs(Arc::new(open)));
    assert_eq!(
        refused(b.build(Attributes::new())),
        ("closed rows apart".into(), Some(("surface", 0)))
    );
}
