//! [`gather_breps`]: several B-reps as one, each at a rigid placement.
//! A B-rep's carriers are exact, so a placement that isn't rigid has no
//! carrier to land on: a circle sheared is no circle, and a plane's chart
//! scaled would no longer be its p-curves'.
use super::gather::{Placed, gather};
use kinogaki_geometry::brep::{
    Body, Brep, Circle, Cone, Curve, Cylinder, Datum, DatumHint, DatumKind, Ellipse, Extrusion,
    Geometry as Carriers, Line, NurbsCurve, NurbsSurface, Offset, Plane, Revolution, Sphere,
    Surface, Torus,
};
use kinogaki_geometry::{Attributes, Column, Error, Frame, Vec3, Xform};
use std::sync::Arc;

/// How far a placement's axes may stray from orthonormal and still be
/// rigid: what composing a chain of rigid placements leaves.
const RIGID: f64 = 1e-9;

/// `values` as one value, each moved by its placement: their bodies, free
/// curves and datums in order, and the first one's Detail attributes. One
/// value at the identity is that value.
///
/// ```
/// use kinogaki_edit::editor::gather_breps;
/// use kinogaki_geometry::Xform;
/// use kinogaki_geometry::brep::Brep;
/// let empty = Brep::empty();
/// let both = gather_breps(&[(&empty, Xform::IDENTITY), (&empty, Xform::IDENTITY)]).unwrap();
/// assert!(both.bodies().is_empty());
/// ```
///
/// # Errors
/// A placement that scales, shears or mirrors, or what the value's checks
/// refuse.
pub fn gather_breps(values: &[(&Brep, Xform)]) -> Result<Brep, Error> {
    if let [(one, Xform::IDENTITY)] = values {
        return Ok((*one).clone());
    }
    let mut bodies = Vec::new();
    let mut datums = Vec::new();
    for (value, at) in values {
        if *at == Xform::IDENTITY {
            bodies.extend(value.bodies().iter().cloned());
            datums.extend(value.datums().iter().cloned());
            continue;
        }
        rigid(at)?;
        for body in value.bodies().iter() {
            bodies.push(placed_body(body, at)?);
        }
        for datum in value.datums().iter() {
            datums.push(placed_datum(datum, at)?);
        }
    }
    let curves: Vec<Placed<'_>> = (values.iter())
        .map(|(value, at)| Placed {
            value: value.curves(),
            at: *at,
        })
        .collect();
    let detail = values
        .first()
        .map_or_else(Attributes::new, |(v, _)| (**v.detail()).clone());
    Brep::new(bodies, gather(&curves)?, datums, detail)
}

/// Whether `at` turns and moves and does nothing else.
fn rigid(at: &Xform) -> Result<(), Error> {
    let [x, y, z] = at.axes();
    let near = |a: f64, b: f64| (a - b).abs() <= RIGID;
    let orthonormal = near(x.dot(x), 1.0)
        && near(y.dot(y), 1.0)
        && near(z.dot(z), 1.0)
        && near(x.dot(y), 0.0)
        && near(y.dot(z), 0.0)
        && near(z.dot(x), 0.0);
    match orthonormal && x.cross(y).dot(z) > 0.0 {
        true => Ok(()),
        false => Err(Error::invalid(
            "a B-rep only moves rigidly: this placement scales, shears or mirrors it",
        )),
    }
}

/// `v` of unit length again, as a chain of rotations leaves it.
fn direction(at: &Xform, v: Vec3) -> Vec3 {
    let turned = at.apply_vector(v);
    turned.normalized().unwrap_or(turned)
}

fn frame(f: &Frame, at: &Xform) -> Result<Frame, Error> {
    let x = direction(at, f.x());
    let turned = at.apply_vector(f.y());
    let z = x.cross(turned).normalized().unwrap_or(f.z());
    Frame::new(at.apply_point(f.origin()), x, z.cross(x), z)
}

fn points(column: &Column<[f64; 3]>, at: &Xform) -> Vec<[f64; 3]> {
    (column.iter())
        .map(|p| at.apply_point(Vec3::from(*p)).to_array())
        .collect()
}

fn curve(value: &Curve, at: &Xform) -> Result<Curve, Error> {
    Ok(match value {
        Curve::Line(l) => Curve::Line(Line::new(
            at.apply_point(l.origin()),
            direction(at, l.direction()),
        )?),
        Curve::Circle(c) => Curve::Circle(Circle::new(frame(c.frame(), at)?, c.radius())?),
        Curve::Ellipse(e) => Curve::Ellipse(Ellipse::new(frame(e.frame(), at)?, e.semi_axes())?),
        Curve::Nurbs(n) => Curve::Nurbs(Arc::new(NurbsCurve::new(
            n.degree(),
            n.knots().clone(),
            points(n.points(), at),
            n.weights().cloned(),
        )?)),
        _ => return Err(Error::invalid("a curve of a kind that can't be moved yet")),
    })
}

fn surface(value: &Surface, at: &Xform) -> Result<Surface, Error> {
    let profile = |c: &Arc<Curve>| curve(c, at).map(Arc::new);
    Ok(match value {
        Surface::Plane(p) => Surface::Plane(Plane::new(frame(p.frame(), at)?)),
        Surface::Cylinder(c) => {
            Surface::Cylinder(Cylinder::new(frame(c.frame(), at)?, c.radius())?)
        }
        Surface::Cone(c) => Surface::Cone(Cone::new(
            frame(c.frame(), at)?,
            c.radius(),
            c.half_angle(),
        )?),
        Surface::Sphere(s) => Surface::Sphere(Sphere::new(frame(s.frame(), at)?, s.radius())?),
        Surface::Torus(t) => {
            Surface::Torus(Torus::new(frame(t.frame(), at)?, t.major(), t.minor())?)
        }
        Surface::Nurbs(n) => Surface::Nurbs(Arc::new(NurbsSurface::new(
            n.degrees(),
            n.knots().clone(),
            points(n.points(), at),
            n.weights().cloned(),
            n.closed(),
        )?)),
        Surface::Extrusion(e) => Surface::Extrusion(Extrusion::new(
            profile(e.profile())?,
            direction(at, e.direction()),
        )?),
        Surface::Revolution(r) => Surface::Revolution(Revolution::new(
            profile(r.profile())?,
            at.apply_point(r.origin()),
            direction(at, r.axis()),
        )?),
        Surface::Offset(o) => {
            Surface::Offset(Offset::new(Arc::new(surface(o.base(), at)?), o.distance())?)
        }
        _ => {
            return Err(Error::invalid(
                "a surface of a kind that can't be moved yet",
            ));
        }
    })
}

/// `body` at `at`: its carriers moved, its charts and p-curves as they are
/// (a rigid move keeps every parameter).
fn placed_body(body: &Body, at: &Xform) -> Result<Body, Error> {
    let g = body.geometry();
    let surfaces: Result<Vec<_>, _> = g.surfaces().iter().map(|s| surface(s, at)).collect();
    let curves: Result<Vec<_>, _> = g.curves().iter().map(|c| curve(c, at)).collect();
    body.with_geometry(Carriers::new(
        points(g.points(), at),
        surfaces?,
        curves?,
        g.pcurves().clone(),
    )?)
}

fn placed_datum(datum: &Datum, at: &Xform) -> Result<Datum, Error> {
    let kind = match datum.kind {
        DatumKind::Plane(f) => DatumKind::Plane(frame(&f, at)?),
        DatumKind::Axis {
            origin,
            direction: d,
        } => DatumKind::Axis {
            origin: at.apply_point(origin),
            direction: direction(at, d),
        },
        DatumKind::Point(p) => DatumKind::Point(at.apply_point(p)),
        _ => return Err(Error::invalid("a datum of a kind that can't be moved yet")),
    };
    Ok(Datum {
        name: datum.name.clone(),
        kind,
        hint: DatumHint {
            center: at.apply_point(datum.hint.center),
            size: datum.hint.size,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A quarter turn about z and a move: rigid, with whole numbers.
    fn turned() -> Xform {
        Xform::from_axes(Vec3::Y, -Vec3::X, Vec3::Z, Vec3::new(10.0, 0.0, 5.0))
    }

    fn same(a: Vec3, b: Vec3) {
        assert!((a - b).length() < 1e-9, "{a:?} and {b:?}");
    }

    /// Every kind of carrier lands on the same points as what it was,
    /// moved.
    #[test]
    fn placed_moves_every_surface_and_curve() {
        let at = turned();
        let line = Curve::Line(Line::new(Vec3::X, Vec3::Y).unwrap());
        let circle = Curve::Circle(Circle::new(Frame::WORLD, 2.0).unwrap());
        let ellipse = Curve::Ellipse(Ellipse::new(Frame::WORLD, [2.0, 1.0]).unwrap());
        let nurbs = Curve::Nurbs(Arc::new(
            NurbsCurve::new(
                1,
                vec![0.0, 0.0, 1.0, 1.0],
                vec![[0.0, 0.0, 0.0], [1.0, 2.0, 3.0]],
                None,
            )
            .unwrap(),
        ));
        for c in [&line, &circle, &ellipse, &nurbs] {
            let moved = curve(c, &at).unwrap();
            for t in [0.0, 0.25, 0.75] {
                same(moved.point(t), at.apply_point(c.point(t)));
            }
        }
        let profile = Arc::new(circle.clone());
        let plane = Surface::Plane(Plane::new(Frame::WORLD));
        let surfaces = [
            plane.clone(),
            Surface::Cylinder(Cylinder::new(Frame::WORLD, 1.5).unwrap()),
            Surface::Cone(Cone::new(Frame::WORLD, 1.0, 0.5).unwrap()),
            Surface::Sphere(Sphere::new(Frame::WORLD, 2.0).unwrap()),
            Surface::Torus(Torus::new(Frame::WORLD, 3.0, 1.0).unwrap()),
            Surface::Extrusion(Extrusion::new(profile.clone(), Vec3::Z).unwrap()),
            Surface::Revolution(
                Revolution::new(Arc::new(line.clone()), Vec3::ZERO, Vec3::Z).unwrap(),
            ),
            Surface::Offset(Offset::new(Arc::new(plane), 0.5).unwrap()),
        ];
        for s in &surfaces {
            let moved = surface(s, &at).unwrap();
            for uv in [[0.0, 0.0], [0.5, 0.25], [1.0, 0.75]] {
                same(
                    moved.point(uv).unwrap(),
                    at.apply_point(s.point(uv).unwrap()),
                );
            }
        }
    }

    #[test]
    fn what_scales_or_mirrors_is_refused() {
        let empty = Brep::empty();
        let scaled = Xform::from_axes(Vec3::X * 2.0, Vec3::Y, Vec3::Z, Vec3::ZERO);
        let mirrored = Xform::from_axes(-Vec3::X, Vec3::Y, Vec3::Z, Vec3::ZERO);
        assert!(gather_breps(&[(&empty, scaled)]).is_err());
        assert!(gather_breps(&[(&empty, mirrored)]).is_err());
    }

    #[test]
    fn one_value_at_the_identity_is_that_value() {
        let empty = Brep::empty();
        assert_eq!(gather_breps(&[(&empty, Xform::IDENTITY)]).unwrap(), empty);
    }
}
