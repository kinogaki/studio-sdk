//! The B-rep fixtures: plain bodies a small kernel written on the SDK
//! alone builds (a box, a closed cylinder with no seam, a cone with its
//! apex a pole edge, a whole sphere with no loops, a trimmed NURBS patch),
//! and one value holding all of them with every part the type has.
use kinogaki_geometry::brep::{
    Brep, BrepBuilder, Circle, Circle2, CoedgeSpec, Cone, Curve, CurveId, Cylinder, Datum,
    DatumHint, DatumKind, EdgeId, EdgeSpec, End, FaceSpec, Frame, History, Line, Line2, Name,
    NurbsCurve, NurbsSurface, OperationId, PCurve, PCurveId, Placement, Plane, Role, Sense, Sphere,
    Surface, SurfaceId, VertexId,
};
use kinogaki_geometry::{Attribute, Attributes, Basis, Class, Column, Curves, Text, TypeInfo};
use std::f64::consts::TAU;
use std::sync::Arc;

/// The bodies' positional tolerance.
const P: f64 = 0.01;

/// The B-rep fixtures, by name.
pub(super) fn breps() -> Vec<(String, Brep)> {
    let one = |b: BrepBuilder| {
        let body = b.build(Attributes::new()).expect("a fixture body is valid");
        Brep::new(
            vec![body],
            Curves::empty(),
            Vec::new(),
            Attributes::new(),
            History::default(),
        )
        .expect("a fixture value is valid")
    };
    vec![
        ("box".to_owned(), one(cuboid([0.0; 3], [1.0, 2.0, 3.0]))),
        ("cylinder".to_owned(), one(cylinder(1.0, 2.0))),
        ("cone".to_owned(), one(cone(0.5, 1.0))),
        ("sphere".to_owned(), one(sphere(1.0))),
        ("trimmed patch".to_owned(), one(patch(0.5, [0.25, 0.75]))),
        ("every part".to_owned(), every_part()),
    ]
}

/// A value with every part: two bodies (one placed twice, with
/// precisions and attributes on every class), free curves, a datum of
/// each kind, Detail attributes and a history over names of every node
/// and role.
fn every_part() -> Brep {
    let mut b = cuboid([0.0; 3], [1.0; 3]);
    let up = Placement::new([
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 3.0],
    ])
    .expect("a rigid placement");
    b.placement(Placement::IDENTITY, vec![up]);
    let attributes = Attributes::new()
        .with(Class::Object, "cad:output_by", Attribute::integer(vec![7]))
        .and_then(|a| a.with(Class::Instance, "instance", Attribute::integer(vec![0, 1])))
        .and_then(|a| {
            let red = Attribute::vector(vec![[1.0, 0.0, 0.0]; 6], TypeInfo::Color);
            a.with(Class::Primitive, "Cd", red)
        })
        .and_then(|a| a.with(Class::Edge, "cad:weight", Attribute::float(vec![0.5; 12])))
        .and_then(|a| a.with(Class::Point, "cad:weight", Attribute::float(vec![0.25; 8])))
        .expect("sized attributes");
    let boxed = b.build(attributes).expect("a fixture body is valid");
    let patch = patch(0.5, [0.25, 0.75])
        .build(Attributes::new())
        .expect("valid");
    let curves = Curves::builder(
        Basis::BSpline,
        vec![[0.0; 3], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 1.0]].into(),
        vec![0, 4].into(),
    )
    .build()
    .expect("a free curve");
    let hint = DatumHint {
        center: [0.0; 3],
        size: 1.0,
    };
    let datums = vec![
        Datum {
            name: name(900),
            kind: DatumKind::Plane(Frame::WORLD),
            hint,
        },
        Datum {
            name: name(901),
            kind: DatumKind::Axis {
                origin: [0.0; 3],
                direction: [0.0, 0.0, 1.0],
            },
            hint,
        },
        Datum {
            name: name(902),
            kind: DatumKind::Point([1.0, 2.0, 3.0]),
            hint: DatumHint {
                center: [1.0, 2.0, 3.0],
                size: 0.0,
            },
        },
    ];
    let by = OperationId(42);
    let face = Name::created(by, Role::Local { index: 0 });
    let piece = Name::part(&face, 1);
    let roles = [
        Role::Imported { body: 0, index: 3 },
        Role::ImportedBody { body: 1 },
        Role::Cap {
            profile: "12+,15-".into(),
            end: End::End,
        },
        Role::Side {
            profile: "12+".into(),
            curve: 2,
            segment: 1,
        },
        Role::Along(face.clone()),
        Role::At(piece.clone()),
        Role::Offset(face.clone()),
        Role::Copy {
            of: piece.clone(),
            index: 3,
        },
        Role::Between {
            faces: vec![face.clone(), piece.clone()].into(),
            index: 0,
        },
    ];
    let generated: Vec<Name> = roles.into_iter().map(|r| Name::created(by, r)).collect();
    let history = History {
        generated,
        modified: vec![face.clone()],
        split: vec![(face.clone(), vec![piece.clone(), Name::part(&face, 2)])],
        merged: vec![(
            vec![piece.clone(), face.clone()],
            Name::merged(&[piece, face]),
        )],
        deleted: vec![name(77)],
    };
    let detail = Attributes::new()
        .with(
            Class::Detail,
            "name",
            Attribute::text(Text::from_strs(["parts"])),
        )
        .expect("a Detail attribute");
    Brep::new(vec![boxed, patch], curves, datums, detail, history).expect("a fixture value")
}

fn name(index: u32) -> Name {
    Name::created(OperationId::NONE, Role::Primitive { index })
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn unit(a: [f64; 3]) -> [f64; 3] {
    let l = dot(a, a).sqrt();
    [a[0] / l, a[1] / l, a[2] / l]
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

/// A polyhedron of `points` and `faces` (each a cycle of point indices,
/// counter-clockwise seen from outside): planes, lines and chart lines.
fn polyhedron(points: &[[f64; 3]], faces: &[&[usize]]) -> BrepBuilder {
    let mut b = BrepBuilder::new(name(1000), P);
    let mut names = 0..;
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
    let p = |i: usize| {
        [
            if i & 1 == 0 { lo[0] } else { hi[0] },
            if i & 2 == 0 { lo[1] } else { hi[1] },
            if i & 4 == 0 { lo[2] } else { hi[2] },
        ]
    };
    let points: Vec<[f64; 3]> = (0..8).map(p).collect();
    polyhedron(
        &points,
        &[
            &[0, 2, 3, 1],
            &[4, 5, 7, 6],
            &[0, 1, 5, 4],
            &[2, 6, 7, 3],
            &[0, 4, 6, 2],
            &[1, 3, 7, 5],
        ],
    )
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
