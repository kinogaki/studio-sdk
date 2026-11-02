//! [`BrepBuilder`]: the one way to make a [`Body`], and the structural
//! half of its rules (the geometric half is `consistency`).
use super::body::{Body, Placement};
use super::geometry::{Curve, Geometry, PCurve, Surface};
use super::names::Name;
use super::topology::{BrepTopology, Coedge, Edge, Face, Loop, Lump, Names, Sense, Shell, Vertex};
use super::{
    CoedgeId, CurveId, EdgeId, Entity, FaceId, LoopId, LumpId, MAX_ENTITIES, PCurveId, ShellId,
    SurfaceId, VertexId,
};
use crate::attributes::Attributes;
use crate::derived::Derived;
use crate::error::GeometryError;
use std::collections::BTreeSet;
use std::sync::Arc;

/// An edge to add: an interval of a carrier between two vertices.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EdgeSpec {
    /// The carrier, or `None` for a pole edge.
    pub curve: Option<CurveId>,
    /// `[t0, t1]`, `t0 < t1`.
    pub interval: [f64; 2],
    /// Start (at `t0`) and end (at `t1`).
    pub vertices: [VertexId; 2],
    /// The distance within which it is tolerant, when it is.
    pub precision: Option<f64>,
}

/// A face to add, without its loops.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FaceSpec {
    /// Its surface.
    pub surface: SurfaceId,
    /// `Same` when its outward normal is the surface's normal.
    pub sense: Sense,
    /// The distance within which it is tolerant, when it is.
    pub precision: Option<f64>,
}

/// One coedge of a loop to add.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CoedgeSpec {
    /// Its edge.
    pub edge: EdgeId,
    /// `Same` when it runs from its edge's start to its end.
    pub sense: Sense,
    /// Its edge in the face's chart.
    pub pcurve: Option<PCurveId>,
    /// The distance within which it is tolerant, when it is.
    pub precision: Option<f64>,
}

/// Assembles a [`Body`]: add carriers and entities, then
/// [`build`](Self::build) checks the body's rules ([`super`]) and links it.
///
/// Every id handed out is the entity's id in the built body. A face's
/// loops and their coedges are numbered in the order faces are added (the
/// face's loops in order, each loop's coedges in order), and each edge's
/// radial ring runs through its coedges in coedge order.
///
/// Placement defaults to the identity, with no further instances.
#[derive(Clone, Debug)]
#[must_use]
pub struct BrepBuilder {
    name: Name,
    tolerance: f64,
    placement: Placement,
    instances: Vec<Placement>,
    surfaces: Vec<Surface>,
    curves: Vec<Curve>,
    pcurves: Vec<PCurve>,
    points: Vec<[f64; 3]>,
    vertices: Vec<Vertex>,
    vertex_names: Vec<Name>,
    edges: Vec<EdgeSpec>,
    edge_names: Vec<Name>,
    coedges: Vec<(CoedgeSpec, LoopId)>,
    loops: Vec<Loop>,
    faces: Vec<(FaceSpec, u32, u32)>,
    face_names: Vec<Name>,
    shells: Vec<Vec<FaceId>>,
    lumps: Vec<Vec<ShellId>>,
}

impl BrepBuilder {
    /// A builder for the body named `name`, built to the positional
    /// `tolerance` (P, in model units).
    pub fn new(name: Name, tolerance: f64) -> Self {
        Self {
            name,
            tolerance,
            placement: Placement::IDENTITY,
            instances: Vec::new(),
            surfaces: Vec::new(),
            curves: Vec::new(),
            pcurves: Vec::new(),
            points: Vec::new(),
            vertices: Vec::new(),
            vertex_names: Vec::new(),
            edges: Vec::new(),
            edge_names: Vec::new(),
            coedges: Vec::new(),
            loops: Vec::new(),
            faces: Vec::new(),
            face_names: Vec::new(),
            shells: Vec::new(),
            lumps: Vec::new(),
        }
    }

    /// Places the body at `placement`, and again at each of `instances`.
    pub fn placement(&mut self, placement: Placement, instances: Vec<Placement>) {
        self.placement = placement;
        self.instances = instances;
    }

    /// Adds a surface.
    pub fn surface(&mut self, surface: Surface) -> SurfaceId {
        self.surfaces.push(surface);
        SurfaceId::new(self.surfaces.len() - 1)
    }

    /// Adds a carrier curve.
    pub fn curve(&mut self, curve: Curve) -> CurveId {
        self.curves.push(curve);
        CurveId::new(self.curves.len() - 1)
    }

    /// Adds a p-curve.
    pub fn pcurve(&mut self, pcurve: PCurve) -> PCurveId {
        self.pcurves.push(pcurve);
        PCurveId::new(self.pcurves.len() - 1)
    }

    /// Adds a vertex at `point`, tolerant within `precision` when given.
    pub fn vertex(&mut self, point: [f64; 3], precision: Option<f64>, name: Name) -> VertexId {
        self.points.push(point);
        self.vertices.push(Vertex { precision });
        self.vertex_names.push(name);
        VertexId::new(self.vertices.len() - 1)
    }

    /// Adds an edge.
    pub fn edge(&mut self, edge: EdgeSpec, name: Name) -> EdgeId {
        self.edges.push(edge);
        self.edge_names.push(name);
        EdgeId::new(self.edges.len() - 1)
    }

    /// Adds a face and its loops, the outer first, each loop's coedges in
    /// order round it (none for a face on a whole closed surface).
    pub fn face(&mut self, face: FaceSpec, loops: Vec<Vec<CoedgeSpec>>, name: Name) -> FaceId {
        let id = FaceId::new(self.faces.len());
        let first_loop = self.loops.len() as u32;
        for coedges in &loops {
            let owner = LoopId::new(self.loops.len());
            let first = self.coedges.len() as u32;
            self.coedges.extend(coedges.iter().map(|&c| (c, owner)));
            self.loops.push(Loop {
                face: id,
                first,
                len: coedges.len() as u32,
            });
        }
        self.faces.push((face, first_loop, loops.len() as u32));
        self.face_names.push(name);
        id
    }

    /// Adds a shell of these faces.
    pub fn shell(&mut self, faces: Vec<FaceId>) -> ShellId {
        self.shells.push(faces);
        ShellId::new(self.shells.len() - 1)
    }

    /// Adds a lump: its outer shell first, then its voids.
    pub fn lump(&mut self, shells: Vec<ShellId>) -> LumpId {
        self.lumps.push(shells);
        LumpId::new(self.lumps.len() - 1)
    }

    /// The body, with `attributes`, checked ([`super`]'s rules).
    ///
    /// # Errors
    /// The first rule broken, as [`ErrorKind::Brep`](crate::ErrorKind::Brep)
    /// naming the entity at fault (or a [`Limit`](crate::ErrorKind::Limit)
    /// or [`Attribute`](crate::ErrorKind::Attribute) refusal).
    pub fn build(self, attributes: Attributes) -> Result<Body, GeometryError> {
        let structure = self.structure()?;
        let geometry = Arc::new(Geometry::new(
            self.points,
            self.surfaces,
            self.curves,
            self.pcurves,
        )?);
        let names = Arc::new(Names {
            faces: self.face_names.into(),
            edges: self.edge_names.into(),
            vertices: self.vertex_names.into(),
            fingerprint: std::sync::OnceLock::new(),
        });
        Body::checked(
            self.name,
            self.tolerance,
            self.placement,
            self.instances.into(),
            geometry,
            Arc::new(structure),
            names,
            Arc::new(attributes),
        )
    }

    /// Checks the structure and links rings, shells and lumps.
    fn structure(&self) -> Result<BrepTopology, GeometryError> {
        let refuse =
            |entity, phrase, detail: &str| Err(GeometryError::brep(entity, phrase, detail.into()));
        let tables = [
            self.surfaces.len(),
            self.curves.len(),
            self.pcurves.len(),
            self.vertices.len(),
            self.edges.len(),
            self.coedges.len(),
            self.loops.len(),
            self.faces.len(),
            self.shells.len(),
            self.lumps.len(),
        ];
        if tables.iter().any(|&n| n > MAX_ENTITIES) {
            return refuse(
                None,
                "too many entities",
                "a body holds at most the entity limit of each kind",
            );
        }
        let precise = |p: Option<f64>| p.is_none_or(|p| p.is_finite() && p >= 0.0);
        for (i, v) in self.vertices.iter().enumerate() {
            let at = Some(Entity::Vertex(VertexId::new(i)));
            if !precise(v.precision) {
                return refuse(
                    at,
                    "precision negative",
                    "a precision must be finite and not negative",
                );
            }
        }
        for (i, e) in self.edges.iter().enumerate() {
            let at = Some(Entity::Edge(EdgeId::new(i)));
            if e.curve.is_some_and(|c| c.index() >= self.curves.len())
                || e.vertices.iter().any(|v| v.index() >= self.vertices.len())
            {
                return refuse(
                    at,
                    "dangling reference",
                    "an edge names a curve or vertex the body lacks",
                );
            }
            let [t0, t1] = e.interval;
            if !(t0.is_finite() && t1.is_finite() && t0 < t1) {
                return refuse(
                    at,
                    "empty edge interval",
                    "an edge's interval must be finite with its start below its end",
                );
            }
            if e.curve.is_none() && e.vertices[0] != e.vertices[1] {
                return refuse(
                    at,
                    "bad pole edge",
                    "an edge without a curve is a pole: both its ends are one vertex",
                );
            }
            if !precise(e.precision) {
                return refuse(
                    at,
                    "precision negative",
                    "a precision must be finite and not negative",
                );
            }
        }
        for (i, (f, _, _)) in self.faces.iter().enumerate() {
            let at = Some(Entity::Face(FaceId::new(i)));
            if f.surface.index() >= self.surfaces.len() {
                return refuse(
                    at,
                    "dangling reference",
                    "a face names a surface the body lacks",
                );
            }
            if !precise(f.precision) {
                return refuse(
                    at,
                    "precision negative",
                    "a precision must be finite and not negative",
                );
            }
        }
        for (i, (c, _)) in self.coedges.iter().enumerate() {
            let at = Some(Entity::Coedge(CoedgeId::new(i)));
            if c.edge.index() >= self.edges.len()
                || c.pcurve.is_some_and(|p| p.index() >= self.pcurves.len())
            {
                return refuse(
                    at,
                    "dangling reference",
                    "a coedge names an edge or p-curve the body lacks",
                );
            }
            if self.edges[c.edge.index()].curve.is_none() && c.pcurve.is_none() {
                return refuse(
                    at,
                    "bad pole edge",
                    "every coedge of a pole edge has a p-curve",
                );
            }
            if !precise(c.precision) {
                return refuse(
                    at,
                    "precision negative",
                    "a precision must be finite and not negative",
                );
            }
        }
        let ends = |c: &CoedgeSpec| {
            let [a, b] = self.edges[c.edge.index()].vertices;
            match c.sense {
                Sense::Same => (a, b),
                Sense::Reversed => (b, a),
            }
        };
        for (l, lp) in self.loops.iter().enumerate() {
            let at = Some(Entity::Loop(LoopId::new(l)));
            let coedges = &self.coedges[lp.first as usize..(lp.first + lp.len) as usize];
            if coedges.is_empty() {
                return refuse(at, "empty loop", "a loop has at least one coedge");
            }
            for (i, (c, _)) in coedges.iter().enumerate() {
                let (next, _) = &coedges[(i + 1) % coedges.len()];
                if ends(c).1 != ends(next).0 {
                    return refuse(
                        at,
                        "open loop",
                        "each coedge of a loop ends where the next starts",
                    );
                }
            }
        }

        // Each face in one shell, each shell in one lump.
        let mut face_shell = vec![None; self.faces.len()];
        let mut shell_faces = Vec::new();
        let mut shell_ranges = Vec::with_capacity(self.shells.len());
        for (s, faces) in self.shells.iter().enumerate() {
            let at = Some(Entity::Shell(ShellId::new(s)));
            if faces.is_empty() {
                return refuse(at, "empty shell", "a shell has at least one face");
            }
            shell_ranges.push((shell_faces.len() as u32, faces.len() as u32));
            for &f in faces {
                if f.index() >= self.faces.len() {
                    return refuse(
                        at,
                        "dangling reference",
                        "a shell names a face the body lacks",
                    );
                }
                if face_shell[f.index()].replace(ShellId::new(s)).is_some() {
                    return refuse(
                        Some(Entity::Face(f)),
                        "face in two shells",
                        "each face is in exactly one shell",
                    );
                }
                shell_faces.push(f);
            }
        }
        if let Some(f) = face_shell.iter().position(Option::is_none) {
            return refuse(
                Some(Entity::Face(FaceId::new(f))),
                "face in no shell",
                "each face is in exactly one shell",
            );
        }
        let mut shell_lump = vec![None; self.shells.len()];
        let mut lump_shells = Vec::new();
        let mut lumps = Vec::with_capacity(self.lumps.len());
        for (l, shells) in self.lumps.iter().enumerate() {
            let at = Some(Entity::Lump(LumpId::new(l)));
            if shells.is_empty() {
                return refuse(at, "empty lump", "a lump has at least one shell");
            }
            lumps.push(Lump {
                first: lump_shells.len() as u32,
                len: shells.len() as u32,
            });
            for &s in shells {
                if s.index() >= self.shells.len() {
                    return refuse(
                        at,
                        "dangling reference",
                        "a lump names a shell the body lacks",
                    );
                }
                if shell_lump[s.index()].replace(LumpId::new(l)).is_some() {
                    return refuse(
                        Some(Entity::Shell(s)),
                        "shell in two lumps",
                        "each shell is in exactly one lump",
                    );
                }
                lump_shells.push(s);
            }
        }
        if let Some(s) = shell_lump.iter().position(Option::is_none) {
            return refuse(
                Some(Entity::Shell(ShellId::new(s))),
                "shell in no lump",
                "each shell is in exactly one lump",
            );
        }

        // Radial rings: an edge's coedges in coedge order; one or two, and
        // two run opposite ways in one shell. A shell is closed when every
        // edge but a pole has two.
        let shell_of = |c: usize| {
            let face = self.loops[self.coedges[c].1.index()].face;
            face_shell[face.index()].unwrap_or(ShellId::new(0))
        };
        let mut uses: Vec<Vec<usize>> = vec![Vec::new(); self.edges.len()];
        for (i, (c, _)) in self.coedges.iter().enumerate() {
            uses[c.edge.index()].push(i);
        }
        let mut radial = vec![0usize; self.coedges.len()];
        let mut open = vec![false; self.shells.len()];
        for (e, ring) in uses.iter().enumerate() {
            let at = Some(Entity::Edge(EdgeId::new(e)));
            match ring.as_slice() {
                [] => return refuse(at, "edge bounds no face", "every edge has a coedge"),
                &[only] => {
                    radial[only] = only;
                    if self.edges[e].curve.is_some() {
                        open[shell_of(only).index()] = true;
                    }
                }
                &[a, b] => {
                    if self.coedges[a].0.sense == self.coedges[b].0.sense
                        || shell_of(a) != shell_of(b)
                    {
                        return refuse(
                            at,
                            "non-manifold edge",
                            "an edge's two coedges run opposite ways in one shell",
                        );
                    }
                    radial[a] = b;
                    radial[b] = a;
                }
                _ => return refuse(at, "non-manifold edge", "an edge has one coedge or two"),
            }
        }

        // Names distinct per kind.
        distinct(&self.face_names, |i| Entity::Face(FaceId::new(i)))?;
        distinct(&self.edge_names, |i| Entity::Edge(EdgeId::new(i)))?;
        distinct(&self.vertex_names, |i| Entity::Vertex(VertexId::new(i)))?;

        let edges: Vec<Edge> = self
            .edges
            .iter()
            .zip(&uses)
            .map(|(e, ring)| Edge {
                curve: e.curve,
                interval: e.interval,
                vertices: e.vertices,
                precision: e.precision,
                coedge: CoedgeId::new(ring[0]),
            })
            .collect();
        let coedges: Vec<Coedge> = self
            .coedges
            .iter()
            .zip(&radial)
            .map(|((c, owner), &r)| Coedge {
                edge: c.edge,
                sense: c.sense,
                owner: *owner,
                radial: CoedgeId::new(r),
                pcurve: c.pcurve,
                precision: c.precision,
            })
            .collect();
        let faces: Vec<Face> = self
            .faces
            .iter()
            .zip(&face_shell)
            .map(|(&(f, first_loop, loops), shell)| Face {
                surface: f.surface,
                sense: f.sense,
                first_loop,
                loops,
                shell: shell.unwrap_or(ShellId::new(0)),
                precision: f.precision,
            })
            .collect();
        let shells: Vec<Shell> = shell_ranges
            .iter()
            .enumerate()
            .map(|(s, &(first, len))| Shell {
                lump: shell_lump[s].unwrap_or(LumpId::new(0)),
                first,
                len,
                closed: !open[s],
            })
            .collect();
        Ok(BrepTopology {
            vertices: self.vertices.clone().into(),
            edges: edges.into(),
            coedges: coedges.into(),
            loops: self.loops.clone().into(),
            faces: faces.into(),
            shells: shells.into(),
            shell_faces: shell_faces.into(),
            lumps: lumps.into(),
            lump_shells: lump_shells.into(),
            derived: Derived::default(),
        })
    }
}

/// Refuses the first name that repeats, blaming its second holder.
fn distinct(names: &[Name], entity: impl Fn(usize) -> Entity) -> Result<(), GeometryError> {
    let mut seen = BTreeSet::new();
    match names.iter().position(|n| !seen.insert(n)) {
        Some(i) => Err(GeometryError::brep(
            Some(entity(i)),
            "duplicate name",
            "two faces, edges or vertices of one body share a name".into(),
        )),
        None => Ok(()),
    }
}
