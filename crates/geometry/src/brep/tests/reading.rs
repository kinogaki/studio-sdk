//! The kernel's bodies read back, edited and fingerprinted.
use super::*;
use crate::{Attribute, Domain, ErrorKind, Meaning};
use std::f64::consts::FRAC_PI_4;
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
    }
    // Loops close: each coedge ends where the next starts.
    for f in t.face_ids() {
        for l in t.face_loops(f) {
            let ring: Vec<CoedgeId> = t.loop_coedges(l).collect();
            for (k, &c) in ring.iter().enumerate() {
                assert_eq!(t.coedge_end(c), t.coedge_start(ring[(k + 1) % ring.len()]));
            }
        }
    }
    // Ids are checked against the body.
    assert_eq!(FaceId::from_index(&body, 5).map(FaceId::index), Some(5));
    assert_eq!(FaceId::from_index(&body, 6), None);
    assert_eq!(
        VertexId::from_index(&body, 7).map(|v| body.geometry().point(v)),
        Some(crate::Vec3::new(2.0, 3.0, 4.0))
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
    assert_eq!(
        body.geometry().point(t.edge(pole).vertices()[0]),
        [0.0; 3].into()
    );
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
                surface.point(p.point(s)).unwrap().into(),
                curve.point(s).into(),
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
            crate::Geometry::empty(),
            Vec::new(),
            Attributes::new(),
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
        kind: DatumKind::Point(at.into()),
        hint: DatumHint {
            center: at.into(),
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
                Domain::Primitive,
                crate::names::NAME,
                Attribute::text(crate::Text::from_strs(names)),
            )
            .unwrap();
        let lines = vec![crate::Kind::POLYLINE; 2].into();
        let t = crate::Primitives::new(
            4,
            vec![0, 2, 4].into(),
            vec![0, 1, 2, 3].into(),
            lines,
            None,
        );
        crate::Geometry::new(points.into(), t.unwrap(), named).unwrap()
    };
    let value = |datums: Vec<Datum>, curves: crate::Geometry| {
        Brep::new(Vec::new(), curves, datums, Attributes::new())
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
            Domain::Primitive,
            "Cd",
            Attribute::tuples(vec![[1.0f32, 0.0, 0.0]; 6], Meaning::Color).unwrap(),
        )
        .unwrap()
        .with(
            Domain::Edge,
            "cad:weight",
            Attribute::tuples(vec![1.0; 12], Meaning::None).unwrap(),
        )
        .unwrap()
        .with(
            Domain::Point,
            "cad:weight",
            Attribute::tuples(vec![1.0; 8], Meaning::None).unwrap(),
        )
        .unwrap()
        .with(
            Domain::Object,
            "cad:output_by",
            Attribute::tuples::<i64>(vec![7], Meaning::None).unwrap(),
        )
        .unwrap();
    let body = b.clone().build(faces.clone()).unwrap();
    assert_eq!(body.count(Domain::Instance), None);
    assert_eq!(body.count(Domain::Vertex), None);
    let short = faces.with(
        Domain::Primitive,
        "Cd",
        Attribute::tuples(vec![[1.0f32; 3]; 5], Meaning::Color).unwrap(),
    );
    assert_eq!(
        b.clone().build(short.unwrap()).unwrap_err().kind,
        ErrorKind::Invalid
    );
    let corner = Attributes::new().with(
        Domain::Vertex,
        "w",
        Attribute::tuples(vec![0.0; 24], Meaning::None).unwrap(),
    );
    assert_eq!(
        b.clone().build(corner.unwrap()).unwrap_err().kind,
        ErrorKind::Invalid
    );
    // An Instance attribute: a body has no such domain (instances place
    // bodies as standard Instances).
    let names = Attributes::new()
        .with(
            Domain::Instance,
            "instance",
            Attribute::tuples::<i64>(vec![0], Meaning::None).unwrap(),
        )
        .unwrap();
    assert_eq!(b.clone().build(names).unwrap_err().kind, ErrorKind::Invalid);
}

#[test]
fn a_value_holds_bodies_curves_and_datums() {
    let body = cuboid([0.0; 3], [1.0; 3]).build(Attributes::new()).unwrap();
    let datum = Datum {
        name: name(1),
        kind: DatumKind::Axis {
            origin: [0.0; 3].into(),
            direction: [0.0, 0.0, 1.0].into(),
        },
        hint: DatumHint {
            center: [0.0; 3].into(),
            size: 2.0,
        },
    };
    let value = Brep::new(
        vec![body.clone()],
        crate::Geometry::empty(),
        vec![datum.clone()],
        Attributes::new(),
    )
    .unwrap();
    assert_eq!((value.bodies().len(), value.datums().len()), (1, 1));
    let skew = Datum {
        kind: DatumKind::Axis {
            origin: [0.0; 3].into(),
            direction: [0.0, 0.0, 2.0].into(),
        },
        ..datum
    };
    let e = Brep::new(
        Vec::new(),
        crate::Geometry::empty(),
        vec![skew],
        Attributes::new(),
    )
    .unwrap_err();
    assert_eq!(e.diagnostic.element, Some(0));
    let detail = Attributes::new()
        .with(
            Domain::Primitive,
            "w",
            Attribute::tuples(vec![0.0], Meaning::None).unwrap(),
        )
        .unwrap();
    assert!(value.with_detail(detail).is_err());
    // Free curves are curves and polylines: a polygon among them refuses.
    let tri = crate::Primitives::of_polygons(3, vec![0, 3].into(), vec![0, 1, 2].into()).unwrap();
    let polygon = crate::Geometry::new(vec![[0.0; 3]; 3].into(), tri, Attributes::new()).unwrap();
    let e = value.with_curves(polygon).unwrap_err();
    assert_eq!(
        (e.diagnostic.element, e.message()),
        (Some(0), "free curve 0 is a polygon")
    );
    assert_eq!(Brep::empty(), Brep::default());
}
