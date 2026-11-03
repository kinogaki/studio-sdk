//! A body's carrier tables (surfaces, curves, p-curves) as one pre-order
//! stream of records: each record a tag (`carrier_tags`), the reals it
//! holds (`carrier_reals`) and its counts (`carrier_counts`), each stream
//! read in order. A carrier holding another (an extrusion's profile, an
//! offset's base) is followed by it.
use super::super::invalid;
use crate::Error;
use crate::element::{ElementReader, ElementWriter, Field};
use kinogaki_geometry::Column;
use kinogaki_geometry::brep::{
    Circle, Circle2, Cone, Curve, Cylinder, Ellipse, Ellipse2, Extrusion, Line, Line2, NurbsCurve,
    NurbsSurface, Offset, PCurve, Plane, Revolution, SeamContinuity, Sphere, Surface, Torus,
};
use kinogaki_geometry::{Frame, Vec3};
use std::sync::Arc;

const PLANE: u8 = 1;
const CYLINDER: u8 = 2;
const CONE: u8 = 3;
const SPHERE: u8 = 4;
const TORUS: u8 = 5;
const NURBS_SURFACE: u8 = 6;
const EXTRUSION: u8 = 7;
const REVOLUTION: u8 = 8;
const OFFSET: u8 = 9;
const LINE: u8 = 20;
const CIRCLE: u8 = 21;
const ELLIPSE: u8 = 22;
const NURBS_CURVE: u8 = 23;
const LINE2: u8 = 30;
const CIRCLE2: u8 = 31;
const ELLIPSE2: u8 = 32;
const NURBS_CURVE2: u8 = 33;

/// The record streams being written.
#[derive(Default)]
pub(super) struct Writer {
    tags: Vec<u8>,
    reals: Vec<f64>,
    counts: Vec<u32>,
}

/// A carrier kind this version can't save.
fn unknown(what: &str) -> Error {
    Error::invalid(format_args!("a {what} of a kind this version can't save"))
}

fn count(n: usize) -> u32 {
    // Tables and NURBS sizes are far below u32's range (MAX_ENTITIES).
    u32::try_from(n).unwrap_or(u32::MAX)
}

impl Writer {
    fn record(&mut self, tag: u8, reals: &[f64]) {
        self.tags.push(tag);
        self.reals.extend_from_slice(reals);
    }

    fn frame(&mut self, f: &Frame) {
        for v in [f.origin(), f.x(), f.y(), f.z()] {
            self.reals.extend_from_slice(&v.to_array());
        }
    }

    fn nurbs<const D: usize>(&mut self, n: &NurbsCurve<D>) {
        let weights = n.weights().map_or(&[][..], Column::as_slice);
        self.counts.extend([
            u32::from(n.degree()),
            count(n.knots().len()),
            count(n.points().len()),
            u32::from(n.weights().is_some()),
        ]);
        self.reals.extend_from_slice(n.knots().as_slice());
        self.reals.extend(n.points().iter().flatten());
        self.reals.extend_from_slice(weights);
    }

    pub(super) fn surface(&mut self, s: &Surface) -> Result<(), Error> {
        match s {
            Surface::Plane(p) => {
                self.record(PLANE, &[]);
                self.frame(p.frame());
            }
            Surface::Cylinder(c) => {
                self.record(CYLINDER, &[]);
                self.frame(c.frame());
                self.reals.push(c.radius());
            }
            Surface::Cone(c) => {
                self.record(CONE, &[]);
                self.frame(c.frame());
                self.reals.extend([c.radius(), c.half_angle()]);
            }
            Surface::Sphere(c) => {
                self.record(SPHERE, &[]);
                self.frame(c.frame());
                self.reals.push(c.radius());
            }
            Surface::Torus(t) => {
                self.record(TORUS, &[]);
                self.frame(t.frame());
                self.reals.extend([t.major(), t.minor()]);
            }
            Surface::Nurbs(n) => {
                self.record(NURBS_SURFACE, &[]);
                let [du, dv] = n.degrees();
                let [ku, kv] = n.knots();
                let closed = n.closed().map(|c| match c {
                    None => Ok(0),
                    Some(SeamContinuity::Position) => Ok(1),
                    Some(SeamContinuity::Tangent) => Ok(2),
                    Some(SeamContinuity::Parametric) => Ok(3),
                    Some(_) => Err(unknown("seam")),
                });
                let [cu, cv] = closed;
                self.counts.extend([
                    u32::from(du),
                    u32::from(dv),
                    count(ku.len()),
                    count(kv.len()),
                    count(n.points().len()),
                    u32::from(n.weights().is_some()),
                    cu?,
                    cv?,
                ]);
                self.reals.extend_from_slice(ku.as_slice());
                self.reals.extend_from_slice(kv.as_slice());
                self.reals.extend(n.points().iter().flatten());
                self.reals
                    .extend_from_slice(n.weights().map_or(&[][..], Column::as_slice));
            }
            Surface::Extrusion(e) => {
                self.record(EXTRUSION, &e.direction().to_array());
                self.curve(e.profile())?;
            }
            Surface::Revolution(r) => {
                self.record(REVOLUTION, &r.origin().to_array());
                self.reals.extend(r.axis().to_array());
                self.curve(r.profile())?;
            }
            Surface::Offset(o) => {
                self.record(OFFSET, &[o.distance()]);
                self.surface(o.base())?;
            }
            _ => return Err(unknown("surface")),
        }
        Ok(())
    }

    pub(super) fn curve(&mut self, c: &Curve) -> Result<(), Error> {
        match c {
            Curve::Line(l) => {
                self.record(LINE, &l.origin().to_array());
                self.reals.extend(l.direction().to_array());
            }
            Curve::Circle(c) => {
                self.record(CIRCLE, &[]);
                self.frame(c.frame());
                self.reals.push(c.radius());
            }
            Curve::Ellipse(e) => {
                self.record(ELLIPSE, &[]);
                self.frame(e.frame());
                self.reals.extend(e.semi_axes());
            }
            Curve::Nurbs(n) => {
                self.record(NURBS_CURVE, &[]);
                self.nurbs(n);
            }
            _ => return Err(unknown("curve")),
        }
        Ok(())
    }

    pub(super) fn pcurve(&mut self, c: &PCurve) -> Result<(), Error> {
        match c {
            PCurve::Line(l) => {
                self.record(LINE2, &l.origin());
                self.reals.extend(l.direction());
            }
            PCurve::Circle(c) => {
                self.record(CIRCLE2, &c.center());
                self.reals.extend(c.x());
                self.reals.push(c.radius());
                self.counts.push(u32::from(c.counter_clockwise()));
            }
            PCurve::Ellipse(e) => {
                self.record(ELLIPSE2, &e.center());
                self.reals.extend(e.x());
                self.reals.extend(e.semi_axes());
                self.counts.push(u32::from(e.counter_clockwise()));
            }
            PCurve::Nurbs(n) => {
                self.record(NURBS_CURVE2, &[]);
                self.nurbs(n);
            }
            _ => return Err(unknown("p-curve")),
        }
        Ok(())
    }

    pub(super) fn finish(self, out: &mut dyn ElementWriter) -> Result<(), Error> {
        out.set("carrierTags", Field::U8s(self.tags))?;
        out.set("carrierReals", Field::F64s(self.reals))?;
        out.set("carrierCounts", Field::U32s(self.counts))
    }
}

/// The record streams being read, each from its cursor.
pub(super) struct Reader<'a> {
    at: String,
    tags: &'a [u8],
    reals: &'a [f64],
    counts: &'a [u32],
}

impl<'a> Reader<'a> {
    pub(super) fn new(element: &'a dyn ElementReader) -> Result<Self, Error> {
        Ok(Self {
            at: element.location(),
            tags: super::super::attributes::u8s(element, "carrierTags")?,
            reals: element.f64s("carrierReals")?,
            counts: element.u32s("carrierCounts")?,
        })
    }

    fn short(&self) -> Error {
        Error::invalid(format_args!("{}: the carrier records end early", self.at))
    }

    /// Whether every stream was read to its end.
    pub(super) fn done(&self) -> Result<(), Error> {
        if self.tags.is_empty() && self.reals.is_empty() && self.counts.is_empty() {
            Ok(())
        } else {
            Err(Error::invalid(format_args!(
                "{}: the carrier records run on",
                self.at
            )))
        }
    }

    fn tag(&mut self) -> Result<u8, Error> {
        let (&t, rest) = self.tags.split_first().ok_or_else(|| self.short())?;
        self.tags = rest;
        Ok(t)
    }

    fn take(&mut self, n: usize) -> Result<&'a [f64], Error> {
        if n > self.reals.len() {
            return Err(self.short());
        }
        let (head, rest) = self.reals.split_at(n);
        self.reals = rest;
        Ok(head)
    }

    fn reals<const N: usize>(&mut self) -> Result<[f64; N], Error> {
        let mut out = [0.0; N];
        out.copy_from_slice(self.take(N)?);
        Ok(out)
    }

    fn real(&mut self) -> Result<f64, Error> {
        Ok(self.reals::<1>()?[0])
    }

    fn counts<const N: usize>(&mut self) -> Result<[u32; N], Error> {
        if N > self.counts.len() {
            return Err(self.short());
        }
        let (head, rest) = self.counts.split_at(N);
        self.counts = rest;
        let mut out = [0; N];
        out.copy_from_slice(head);
        Ok(out)
    }

    fn bit(&self, v: u32) -> Result<bool, Error> {
        match v {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(Error::invalid(format_args!(
                "{}: a flag not 0 or 1",
                self.at
            ))),
        }
    }

    fn flag(&mut self) -> Result<bool, Error> {
        let [v] = self.counts::<1>()?;
        self.bit(v)
    }

    fn checked<T>(&self, r: Result<T, kinogaki_geometry::Error>) -> Result<T, Error> {
        r.map_err(|e| invalid(&self.at, e))
    }

    fn frame(&mut self) -> Result<Frame, Error> {
        let [o0, o1, o2, x0, x1, x2, y0, y1, y2, z0, z1, z2] = self.reals::<12>()?;
        self.checked(Frame::new(
            Vec3::new(o0, o1, o2),
            Vec3::new(x0, x1, x2),
            Vec3::new(y0, y1, y2),
            Vec3::new(z0, z1, z2),
        ))
    }

    fn degree(&self, d: u32) -> Result<u8, Error> {
        u8::try_from(d).map_err(|_| Error::invalid(format_args!("{}: a degree too high", self.at)))
    }

    fn points<const D: usize>(&mut self, n: usize) -> Result<Vec<[f64; D]>, Error> {
        let flat = self.take(n.checked_mul(D).ok_or_else(|| self.short())?)?;
        Ok(flat.as_chunks::<D>().0.to_vec())
    }

    fn weights(&mut self, rational: bool, n: usize) -> Result<Option<Column<f64>>, Error> {
        rational
            .then(|| self.take(n).map(|w| w.to_vec().into()))
            .transpose()
    }

    fn nurbs<const D: usize>(&mut self) -> Result<NurbsCurve<D>, Error> {
        let [degree, knots, points, rational] = self.counts::<4>()?;
        let degree = self.degree(degree)?;
        let rational = self.bit(rational)?;
        let knots = self.take(knots as usize)?.to_vec();
        let points = self.points::<D>(points as usize)?;
        let weights = self.weights(rational, points.len())?;
        self.checked(NurbsCurve::new(degree, knots, points, weights))
    }

    pub(super) fn surface(&mut self) -> Result<Surface, Error> {
        Ok(match self.tag()? {
            PLANE => Surface::Plane(Plane::new(self.frame()?)),
            CYLINDER => {
                let frame = self.frame()?;
                let r = self.real()?;
                Surface::Cylinder(self.checked(Cylinder::new(frame, r))?)
            }
            CONE => {
                let frame = self.frame()?;
                let [r, a] = self.reals()?;
                Surface::Cone(self.checked(Cone::new(frame, r, a))?)
            }
            SPHERE => {
                let frame = self.frame()?;
                let r = self.real()?;
                Surface::Sphere(self.checked(Sphere::new(frame, r))?)
            }
            TORUS => {
                let frame = self.frame()?;
                let [major, minor] = self.reals()?;
                Surface::Torus(self.checked(Torus::new(frame, major, minor))?)
            }
            NURBS_SURFACE => {
                let [du, dv, ku, kv, points, rational, cu, cv] = self.counts::<8>()?;
                let degrees = [self.degree(du)?, self.degree(dv)?];
                let rational = self.bit(rational)?;
                let closed = [cu, cv].map(|c| match c {
                    0 => Ok(None),
                    1 => Ok(Some(SeamContinuity::Position)),
                    2 => Ok(Some(SeamContinuity::Tangent)),
                    3 => Ok(Some(SeamContinuity::Parametric)),
                    _ => Err(Error::invalid(format_args!(
                        "{}: no seam kind {c}",
                        self.at
                    ))),
                });
                let [cu, cv] = closed;
                let knots = [
                    Column::from(self.take(ku as usize)?.to_vec()),
                    Column::from(self.take(kv as usize)?.to_vec()),
                ];
                let points = self.points::<3>(points as usize)?;
                let weights = self.weights(rational, points.len())?;
                let s = NurbsSurface::new(degrees, knots, points, weights, [cu?, cv?]);
                Surface::Nurbs(Arc::new(self.checked(s)?))
            }
            EXTRUSION => {
                let direction = Vec3::from(self.reals::<3>()?);
                let profile = Arc::new(self.curve()?);
                Surface::Extrusion(self.checked(Extrusion::new(profile, direction))?)
            }
            REVOLUTION => {
                let [o0, o1, o2, a0, a1, a2] = self.reals()?;
                let profile = Arc::new(self.curve()?);
                let r = Revolution::new(profile, Vec3::new(o0, o1, o2), Vec3::new(a0, a1, a2));
                Surface::Revolution(self.checked(r)?)
            }
            OFFSET => {
                let distance = self.real()?;
                let base = Arc::new(self.surface()?);
                Surface::Offset(self.checked(Offset::new(base, distance))?)
            }
            t => {
                return Err(Error::invalid(format_args!(
                    "{}: no surface kind {t}",
                    self.at
                )));
            }
        })
    }

    pub(super) fn curve(&mut self) -> Result<Curve, Error> {
        Ok(match self.tag()? {
            LINE => {
                let [o0, o1, o2, d0, d1, d2] = self.reals()?;
                Curve::Line(self.checked(Line::new(Vec3::new(o0, o1, o2), Vec3::new(d0, d1, d2)))?)
            }
            CIRCLE => {
                let frame = self.frame()?;
                let r = self.real()?;
                Curve::Circle(self.checked(Circle::new(frame, r))?)
            }
            ELLIPSE => {
                let frame = self.frame()?;
                let axes = self.reals()?;
                Curve::Ellipse(self.checked(Ellipse::new(frame, axes))?)
            }
            NURBS_CURVE => Curve::Nurbs(Arc::new(self.nurbs()?)),
            t => {
                return Err(Error::invalid(format_args!(
                    "{}: no curve kind {t}",
                    self.at
                )));
            }
        })
    }

    pub(super) fn pcurve(&mut self) -> Result<PCurve, Error> {
        Ok(match self.tag()? {
            LINE2 => {
                let [o0, o1, d0, d1] = self.reals()?;
                PCurve::Line(self.checked(Line2::new([o0, o1], [d0, d1]))?)
            }
            CIRCLE2 => {
                let [c0, c1, x0, x1, r] = self.reals()?;
                let ccw = self.flag()?;
                PCurve::Circle(self.checked(Circle2::new([c0, c1], [x0, x1], r, ccw))?)
            }
            ELLIPSE2 => {
                let [c0, c1, x0, x1, a, b] = self.reals()?;
                let ccw = self.flag()?;
                let e = Ellipse2::new([c0, c1], [x0, x1], [a, b], ccw);
                PCurve::Ellipse(self.checked(e)?)
            }
            NURBS_CURVE2 => PCurve::Nurbs(Arc::new(self.nurbs()?)),
            t => {
                return Err(Error::invalid(format_args!(
                    "{}: no p-curve kind {t}",
                    self.at
                )));
            }
        })
    }
}
