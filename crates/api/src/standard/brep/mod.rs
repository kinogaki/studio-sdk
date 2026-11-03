//! [`BrepCodec`]: how [`Brep`] values save.
mod carriers;
mod names;

use super::geometry::{read_geometry, write_geometry};
use super::{attributes, brep, invalid, selection};
use crate::Error;
use crate::codec::DataCodec;
use crate::components::Components;
use crate::data::Data;
use crate::element::{ElementReader, ElementWriter, Field};
use kinogaki_geometry::brep::{
    Body, Brep, BrepBuilder, CoedgeSpec, Datum, DatumHint, DatumKind, EdgeSpec, FaceSpec, Sense,
};
use kinogaki_geometry::{Frame, Vec3};

/// Saves [`Brep`]: one table of every persistent name the value holds
/// (`name_tags`, `name_words`, `name_texts`), which everything else names
/// by index; the datums (`datum_names`,
/// `datum_kinds`, `datum_reals`, `datum_hints`); the free curves as the
/// child `curves` (a geometry value, as
/// [`GeometryCodec`](super::GeometryCodec) saves one); one
/// child `body-<i>` (kind `body`) per body; and the Detail attributes.
///
/// A body (at its own origin: an assembly places bodies as
/// [`Instances`](super::Instances)) holds its `name`, `tolerance`, its
/// vertices' `points`, its carriers as one record
/// stream (`surface_count`, `curve_count`, `pcurve_count` and the
/// `carrier_*` fields), its topology as index columns (edges, faces with
/// their loops' coedges in order, shells and lumps), the entities' names,
/// the precisions of the entities that have one (`*_precise` and
/// `*_precision`), and its attributes. `u32::MAX` stands for no carrier.
///
/// Reading rebuilds every body through [`BrepBuilder`], so it checks every
/// rule again, and the ids come back as they were. A round trip is
/// bit-exact. Components save as the standard
/// [`Selection`](kinogaki_geometry::Selection), numbered as
/// [`Brep::count`] numbers the value's elements.
///
/// ```
/// use kinogaki_studio_api::DataCodec;
/// use kinogaki_studio_api::element::MemoryElement;
/// use kinogaki_studio_api::geometry::brep::Brep;
/// use kinogaki_studio_api::standard::{self, BrepCodec};
///
/// let data = standard::brep().wrap(Brep::empty());
/// let mut saved = MemoryElement::new("result");
/// saved.write(|w| BrepCodec.write(&data, None, w)).unwrap();
/// let read = saved.read(|r| BrepCodec.read(r, None)).unwrap();
/// assert_eq!(standard::brep().get(&read), Some(&Brep::empty()));
/// ```
#[derive(Clone, Copy, Debug, Default)]
pub struct BrepCodec;

/// No carrier (a pole edge's curve, a coedge without a p-curve).
const NONE: u32 = u32::MAX;

impl DataCodec for BrepCodec {
    fn write(
        &self,
        data: &Data,
        _base: Option<&Data>,
        out: &mut dyn ElementWriter,
    ) -> Result<(), Error> {
        let value = brep()
            .get(data)
            .ok_or_else(|| Error::invalid("not a B-rep"))?;
        let mut names = names::Writer::default();
        for (i, body) in value.bodies().iter().enumerate() {
            let mut child = out.child(&format!("body-{i}"), "body")?;
            write_body(body, &mut names, &mut *child)?;
        }
        write_datums(value.datums(), &mut names, out)?;
        write_geometry(value.curves(), &mut *out.child("curves", "curves")?)?;
        attributes::write(value.detail(), out)?;
        names.finish(out)
    }

    fn read(&self, element: &dyn ElementReader, _base: Option<&Data>) -> Result<Data, Error> {
        let at = element.location();
        let names = names::Table::read(element)?;
        let mut bodies = Vec::new();
        while let Some(child) = element.child(&format!("body-{}", bodies.len())) {
            bodies.push(read_body(&*child, &names)?);
        }
        let datums = read_datums(element, &names)?;
        let curves = read_geometry(&*element.required_child("curves")?)?;
        let detail = attributes::read(element)?;
        let value = Brep::new(bodies, curves, datums, detail);
        Ok(brep().wrap(value.map_err(|e| invalid(&at, e))?))
    }

    fn write_components(
        &self,
        components: &Components,
        _data: &Data,
        out: &mut dyn ElementWriter,
    ) -> Result<bool, Error> {
        selection::write(components, out)
    }

    fn read_components(
        &self,
        element: &dyn ElementReader,
        data: &Data,
    ) -> Result<Components, Error> {
        let value = brep()
            .get(data)
            .ok_or_else(|| Error::invalid("not a B-rep"))?;
        selection::read(element, data.ty(), |domain| value.count(domain))
    }
}

fn sense(s: Sense) -> u8 {
    u8::from(s == Sense::Reversed)
}

fn read_sense(at: &str, s: u8) -> Result<Sense, Error> {
    match s {
        0 => Ok(Sense::Same),
        1 => Ok(Sense::Reversed),
        _ => Err(Error::invalid(format_args!("{at}: no sense {s}"))),
    }
}

/// `offsets` (each run's start, then the end) over `items`, as runs.
fn runs<'a>(at: &str, offsets: &[u32], items: &'a [u32]) -> Result<Vec<&'a [u32]>, Error> {
    let bad = || Error::invalid(format_args!("{at}: runs out of order or range"));
    if offsets
        .last()
        .map_or(!items.is_empty(), |&e| e as usize != items.len())
    {
        return Err(bad());
    }
    (offsets.windows(2))
        .map(|w| items.get(w[0] as usize..w[1] as usize).ok_or_else(bad))
        .collect()
}

/// Run offsets over `lengths`, starting at 0.
fn offsets(lengths: impl IntoIterator<Item = usize>) -> Vec<u32> {
    let mut at = 0u32;
    let mut out = vec![0];
    for n in lengths {
        at += n as u32;
        out.push(at);
    }
    out
}

/// Writes the indices of the entities with a precision and their
/// precisions.
fn write_precisions(
    out: &mut dyn ElementWriter,
    what: &str,
    precisions: impl Iterator<Item = Option<f64>>,
) -> Result<(), Error> {
    let (mut precise, mut values) = (Vec::new(), Vec::new());
    for (i, p) in precisions.enumerate() {
        if let Some(p) = p {
            precise.push(i as u32);
            values.push(p);
        }
    }
    out.set(&format!("{what}_precise"), Field::U32s(precise))?;
    out.set(&format!("{what}_precision"), Field::F64s(values))
}

/// Each of `n` entities' precision, from what [`write_precisions`] wrote.
fn read_precisions(
    element: &dyn ElementReader,
    what: &str,
    n: usize,
) -> Result<Vec<Option<f64>>, Error> {
    let at = element.location();
    let precise = element.u32s(&format!("{what}_precise"))?;
    let values = element.f64s(&format!("{what}_precision"))?;
    if precise.len() != values.len() {
        return Err(Error::invalid(format_args!(
            "{at}: {what} precisions missized"
        )));
    }
    let mut out = vec![None; n];
    for (&i, &p) in precise.iter().zip(values) {
        *out.get_mut(i as usize)
            .ok_or_else(|| Error::invalid(format_args!("{at}: no {what} {i}")))? = Some(p);
    }
    Ok(out)
}

fn index(at: &str, ids: &[impl Copy], i: u32) -> Result<(), Error> {
    if (i as usize) < ids.len() {
        Ok(())
    } else {
        Err(Error::invalid(format_args!("{at}: no entity {i}")))
    }
}

fn write_body(
    body: &Body,
    names: &mut names::Writer,
    out: &mut dyn ElementWriter,
) -> Result<(), Error> {
    let (g, t, n) = (body.geometry(), body.topology(), body.names());
    out.set("name", Field::Int(i64::from(names.name(body.name())?)))?;
    out.set("tolerance", Field::Float(body.tolerance()))?;
    // Carriers.
    out.set_rows("points", 3, Field::F64s(attributes::flat(g.points())))?;
    let mut carriers = carriers::Writer::default();
    for s in g.surfaces().iter() {
        carriers.surface(s)?;
    }
    for c in g.curves().iter() {
        carriers.curve(c)?;
    }
    for c in g.pcurves().iter() {
        carriers.pcurve(c)?;
    }
    out.set("surfaceCount", Field::Int(g.surfaces().len() as i64))?;
    out.set("curveCount", Field::Int(g.curves().len() as i64))?;
    out.set("pcurveCount", Field::Int(g.pcurves().len() as i64))?;
    carriers.finish(out)?;
    // Vertices and edges.
    out.set(
        "vertexNames",
        Field::U32s(names.names(n.vertices().iter())?),
    )?;
    let vertices = t.vertex_ids().map(|v| t.vertex(v).precision());
    write_precisions(out, "vertex", vertices)?;
    let edges: Vec<_> = t.edge_ids().map(|e| t.edge(e)).collect();
    let curve =
        |c: Option<_>| c.map_or(NONE, |c: kinogaki_geometry::brep::CurveId| c.index() as u32);
    out.set(
        "edgeCurves",
        Field::U32s(edges.iter().map(|e| curve(e.curve())).collect()),
    )?;
    let intervals = edges.iter().flat_map(|e| e.interval()).collect();
    out.set_rows("edgeIntervals", 2, Field::F64s(intervals))?;
    let ends = edges
        .iter()
        .flat_map(|e| e.vertices().map(|v| v.index() as u32))
        .collect();
    out.set_rows("edgeVertices", 2, Field::U32s(ends))?;
    out.set("edgeNames", Field::U32s(names.names(n.edges().iter())?))?;
    write_precisions(out, "edge", edges.iter().map(|e| e.precision()))?;
    // Faces, their loops and coedges, in order.
    let faces: Vec<_> = t.face_ids().collect();
    let surfaces = faces
        .iter()
        .map(|&f| t.face(f).surface().index() as u32)
        .collect();
    out.set("faceSurfaces", Field::U32s(surfaces))?;
    out.set(
        "faceSenses",
        Field::U8s(faces.iter().map(|&f| sense(t.face(f).sense())).collect()),
    )?;
    out.set("faceNames", Field::U32s(names.names(n.faces().iter())?))?;
    write_precisions(out, "face", faces.iter().map(|&f| t.face(f).precision()))?;
    let loops: Vec<_> = faces.iter().flat_map(|&f| t.face_loops(f)).collect();
    out.set(
        "faceLoops",
        Field::U32s(offsets(faces.iter().map(|&f| t.face_loops(f).len()))),
    )?;
    out.set(
        "loopCoedges",
        Field::U32s(offsets(loops.iter().map(|&l| t.loop_coedges(l).len()))),
    )?;
    let coedges: Vec<_> = loops
        .iter()
        .flat_map(|&l| t.loop_coedges(l))
        .map(|c| t.coedge(c))
        .collect();
    out.set(
        "coedgeEdges",
        Field::U32s(coedges.iter().map(|c| c.edge().index() as u32).collect()),
    )?;
    out.set(
        "coedgeSenses",
        Field::U8s(coedges.iter().map(|c| sense(c.sense())).collect()),
    )?;
    let pcurves = coedges
        .iter()
        .map(|c| c.pcurve().map_or(NONE, |p| p.index() as u32))
        .collect();
    out.set("coedgePcurves", Field::U32s(pcurves))?;
    write_precisions(out, "coedge", coedges.iter().map(|c| c.precision()))?;
    // Shells and lumps.
    let shells: Vec<_> = t.shell_ids().collect();
    out.set(
        "shellFaces",
        Field::U32s(offsets(shells.iter().map(|&s| t.shell_faces(s).len()))),
    )?;
    let shell_faces = shells
        .iter()
        .flat_map(|&s| t.shell_faces(s))
        .map(|f| f.index() as u32);
    out.set("shellFaceIds", Field::U32s(shell_faces.collect()))?;
    let lumps: Vec<_> = t.lump_ids().collect();
    out.set(
        "lumpShells",
        Field::U32s(offsets(lumps.iter().map(|&l| t.lump_shells(l).len()))),
    )?;
    let lump_shells = lumps
        .iter()
        .flat_map(|&l| t.lump_shells(l))
        .map(|s| s.index() as u32);
    out.set("lumpShellIds", Field::U32s(lump_shells.collect()))?;
    attributes::write(body.attributes(), out)
}

fn read_body(element: &dyn ElementReader, names: &names::Table) -> Result<Body, Error> {
    let at = element.location();
    let name = names.get(
        u32::try_from(element.int("name")?)
            .map_err(|_| Error::invalid(format_args!("{at}: no name")))?,
    )?;
    let mut b = BrepBuilder::new(name, element.float("tolerance")?);
    // Carriers, in table order.
    let mut carriers = carriers::Reader::new(element)?;
    let surfaces: Vec<_> = (0..attributes::count(element, "surfaceCount")?)
        .map(|_| carriers.surface().map(|s| b.surface(s)))
        .collect::<Result<_, _>>()?;
    let curves: Vec<_> = (0..attributes::count(element, "curveCount")?)
        .map(|_| carriers.curve().map(|c| b.curve(c)))
        .collect::<Result<_, _>>()?;
    let pcurves: Vec<_> = (0..attributes::count(element, "pcurveCount")?)
        .map(|_| carriers.pcurve().map(|c| b.pcurve(c)))
        .collect::<Result<_, _>>()?;
    carriers.done()?;
    // Vertices and edges.
    let points = attributes::rows(element, "points")?;
    let vertex_names = names.all(element.u32s("vertexNames")?)?;
    let precisions = read_precisions(element, "vertex", points.len())?;
    if vertex_names.len() != points.len() {
        return Err(Error::invalid(format_args!("{at}: vertex names missized")));
    }
    let vertices: Vec<_> = (points.iter().zip(precisions).zip(vertex_names))
        .map(|((&p, precision), name)| b.vertex(p.into(), precision, name))
        .collect();
    let edge_curves = element.u32s("edgeCurves")?;
    let intervals = element.f64s("edgeIntervals")?;
    let ends = element.u32s("edgeVertices")?;
    let edge_names = names.all(element.u32s("edgeNames")?)?;
    let n = edge_curves.len();
    if intervals.len() != 2 * n || ends.len() != 2 * n || edge_names.len() != n {
        return Err(Error::invalid(format_args!("{at}: edge columns missized")));
    }
    let precisions = read_precisions(element, "edge", n)?;
    let mut edges = Vec::with_capacity(n);
    for (i, (name, precision)) in edge_names.into_iter().zip(precisions).enumerate() {
        let curve = match edge_curves[i] {
            NONE => None,
            c => {
                index(&at, &curves, c)?;
                Some(curves[c as usize])
            }
        };
        let [v0, v1] = [ends[2 * i], ends[2 * i + 1]];
        index(&at, &vertices, v0)?;
        index(&at, &vertices, v1)?;
        let spec = EdgeSpec {
            curve,
            interval: [intervals[2 * i], intervals[2 * i + 1]],
            vertices: [vertices[v0 as usize], vertices[v1 as usize]],
            precision,
        };
        edges.push(b.edge(spec, name));
    }
    // Faces with their loops' coedges.
    let face_surfaces = element.u32s("faceSurfaces")?;
    let face_senses = attributes::u8s(element, "faceSenses")?;
    let face_names = names.all(element.u32s("faceNames")?)?;
    let nf = face_surfaces.len();
    if face_senses.len() != nf || face_names.len() != nf {
        return Err(Error::invalid(format_args!("{at}: face columns missized")));
    }
    let face_precisions = read_precisions(element, "face", nf)?;
    let coedge_edges = element.u32s("coedgeEdges")?;
    let coedge_senses = attributes::u8s(element, "coedgeSenses")?;
    let coedge_pcurves = element.u32s("coedgePcurves")?;
    let nc = coedge_edges.len();
    if coedge_senses.len() != nc || coedge_pcurves.len() != nc {
        return Err(Error::invalid(format_args!(
            "{at}: coedge columns missized"
        )));
    }
    let coedge_precisions = read_precisions(element, "coedge", nc)?;
    let ordinals: Vec<u32> = (0..nc as u32).collect();
    let loop_runs = runs(&at, element.u32s("loopCoedges")?, &ordinals)?;
    let loop_ordinals: Vec<u32> = (0..loop_runs.len() as u32).collect();
    let face_runs = runs(&at, element.u32s("faceLoops")?, &loop_ordinals)?;
    if face_runs.len() != nf {
        return Err(Error::invalid(format_args!("{at}: face loops missized")));
    }
    let mut faces = Vec::with_capacity(nf);
    for (f, (name, precision)) in face_names.into_iter().zip(face_precisions).enumerate() {
        let s = face_surfaces[f];
        index(&at, &surfaces, s)?;
        let spec = FaceSpec {
            surface: surfaces[s as usize],
            sense: read_sense(&at, face_senses[f])?,
            precision,
        };
        let mut loops = Vec::new();
        for &l in face_runs[f] {
            let mut coedges = Vec::new();
            for &c in loop_runs[l as usize] {
                let c = c as usize;
                index(&at, &edges, coedge_edges[c])?;
                let pcurve = match coedge_pcurves[c] {
                    NONE => None,
                    p => {
                        index(&at, &pcurves, p)?;
                        Some(pcurves[p as usize])
                    }
                };
                coedges.push(CoedgeSpec {
                    edge: edges[coedge_edges[c] as usize],
                    sense: read_sense(&at, coedge_senses[c])?,
                    pcurve,
                    precision: coedge_precisions[c],
                });
            }
            loops.push(coedges);
        }
        faces.push(b.face(spec, loops, name));
    }
    // Shells and lumps.
    let mut shells = Vec::new();
    for run in runs(
        &at,
        element.u32s("shellFaces")?,
        element.u32s("shellFaceIds")?,
    )? {
        let mut ids = Vec::with_capacity(run.len());
        for &f in run {
            index(&at, &faces, f)?;
            ids.push(faces[f as usize]);
        }
        shells.push(b.shell(ids));
    }
    for run in runs(
        &at,
        element.u32s("lumpShells")?,
        element.u32s("lumpShellIds")?,
    )? {
        let mut ids = Vec::with_capacity(run.len());
        for &s in run {
            index(&at, &shells, s)?;
            ids.push(shells[s as usize]);
        }
        b.lump(ids);
    }
    let body: Result<Body, Error> = b.build(attributes::read(element)?);
    body.map_err(|e| invalid(&at, e))
}

fn write_datums(
    datums: &[Datum],
    names: &mut names::Writer,
    out: &mut dyn ElementWriter,
) -> Result<(), Error> {
    let (mut kinds, mut reals, mut hints) = (Vec::new(), Vec::new(), Vec::new());
    for d in datums {
        match d.kind {
            DatumKind::Plane(f) => {
                kinds.push(0);
                reals.extend(
                    [f.origin(), f.x(), f.y(), f.z()]
                        .map(Vec3::to_array)
                        .as_flattened(),
                );
            }
            DatumKind::Axis { origin, direction } => {
                kinds.push(1);
                reals.extend(origin.to_array().into_iter().chain(direction.to_array()));
            }
            DatumKind::Point(p) => {
                kinds.push(2);
                reals.extend(p.to_array());
            }
            _ => return Err(Error::invalid("a datum of a kind this version can't save")),
        }
        hints.extend(d.hint.center.to_array().into_iter().chain([d.hint.size]));
    }
    out.set(
        "datumNames",
        Field::U32s(names.names(datums.iter().map(|d| &d.name))?),
    )?;
    out.set("datumKinds", Field::U8s(kinds))?;
    out.set("datumReals", Field::F64s(reals))?;
    out.set_rows("datumHints", 4, Field::F64s(hints))
}

fn read_datums(element: &dyn ElementReader, names: &names::Table) -> Result<Vec<Datum>, Error> {
    let at = element.location();
    let kinds = attributes::u8s(element, "datumKinds")?;
    let datum_names = names.all(element.u32s("datumNames")?)?;
    let hints = element.f64s("datumHints")?;
    let mut reals = element.f64s("datumReals")?;
    if datum_names.len() != kinds.len() || hints.len() != 4 * kinds.len() {
        return Err(Error::invalid(format_args!("{at}: datum columns missized")));
    }
    let mut take = |n: usize| -> Result<&[f64], Error> {
        if n > reals.len() {
            return Err(Error::invalid(format_args!(
                "{at}: the datum reals end early"
            )));
        }
        let (head, rest) = reals.split_at(n);
        reals = rest;
        Ok(head)
    };
    let mut datums = Vec::with_capacity(kinds.len());
    for (i, (&kind, name)) in kinds.iter().zip(datum_names).enumerate() {
        let v = |r: &[f64], k: usize| Vec3::new(r[k], r[k + 1], r[k + 2]);
        let kind = match kind {
            0 => {
                let r = take(12)?;
                let frame = Frame::new(v(r, 0), v(r, 3), v(r, 6), v(r, 9));
                DatumKind::Plane(frame.map_err(|e| invalid(&at, e))?)
            }
            1 => {
                let r = take(6)?;
                DatumKind::Axis {
                    origin: v(r, 0),
                    direction: v(r, 3),
                }
            }
            2 => DatumKind::Point(v(take(3)?, 0)),
            k => return Err(Error::invalid(format_args!("{at}: no datum kind {k}"))),
        };
        let h = &hints[4 * i..4 * i + 4];
        datums.push(Datum {
            name,
            kind,
            hint: DatumHint {
                center: Vec3::new(h[0], h[1], h[2]),
                size: h[3],
            },
        });
    }
    if !reals.is_empty() {
        return Err(Error::invalid(format_args!("{at}: the datum reals run on")));
    }
    Ok(datums)
}
