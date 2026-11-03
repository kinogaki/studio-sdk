//! `kinogaki.brep` evaluation against closed forms: the analytic charts,
//! offsets, swept surfaces and NURBS, and the one evaluator geometry
//! curves and the B-rep share.
use super::*;
use crate::{Column, CurveData, Frame, Kind, Knots, Primitives, Vec3};
use std::f64::consts::{FRAC_PI_2, FRAC_PI_4, PI, TAU};
use std::sync::Arc;

/// Whether `a` and `b` agree within `tolerance` in every component.
fn close(a: impl Into<Vec3>, b: impl Into<Vec3>, tolerance: f64) -> bool {
    let d = a.into() - b.into();
    [d.x, d.y, d.z].iter().all(|c| c.abs() <= tolerance)
}

/// `a × b` at unit length.
fn unit_cross(a: Vec3, b: Vec3) -> Vec3 {
    a.cross(b).normalized().unwrap_or(Vec3::ZERO)
}

#[test]
fn analytic_charts_follow_the_contract() {
    let f = Frame::new(
        Vec3::from([1.0, 2.0, 3.0]),
        Vec3::from([0.0, 1.0, 0.0]),
        Vec3::from([-1.0, 0.0, 0.0]),
        Vec3::from([0.0, 0.0, 1.0]),
    )
    .unwrap();
    let sphere = Surface::Sphere(Sphere::new(f, 2.0).unwrap());
    // The pole of the sphere, its normal along z.
    assert!(close(
        sphere.point([0.3, FRAC_PI_2]).unwrap(),
        Vec3::from([1.0, 2.0, 5.0]),
        1e-12
    ));
    assert!(close(
        sphere.normal([0.3, FRAC_PI_2]).unwrap(),
        Vec3::from([0.0, 0.0, 1.0]),
        1e-12
    ));
    // θ = π/2 on the equator: along the frame's y.
    assert!(close(
        sphere.point([FRAC_PI_2, 0.0]).unwrap(),
        Vec3::from([-1.0, 2.0, 3.0]),
        1e-12
    ));
    let cylinder = Surface::Cylinder(Cylinder::new(f, 1.0).unwrap());
    let [p, du, dv] = cylinder.partials([0.0, 2.0]).unwrap();
    assert!(close(p, Vec3::from([1.0, 3.0, 5.0]), 1e-12));
    // u × v is the outward normal.
    assert!(close(
        unit_cross(du, dv),
        cylinder.normal([0.0, 2.0]).unwrap(),
        1e-12
    ));
    assert!(close(
        cylinder.normal([0.0, 2.0]).unwrap(),
        Vec3::from([0.0, 1.0, 0.0]),
        1e-12
    ));
    // A cone of radius 1 at h = 0, half angle π/4: radius 3 at h = 2.
    let cone = Surface::Cone(Cone::new(Frame::WORLD, 1.0, FRAC_PI_4).unwrap());
    assert!(close(
        cone.point([0.0, 2.0]).unwrap(),
        Vec3::from([3.0, 0.0, 2.0]),
        1e-12
    ));
    let n = cone.normal([0.0, 1.0]).unwrap();
    assert!(close(
        n,
        Vec3::from([FRAC_PI_4.cos(), 0.0, -FRAC_PI_4.sin()]),
        1e-12
    ));
    let torus = Surface::Torus(Torus::new(Frame::WORLD, 3.0, 1.0).unwrap());
    assert!(close(
        torus.point([PI, PI]).unwrap(),
        Vec3::from([-2.0, 0.0, 0.0]),
        1e-12
    ));
    assert!(close(
        torus.normal([PI, PI]).unwrap(),
        Vec3::from([1.0, 0.0, 0.0]),
        1e-12
    ));
    assert_eq!(torus.periods(), [Some(TAU); 2]);
    // An offset of the sphere is the sphere of radius 2.5.
    let offset = Surface::Offset(Offset::new(Arc::new(sphere.clone()), 0.5).unwrap());
    assert!(close(
        offset.point([0.0, FRAC_PI_2]).unwrap(),
        Vec3::from([1.0, 2.0, 5.5]),
        1e-12
    ));
    assert!(Offset::new(Arc::new(offset), 1.0).is_err());
}

/// An offset's partials are `∂S + d·∂N`: a radius-2 cylinder offset by 1
/// turns at radius 3, and over every analytic base they agree with finite
/// differences of the offset's points. Over a base without closed-form
/// normal derivatives they are refused, its points still given.
#[test]
fn offset_partials_follow_the_displaced_surface() {
    let cylinder = Arc::new(Surface::Cylinder(Cylinder::new(Frame::WORLD, 2.0).unwrap()));
    let offset = Surface::Offset(Offset::new(cylinder, 1.0).unwrap());
    let [p, du, dv] = offset.partials([0.0, 0.5]).unwrap();
    assert!(close(p, Vec3::from([3.0, 0.0, 0.5]), 1e-12));
    assert!(
        close(du, Vec3::from([0.0, 3.0, 0.0]), 1e-12),
        "radius 3, not 2: {du:?}"
    );
    assert!(close(dv, Vec3::from([0.0, 0.0, 1.0]), 1e-12));
    let tilted = Frame::new(
        Vec3::from([1.0, -2.0, 0.5]),
        Vec3::from([0.0, 1.0, 0.0]),
        Vec3::from([0.0, 0.0, 1.0]),
        Vec3::from([1.0, 0.0, 0.0]),
    )
    .unwrap();
    let bases = [
        Surface::Plane(Plane::new(tilted)),
        Surface::Cylinder(Cylinder::new(tilted, 1.5).unwrap()),
        Surface::Cone(Cone::new(tilted, 1.0, 0.4).unwrap()),
        Surface::Sphere(Sphere::new(tilted, 2.0).unwrap()),
        Surface::Torus(Torus::new(tilted, 3.0, 1.0).unwrap()),
    ];
    let h = 1e-6;
    for base in bases {
        for distance in [0.5, -0.25] {
            let offset = Surface::Offset(Offset::new(Arc::new(base.clone()), distance).unwrap());
            for uv in [[0.3, 0.2], [2.0, -0.7], [4.5, 1.1]] {
                let [p, du, dv] = offset.partials(uv).unwrap();
                assert!(close(p, offset.point(uv).unwrap(), 1e-12));
                let at = |u: f64, v: f64| offset.point([u, v]).unwrap();
                let central = |a: Vec3, b: Vec3| (a - b) / (2.0 * h);
                let (u, v) = (uv[0], uv[1]);
                let fu = central(at(u + h, v), at(u - h, v));
                let fv = central(at(u, v + h), at(u, v - h));
                assert!(
                    close(du, fu, 1e-6),
                    "{base:?} ∂u at {uv:?}: {du:?} vs {fu:?}"
                );
                assert!(
                    close(dv, fv, 1e-6),
                    "{base:?} ∂v at {uv:?}: {dv:?} vs {fv:?}"
                );
            }
        }
    }
    let line = Arc::new(Curve::Line(
        Line::new(Vec3::from([0.0; 3]), Vec3::from([1.0, 0.0, 0.0])).unwrap(),
    ));
    let extruded = Arc::new(Surface::Extrusion(
        Extrusion::new(line, Vec3::from([0.0, 1.0, 0.0])).unwrap(),
    ));
    let offset = Surface::Offset(Offset::new(extruded, 1.0).unwrap());
    assert!(
        offset.partials([0.5, 0.5]).is_none(),
        "no closed form: refused"
    );
    assert!(close(
        offset.point([0.5, 0.5]).unwrap(),
        Vec3::from([0.5, 0.5, 1.0]),
        1e-12
    ));
}

#[test]
fn provenance_surfaces_sweep_their_profiles() {
    let circle = Arc::new(Curve::Circle(
        Circle::new(
            Frame::new(
                Vec3::from([3.0, 0.0, 0.0]),
                Vec3::from([1.0, 0.0, 0.0]),
                Vec3::from([0.0, 0.0, 1.0]),
                Vec3::from([0.0, -1.0, 0.0]),
            )
            .unwrap(),
            1.0,
        )
        .unwrap(),
    ));
    // A circle revolved about z is a torus.
    let revolved = Surface::Revolution(
        Revolution::new(
            circle.clone(),
            Vec3::from([0.0; 3]),
            Vec3::from([0.0, 0.0, 1.0]),
        )
        .unwrap(),
    );
    let torus = Surface::Torus(Torus::new(Frame::WORLD, 3.0, 1.0).unwrap());
    for (theta, phi) in [(0.0, 0.0), (1.0, 2.0), (4.0, 5.5)] {
        assert!(close(
            revolved.point([theta, phi]).unwrap(),
            torus.point([theta, phi]).unwrap(),
            1e-12
        ));
    }
    // Extruded along y: a cylinder of radius 1 about the line x = 3.
    let extruded =
        Surface::Extrusion(Extrusion::new(circle, Vec3::from([0.0, -1.0, 0.0])).unwrap());
    assert!(close(
        extruded.point([FRAC_PI_2, 2.0]).unwrap(),
        Vec3::from([3.0, -2.0, 1.0]),
        1e-12
    ));
    assert_eq!(extruded.periods(), [Some(TAU), None]);
    let line = Arc::new(Curve::Line(
        Line::new(Vec3::from([0.0; 3]), Vec3::from([1.0, 0.0, 0.0])).unwrap(),
    ));
    assert!(Extrusion::new(line, Vec3::from([1.0, 0.0, 0.0])).is_err());
}

#[test]
fn rational_nurbs_hold_arcs_exactly() {
    // A quarter circle as a rational quadratic.
    let w = FRAC_PI_4.cos();
    let arc = NurbsCurve::new(
        2,
        vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        vec![[1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
        Some(vec![1.0, w, 1.0].into()),
    )
    .unwrap();
    for k in 0..=10 {
        let p = arc.point(k as f64 / 10.0);
        assert!((p[0].hypot(p[1]) - 1.0).abs() < 1e-14);
    }
    // Degree up to the product limit, and the knots' rules.
    let flat = |d: u8| {
        NurbsCurve::new(
            d,
            [vec![0.0; d as usize + 1], vec![1.0; d as usize + 1]].concat(),
            vec![[0.0; 3]; d as usize + 1],
            None,
        )
    };
    assert!(flat(25).is_ok());
    assert!(
        flat(26)
            .unwrap_err()
            .message()
            .starts_with("degree out of range: ")
    );
    let knots = |k: Vec<f64>| NurbsCurve::new(1, k, vec![[0.0; 2]; 3], None);
    assert!(knots(vec![0.0, 0.0, 0.5, 1.0, 1.0]).is_ok());
    assert!(knots(vec![0.0, 0.0, 1.0, 0.5, 1.0]).is_err(), "decreasing");
    assert!(
        knots(vec![0.0, 0.0, 0.0, 1.0, 1.0]).is_err(),
        "end knot too often"
    );
    assert!(
        NurbsCurve::new(
            2,
            vec![0.0, 0.0, 0.0, 0.5, 0.5, 0.5, 1.0, 1.0, 1.0],
            vec![[0.0; 2]; 6],
            None
        )
        .is_err(),
        "interior knot too often"
    );
    assert!(
        NurbsCurve::new(
            1,
            vec![0.0, 0.0, 1.0, 1.0],
            vec![[0.0; 2]; 2],
            Some(vec![1.0, 0.0].into())
        )
        .is_err(),
        "weight 0"
    );
}

#[test]
fn a_closed_nurbs_direction_wraps() {
    // A degree-1 tube: four rows round a square, the first again last.
    let ring = [[1.0, 0.0], [0.0, 1.0], [-1.0, 0.0], [0.0, -1.0], [1.0, 0.0]];
    let points: Vec<[f64; 3]> = ring
        .iter()
        .flat_map(|&[x, y]| [[x, y, 0.0], [x, y, 1.0]])
        .collect();
    let u: Column<f64> = vec![0.0, 0.0, 1.0, 2.0, 3.0, 4.0, 4.0].into();
    let v: Column<f64> = vec![0.0, 0.0, 1.0, 1.0].into();
    let tube = NurbsSurface::new(
        [1, 1],
        [u, v],
        points,
        None,
        [Some(SeamContinuity::Position), None],
    )
    .unwrap();
    assert_eq!(tube.periods(), [Some(4.0), None]);
    let s = Surface::Nurbs(Arc::new(tube));
    assert_eq!(s.point([5.5, 0.5]), s.point([1.5, 0.5]));
    assert_eq!(s.point([-0.5, 0.5]), s.point([3.5, 0.5]));
}

#[test]
fn geometry_curves_and_brep_nurbs_evaluate_alike() {
    use crate::Basis;
    // A rational cubic with a double interior knot, and its polynomial
    // twin: one evaluator behind both values.
    let knots = vec![0.0, 0.0, 0.0, 0.0, 0.3, 0.3, 0.7, 1.5, 1.5, 1.5, 1.5];
    let points = vec![
        [0.0, 0.0, 0.0],
        [1.0, 2.0, 0.5],
        [2.0, -1.0, 1.0],
        [3.0, 0.5, -0.5],
        [4.0, 2.5, 0.0],
        [5.0, 0.0, 1.5],
        [6.0, 1.0, 0.0],
    ];
    for weights in [None, Some(vec![1.0, 0.5, 2.0, 1.0, 0.7, 1.3, 1.0])] {
        let data = CurveData::new(
            vec![3].into(),
            Knots::new(vec![0, 11].into(), knots.clone().into()),
            weights.clone().map(Column::from),
        );
        let kinds = vec![Kind::curve(Basis::Nurbs, false)];
        let vertices: Vec<u32> = (0..7).collect();
        let table = Primitives::new(
            7,
            vec![0, 7].into(),
            vertices.into(),
            kinds.into(),
            Some(data),
        );
        let value =
            crate::Geometry::new(points.clone().into(), table.unwrap(), Attributes::new()).unwrap();
        let curve = value.curve(0);
        let nurbs = NurbsCurve::new(
            3,
            knots.clone(),
            points.clone(),
            weights.clone().map(Column::from),
        )
        .unwrap();
        assert_eq!(curve.domain(), nurbs.domain());
        for k in 0..=150 {
            let t = 1.5 * k as f64 / 150.0;
            assert!(close(curve.point(t), nurbs.point(t), 1e-12), "at {t}");
        }
    }
}
