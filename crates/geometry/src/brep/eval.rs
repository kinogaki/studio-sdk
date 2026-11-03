//! Evaluation: the point and first derivatives of every carrier, which is
//! how the contract says what a value means ("the vertex is at the
//! curve's `t0`", "material on the left"). Nothing beyond that: no
//! closest points, intersections or bounds.
use super::geometry::{Curve, PCurve, Surface};
use crate::vector::{add, cross, dot, normalize_or_zero, scale, sub};
use std::f64::consts::TAU;

type V3 = [f64; 3];

fn along(frame_x: V3, frame_y: V3, c: f64, s: f64) -> V3 {
    add(scale(frame_x, c), scale(frame_y, s))
}

impl Surface {
    /// The point at `uv`; `None` only for an offset where its base has no
    /// normal (a pole, a collapsed side).
    pub fn point(&self, uv: [f64; 2]) -> Option<[f64; 3]> {
        match self {
            Surface::Offset(off) => {
                let p = off.base().point(uv)?;
                Some(add(p, scale(off.base().normal(uv)?, off.distance())))
            }
            _ => Some(self.partials(uv)?[0]),
        }
    }

    /// The point and first partial derivatives `[S, ∂S/∂u, ∂S/∂v]` at
    /// `uv` (one-sided from above at a NURBS knot); `None` where
    /// [`point`](Self::point) is. An offset's are `S + d·N` and
    /// `∂S + d·∂N`, exact over a plane, cylinder, cone, sphere or torus,
    /// whose normals' derivatives have closed forms; over another base
    /// (NURBS, an extrusion, a revolution) they need its second
    /// derivatives, which nothing here evaluates: `None`, and a kernel
    /// evaluates such an offset itself.
    pub(crate) fn partials(&self, uv: [f64; 2]) -> Option<[[f64; 3]; 3]> {
        let [u, v] = uv;
        Some(match self {
            Surface::Plane(p) => {
                let f = p.frame();
                [add(f.origin(), along(f.x(), f.y(), u, v)), f.x(), f.y()]
            }
            Surface::Cylinder(c) => {
                let (f, r) = (c.frame(), c.radius());
                let (s, co) = u.sin_cos();
                let ring = along(f.x(), f.y(), co, s);
                [
                    add(add(f.origin(), scale(ring, r)), scale(f.z(), v)),
                    scale(along(f.x(), f.y(), -s, co), r),
                    f.z(),
                ]
            }
            Surface::Cone(c) => {
                let f = c.frame();
                let slope = c.half_angle().tan();
                let rho = c.radius() + v * slope;
                let (s, co) = u.sin_cos();
                let ring = along(f.x(), f.y(), co, s);
                [
                    add(add(f.origin(), scale(ring, rho)), scale(f.z(), v)),
                    scale(along(f.x(), f.y(), -s, co), rho),
                    add(scale(ring, slope), f.z()),
                ]
            }
            Surface::Sphere(sp) => {
                let (f, r) = (sp.frame(), sp.radius());
                let (s, co) = u.sin_cos();
                let (sp_, cp) = v.sin_cos();
                let ring = along(f.x(), f.y(), co, s);
                [
                    add(f.origin(), add(scale(ring, r * cp), scale(f.z(), r * sp_))),
                    scale(along(f.x(), f.y(), -s, co), r * cp),
                    add(scale(ring, -r * sp_), scale(f.z(), r * cp)),
                ]
            }
            Surface::Torus(t) => {
                let (f, big, r) = (t.frame(), t.major(), t.minor());
                let (s, co) = u.sin_cos();
                let (sp, cp) = v.sin_cos();
                let ring = along(f.x(), f.y(), co, s);
                [
                    add(
                        f.origin(),
                        add(scale(ring, big + r * cp), scale(f.z(), r * sp)),
                    ),
                    scale(along(f.x(), f.y(), -s, co), big + r * cp),
                    add(scale(ring, -r * sp), scale(f.z(), r * cp)),
                ]
            }
            Surface::Nurbs(n) => n.jet(uv),
            Surface::Extrusion(e) => {
                let [p, d] = e.profile().jet(u);
                [add(p, scale(e.direction(), v)), d, e.direction()]
            }
            Surface::Revolution(r) => {
                // o + q∥ + cos θ·q⊥ + sin θ·(a × q⊥), q = C(t) − o.
                let (o, a) = (r.origin(), r.axis());
                let [p, d] = r.profile().jet(v);
                let (s, co) = u.sin_cos();
                let turn = |q: V3, with_axial: bool| {
                    let axial = scale(a, dot(q, a));
                    let radial = sub(q, axial);
                    let turned = cross(a, radial);
                    let round = add(scale(radial, co), scale(turned, s));
                    let d_round = add(scale(radial, -s), scale(turned, co));
                    (if with_axial { add(round, axial) } else { round }, d_round)
                };
                let (point, d_theta) = turn(sub(p, o), true);
                let (d_t, _) = turn(d, true);
                [add(o, point), d_theta, d_t]
            }
            Surface::Offset(off) => {
                let (base, d) = (off.base(), off.distance());
                let [p, du, dv] = base.partials(uv)?;
                let [nu, nv] = base.normal_partials(uv)?;
                let n = base.normal(uv)?;
                [
                    add(p, scale(n, d)),
                    add(du, scale(nu, d)),
                    add(dv, scale(nv, d)),
                ]
            }
        })
    }

    /// The derivatives `[∂N/∂u, ∂N/∂v]` of the unit [`normal`](Self::normal)
    /// at `uv`, for the kinds whose normal has a closed form (plane,
    /// cylinder, cone, sphere, torus); `None` for the others.
    fn normal_partials(&self, uv: [f64; 2]) -> Option<[V3; 2]> {
        let [u, v] = uv;
        let (s, co) = u.sin_cos();
        // The ring's derivative in u: the frame's tangent of the turn.
        let tangent = |x: V3, y: V3| along(x, y, -s, co);
        Some(match self {
            Surface::Plane(_) => [[0.0; 3]; 2],
            Surface::Cylinder(c) => [tangent(c.frame().x(), c.frame().y()), [0.0; 3]],
            Surface::Cone(c) => {
                let f = c.frame();
                [scale(tangent(f.x(), f.y()), c.half_angle().cos()), [0.0; 3]]
            }
            Surface::Sphere(sp) => {
                let f = sp.frame();
                let (sv, cv) = v.sin_cos();
                let ring = along(f.x(), f.y(), co, s);
                [
                    scale(tangent(f.x(), f.y()), cv),
                    add(scale(ring, -sv), scale(f.z(), cv)),
                ]
            }
            Surface::Torus(t) => {
                let f = t.frame();
                let (sv, cv) = v.sin_cos();
                let ring = along(f.x(), f.y(), co, s);
                let side = if t.major() + t.minor() * cv < 0.0 {
                    -1.0
                } else {
                    1.0
                };
                [
                    scale(tangent(f.x(), f.y()), side * cv),
                    scale(add(scale(ring, -sv), scale(f.z(), cv)), side),
                ]
            }
            _ => return None,
        })
    }

    /// The unit normal at `uv`: `∂u × ∂v` normalized. The analytic kinds
    /// use their closed forms, defined wherever their parameters are (at a
    /// sphere's poles, along a cone's apex): a cylinder's, cone's,
    /// sphere's and torus's point away from the axis or centre (a
    /// degenerate torus's turns inside out where `R + r cos φ < 0`). An
    /// offset's is its base's. `None` where the partials are parallel or
    /// undefined.
    pub fn normal(&self, uv: [f64; 2]) -> Option<[f64; 3]> {
        let [u, v] = uv;
        match self {
            Surface::Plane(p) => Some(p.frame().z()),
            Surface::Cylinder(c) => {
                let f = c.frame();
                let (s, co) = u.sin_cos();
                Some(along(f.x(), f.y(), co, s))
            }
            Surface::Cone(c) => {
                let f = c.frame();
                let (sa, ca) = c.half_angle().sin_cos();
                let (s, co) = u.sin_cos();
                Some(sub(scale(along(f.x(), f.y(), co, s), ca), scale(f.z(), sa)))
            }
            Surface::Sphere(sp) => {
                let f = sp.frame();
                let (s, co) = u.sin_cos();
                let (sv, cv) = v.sin_cos();
                Some(add(scale(along(f.x(), f.y(), co, s), cv), scale(f.z(), sv)))
            }
            Surface::Torus(t) => {
                let f = t.frame();
                let (s, co) = u.sin_cos();
                let (sv, cv) = v.sin_cos();
                let n = add(scale(along(f.x(), f.y(), co, s), cv), scale(f.z(), sv));
                Some(if t.major() + t.minor() * cv < 0.0 {
                    scale(n, -1.0)
                } else {
                    n
                })
            }
            Surface::Offset(off) => off.base().normal(uv),
            _ => {
                let [_, du, dv] = self.partials(uv)?;
                let n = normalize_or_zero(cross(du, dv));
                (n != [0.0; 3]).then_some(n)
            }
        }
    }

    /// The period of each parameter `[u, v]`: 2π in θ for the cylinder,
    /// cone, sphere and revolution, and in both for the torus; an
    /// extrusion's profile's period in `t`; for NURBS, the domain's width
    /// in each closed direction; an offset's base's. `None`: open.
    pub(crate) fn periods(&self) -> [Option<f64>; 2] {
        let turn = Some(TAU);
        match self {
            Surface::Plane(_) => [None, None],
            Surface::Cylinder(_) | Surface::Cone(_) | Surface::Sphere(_) => [turn, None],
            Surface::Revolution(_) => [turn, None],
            Surface::Torus(_) => [turn, turn],
            Surface::Extrusion(e) => [e.profile().period(), None],
            Surface::Nurbs(n) => n.periods(),
            Surface::Offset(o) => o.base().periods(),
        }
    }
}

impl Curve {
    /// The point at `t`.
    pub fn point(&self, t: f64) -> [f64; 3] {
        self.jet(t)[0]
    }

    pub(crate) fn jet(&self, t: f64) -> [[f64; 3]; 2] {
        match self {
            Curve::Line(l) => [add(l.origin(), scale(l.direction(), t)), l.direction()],
            Curve::Circle(c) => {
                let (f, r) = (c.frame(), c.radius());
                let (s, co) = t.sin_cos();
                [
                    add(f.origin(), scale(along(f.x(), f.y(), co, s), r)),
                    scale(along(f.x(), f.y(), -s, co), r),
                ]
            }
            Curve::Ellipse(e) => {
                let (f, [a, b]) = (e.frame(), e.semi_axes());
                let (s, co) = t.sin_cos();
                [
                    add(f.origin(), along(f.x(), f.y(), a * co, b * s)),
                    along(f.x(), f.y(), -a * s, b * co),
                ]
            }
            Curve::Nurbs(n) => n.jet(t),
        }
    }
}

impl PCurve {
    /// The chart point at `t`.
    pub fn point(&self, t: f64) -> [f64; 2] {
        self.jet(t)[0]
    }

    pub(crate) fn jet(&self, t: f64) -> [[f64; 2]; 2] {
        let mix =
            |x: [f64; 2], y: [f64; 2], a: f64, b: f64| [x[0] * a + y[0] * b, x[1] * a + y[1] * b];
        let plus = |p: [f64; 2], q: [f64; 2]| [p[0] + q[0], p[1] + q[1]];
        match self {
            PCurve::Line(l) => {
                let (o, d) = (l.origin(), l.direction());
                [[o[0] + t * d[0], o[1] + t * d[1]], d]
            }
            PCurve::Circle(c) => {
                let ((x, y), r) = (c.axes(), c.radius());
                let (s, co) = t.sin_cos();
                [
                    plus(c.center(), mix(x, y, r * co, r * s)),
                    mix(x, y, -r * s, r * co),
                ]
            }
            PCurve::Ellipse(e) => {
                let ((x, y), [a, b]) = (e.axes(), e.semi_axes());
                let (s, co) = t.sin_cos();
                [
                    plus(e.center(), mix(x, y, a * co, b * s)),
                    mix(x, y, -a * s, b * co),
                ]
            }
            PCurve::Nurbs(n) => n.jet(t),
        }
    }
}
