//! Each rule the builder checks, broken once.
use super::*;
use crate::ErrorKind;

/// The refusal's short phrase (before the rule it broke), and the index of
/// the entity it blames. Its text names no id and no number.
fn refused(result: Result<Body, crate::Error>) -> (String, Option<usize>) {
    let e = result.unwrap_err();
    assert_eq!(e.kind, ErrorKind::Invalid);
    let text = e.to_string();
    assert!(!text.chars().any(|c| c.is_ascii_digit()), "{text}");
    let (phrase, rule) = text.split_once(": ").expect("a phrase, then the rule");
    assert!(!rule.is_empty());
    (phrase.to_string(), e.diagnostic.element)
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
        let v = b.vertex(Vec3::from([1.0, 0.0, 0.0]), None, name(1));
        (b, plane, circle, p, v)
    };
    // An open loop.
    let (mut b, plane, circle, p, v) = disc();
    let w = b.vertex(Vec3::from([0.0, 1.0, 0.0]), None, name(2));
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
        ("open loop".into(), Some(0))
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
    let w = b.vertex(Vec3::from([0.0, 1.0, 0.0]), None, name(2));
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
        ("face in no shell".into(), Some(0))
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
        ("edge bounds no face".into(), Some(1))
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
        ("non-manifold edge".into(), Some(0))
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
        ("duplicate name".into(), Some(1))
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
    b.vertex(Vec3::from([0.0; 3]), Some(-1.0), name(9));
    assert_eq!(refused(b.build(Attributes::new())).0, "precision negative");
    assert_eq!(
        refused(BrepBuilder::new(name(0), 0.0).build(Attributes::new())).0,
        "tolerance not positive"
    );
    let mut scaled = BrepBuilder::new(name(0), 0.01);
    let doubled = crate::Xform::from_rows([
        [2.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
    ]);
    scaled.placement(doubled, Vec::new());
    assert_eq!(
        refused(scaled.build(Attributes::new())).0,
        "placement not rigid"
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
        let v = b.vertex(Vec3::from([1.05, 0.0, 0.0]), precision, name(1));
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
    let v = b.vertex(Vec3::from([1.0, 0.0, 0.0]), None, name(1));
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
        ("p-curve off its edge".into(), Some(0))
    );
    // Consecutive p-curves a period apart on the side of a cylinder: the
    // loop jumps lifts mid-way.
    let mut b = cylinder(1.0, 2.0);
    let side = b.surface(Surface::Cylinder(Cylinder::new(Frame::WORLD, 1.0).unwrap()));
    let a = b.vertex(Vec3::from([1.0, 0.0, 5.0]), None, name(100));
    let c = b.vertex(Vec3::from([-1.0, 0.0, 5.0]), None, name(101));
    let raised = Frame::new(
        Vec3::from([0.0, 0.0, 5.0]),
        Vec3::from([1.0, 0.0, 0.0]),
        Vec3::from([0.0, 1.0, 0.0]),
        Vec3::from([0.0, 0.0, 1.0]),
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
        ("closed rows apart".into(), Some(0))
    );
}
