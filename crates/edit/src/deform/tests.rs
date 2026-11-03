use super::*;

fn close(a: Vec3, b: Vec3) -> bool {
    (a - b).length() < 1e-12
}

fn v(x: f64, y: f64, z: f64) -> Vec3 {
    Vec3::new(x, y, z)
}

const ONES: Vec3 = Vec3::new(1.0, 1.0, 1.0);
const STILL: [f64; 4] = [0.0, 0.0, 0.0, 1.0];

/// A quarter turn about Z.
fn quarter() -> [f64; 4] {
    axis_angle(Vec3::Z, std::f64::consts::FRAC_PI_2)
}

fn deform(step: Move, symmetry: Option<Symmetry>) -> Deform {
    Deform {
        step,
        weights: None,
        symmetry,
    }
}

#[test]
fn full_weight_is_the_matrix_and_none_is_still() {
    let m = Move::new(
        v(1.0, 2.0, 3.0),
        quarter(),
        v(2.0, 1.0, 1.0),
        v(0.5, 0.0, 0.0),
    )
    .unwrap();
    let d = Deform::rigid(m);
    for p in [Vec3::ZERO, Vec3::X, v(0.3, -2.0, 4.0)] {
        assert!(close(d.place(p, 1.0), d.xform().apply_point(p)), "{p:?}");
        assert_eq!(d.place(p, 0.0), p);
    }
    // A quarter turn about the pivot, then the translation.
    assert!(close(
        d.place(v(1.5, 0.0, 0.0), 1.0),
        v(1.5, 2.0 + 2.0, 3.0)
    ));
}

#[test]
fn half_weight_goes_half_way() {
    let m = Move::new(v(2.0, 0.0, 0.0), quarter(), ONES, Vec3::ZERO).unwrap();
    // Half the translation and an eighth turn.
    let eighth = std::f64::consts::FRAC_PI_4;
    let q = Deform::rigid(m).place(Vec3::X, 0.5);
    assert!(close(q, v(1.0 + eighth.cos(), eighth.sin(), 0.0)));
    assert!((m.angle() - std::f64::consts::FRAC_PI_2).abs() < 1e-12);
}

#[test]
fn a_rotation_and_its_negation_move_alike() {
    let q = quarter();
    let a = Deform::rigid(Move::new(Vec3::ZERO, q, ONES, Vec3::ZERO).unwrap());
    let b = Deform::rigid(Move::new(Vec3::ZERO, q.map(|c| -c), ONES, Vec3::ZERO).unwrap());
    let p = v(1.0, 2.0, 3.0);
    assert!(close(a.place(p, 0.3), b.place(p, 0.3)));
    assert!(Move::new(v(f64::NAN, 0.0, 0.0), q, ONES, Vec3::ZERO).is_none());
    assert!(Move::new(Vec3::ZERO, [0.0; 4], ONES, Vec3::ZERO).is_none());
}

#[test]
fn mirrored_sides_move_as_images_and_the_plane_holds() {
    let mirror = Symmetry::new(Axis::X, 0.0, 1e-6, Side::Positive).unwrap();
    let m = Move::new(v(1.0, 0.5, 0.0), STILL, ONES, Vec3::ZERO).unwrap();
    let d = deform(m, Some(mirror));
    // The primary side moves by the move, the other by its image.
    assert_eq!(d.place(v(2.0, 0.0, 0.0), 1.0), v(3.0, 0.5, 0.0));
    assert_eq!(d.place(v(-2.0, 0.0, 0.0), 1.0), v(-3.0, 0.5, 0.0));
    // On the plane: moved, then back onto it, at any weight.
    assert_eq!(d.place(Vec3::ZERO, 1.0), v(0.0, 0.5, 0.0));
    assert_eq!(d.place(Vec3::ZERO, 0.5), v(0.0, 0.25, 0.0));
    // Half weight on the mirrored side: half of the image.
    assert_eq!(d.place(v(-2.0, 0.0, 0.0), 0.5), v(-2.5, 0.25, 0.0));
    assert!(Symmetry::new(Axis::Y, f64::INFINITY, 0.0, Side::On).is_none());
    assert!(Symmetry::new(Axis::Y, 0.0, -1.0, Side::On).is_none());
}

#[test]
fn a_symmetry_with_every_side_primary_mirrors_nothing() {
    let all = Symmetry::new(Axis::Z, 1.0, 0.0, Side::On).unwrap();
    let d = deform(
        Move::new(v(0.0, 0.0, 1.0), STILL, ONES, Vec3::ZERO).unwrap(),
        Some(all),
    );
    assert_eq!(d.place(v(0.0, 0.0, 3.0), 1.0), v(0.0, 0.0, 4.0));
    assert_eq!(d.place(v(0.0, 0.0, -3.0), 1.0), v(0.0, 0.0, -2.0));
    assert_eq!(all.side(v(0.0, 0.0, 1.0)), Side::On);
    assert!(!all.mirrored(v(0.0, 0.0, -3.0)));
}

#[test]
fn scale_acts_along_the_axes() {
    // Stretch 2× along the axes' X, turned a quarter about Z: the value's Y.
    let m = Move::new(Vec3::ZERO, STILL, v(2.0, 1.0, 1.0), Vec3::ZERO)
        .unwrap()
        .with_axes(quarter())
        .unwrap();
    let d = Deform::rigid(m);
    assert!(close(d.place(Vec3::Y, 1.0), v(0.0, 2.0, 0.0)));
    assert!(close(d.place(Vec3::X, 1.0), Vec3::X));
    assert!(!m.uniform_scale());
    assert!(
        Move::new(Vec3::ZERO, STILL, ONES, Vec3::ZERO)
            .unwrap()
            .uniform_scale()
    );
    assert!(m.with_axes([0.0; 4]).is_none());
}

#[test]
fn axes_index_coordinates() {
    assert_eq!([Axis::X, Axis::Y, Axis::Z].map(Axis::index), [0, 1, 2]);
}
