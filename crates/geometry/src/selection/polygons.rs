//! A [`Selection`] over a [`Polygons`] value: what it holds there, and the
//! same selection as another domain. One meaning for core and every plugin
//! (the status bar's counts, a selection's box, a level change).
use super::Selection;
use crate::attributes::Domain;
use crate::column::Column;
use crate::error::{ErrorKind, GeometryError};
use crate::polygons::Polygons;

/// How [`Selection::converted_by`] relates a target element to the
/// selected elements around it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) enum Coverage {
    /// One selected related element is enough (an edge of a selected
    /// polygon, a point of a selected edge).
    Any,
    /// Every related element must be selected (a polygon all of whose
    /// points are selected).
    Whole,
}

/// What a selection holds on a value, per level. Its own domain counts its
/// elements; the other levels count those whose points it holds wholly.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct SelectionCounts {
    /// Points.
    pub points: usize,
    /// Edges.
    pub edges: usize,
    /// Polygons only: polylines have no face to count.
    pub polygons: usize,
}

impl Selection {
    /// Whether it was made on `on`'s topology (or on none in particular) and
    /// every index is one of its elements.
    ///
    /// # Errors
    /// A stale selection ([`ErrorKind::Selection`], no element), a domain
    /// `on` has none of, or the first index past the end (at it).
    pub fn validate(&self, on: &Polygons) -> Result<(), GeometryError> {
        if self.is_stale(on.topology()) {
            return Err(GeometryError::whole(
                ErrorKind::Selection,
                "the selection was made on another topology",
            ));
        }
        let Some(count) = on.count(self.domain()) else {
            return Err(GeometryError::whole(
                ErrorKind::Selection,
                format!("polygons have no {} domain", self.domain().name()),
            ));
        };
        let indices = self.indices();
        match indices.last() {
            Some(&last) if last as usize >= count => {
                let first = indices.partition_point(|&i| (i as usize) < count);
                let i = indices[first];
                Err(GeometryError::at(
                    ErrorKind::Selection,
                    i as usize,
                    format!("{} {i} is past the last of {count}", self.domain().name()),
                ))
            }
            _ => Ok(()),
        }
    }

    /// The selected elements, ascending: every one of `on`'s for an
    /// all-selection, else the indices themselves (shared, no copy).
    ///
    /// # Errors
    /// As [`validate`](Self::validate).
    pub fn resolved(&self, on: &Polygons) -> Result<Column<u32>, GeometryError> {
        self.validate(on)?;
        Ok(if self.is_all() {
            let count = on.count(self.domain()).unwrap_or(0) as u32;
            Column::new((0..count).collect())
        } else {
            self.indices().clone()
        })
    }

    /// The selection as `domain` by the standard rule: `Coverage::Whole`
    /// into Primitive, and from Point into Edge;
    /// `Coverage::Any` otherwise. An all-selection stays all; the same
    /// domain is the selection itself. Elements related to nothing selected
    /// are never added.
    ///
    /// # Errors
    /// As [`validate`](Self::validate), or a domain `on` has none of.
    pub fn converted(&self, on: &Polygons, domain: Domain) -> Result<Selection, GeometryError> {
        self.validate(on)?;
        if domain == self.domain() {
            return Ok(self.clone());
        }
        if self.is_all() {
            target(on, domain)?;
            return Ok(Selection::all(domain, Some(on.topology().fingerprint())));
        }
        let whole = domain == Domain::Primitive
            || (domain == Domain::Edge && self.domain() == Domain::Point);
        let coverage = if whole {
            Coverage::Whole
        } else {
            Coverage::Any
        };
        self.converted_by(on, domain, coverage)
    }

    /// The selection as `domain`, each target element selected by
    /// `coverage` of the elements it relates to directly (a polygon's
    /// points, edges and vertices; an edge's points and the polygons and
    /// vertices along it). The same domain is the selection itself.
    ///
    /// # Errors
    /// As [`converted`](Self::converted).
    pub(crate) fn converted_by(
        &self,
        on: &Polygons,
        domain: Domain,
        coverage: Coverage,
    ) -> Result<Selection, GeometryError> {
        self.validate(on)?;
        if domain == self.domain() {
            return Ok(self.clone());
        }
        let targets = target(on, domain)?;
        let sources = on.count(self.domain()).unwrap_or(0);
        let mut chosen = vec![self.is_all(); sources];
        for &i in self.indices().iter() {
            chosen[i as usize] = true;
        }
        let whole = coverage == Coverage::Whole;
        let mut touched = vec![false; targets];
        let mut missing = if whole {
            vec![false; targets]
        } else {
            Vec::new()
        };
        let mut relate = |source: usize, target: usize| {
            if chosen[source] {
                touched[target] = true;
            } else if whole {
                missing[target] = true;
            }
        };
        let (from, to) = (self.domain(), domain);
        let topology = on.topology();
        if matches!(
            (from, to),
            (Domain::Point, Domain::Edge) | (Domain::Edge, Domain::Point)
        ) {
            for (edge, e) in topology.edges().iter().enumerate() {
                for point in e.points {
                    match from {
                        Domain::Point => relate(point as usize, edge),
                        _ => relate(edge, point as usize),
                    }
                }
            }
        } else {
            let index = (from == Domain::Edge || to == Domain::Edge).then(|| topology.index());
            let polygons = topology.polygons();
            for p in 0..topology.primitives() {
                let start = topology.offsets()[p] as usize;
                for (corner, &point) in topology.primitive(p).iter().enumerate() {
                    let element = |c: Domain| match c {
                        Domain::Point => Some(point as usize),
                        Domain::Primitive => Some(p),
                        Domain::Vertex => Some(start + corner),
                        // Every element belongs to the one detail.
                        Domain::Detail => Some(0),
                        Domain::Edge => {
                            let index = index.expect("an edge conversion builds the index");
                            if p < polygons {
                                Some(index.face_edges().row(p)[corner] as usize)
                            } else {
                                // A polyline's last vertex starts no segment.
                                index
                                    .polyline_edges()
                                    .row(p - polygons)
                                    .get(corner)
                                    .map(|&e| e as usize)
                            }
                        }
                        _ => None,
                    };
                    if let (Some(source), Some(target)) = (element(from), element(to)) {
                        relate(source, target);
                    }
                }
            }
        }
        let ids: Vec<u32> = (0..targets)
            .filter(|&i| touched[i] && (!whole || !missing[i]))
            .map(|i| i as u32)
            .collect();
        Selection::from_ascending(domain, ids.into(), None, Some(topology.fingerprint()))
    }

    /// The box around the points the selected elements touch, in `on`'s
    /// space; `None` when it touches none or doesn't belong to `on`.
    pub fn bounds(&self, on: &Polygons) -> Option<crate::Bounds> {
        let mask = self.touched_points(on).ok()?;
        let positions = on.positions();
        crate::Bounds::of(
            mask.iter()
                .enumerate()
                .filter(|(_, m)| **m)
                .map(|(p, _)| crate::Vec3::from(positions[p])),
        )
    }

    /// The status bar's counts on `on`; all zero when the selection
    /// doesn't belong to it.
    pub fn counts(&self, on: &Polygons) -> SelectionCounts {
        let Ok(points) = self.touched_points(on) else {
            return SelectionCounts::default();
        };
        let topology = on.topology();
        let polygons = topology.polygons();
        let own = |n: usize| self.len(n);
        let wholly = |ids: &[u32]| ids.iter().all(|&p| points[p as usize]);
        let domain = self.domain();
        SelectionCounts {
            points: match domain {
                Domain::Point => own(topology.points()),
                _ => points.iter().filter(|&&p| p).count(),
            },
            edges: match domain {
                Domain::Edge => own(topology.edges().len()),
                _ => topology
                    .edges()
                    .iter()
                    .filter(|e| wholly(&e.points))
                    .count(),
            },
            polygons: match domain {
                Domain::Primitive if self.is_all() => polygons,
                Domain::Primitive => self.indices().partition_point(|&p| (p as usize) < polygons),
                _ => (0..polygons)
                    .filter(|&p| wholly(topology.primitive(p)))
                    .count(),
            },
        }
    }

    /// One flag a point of `on`: whether a selected element touches it (a
    /// selected point, or a point of a selected edge, primitive or vertex;
    /// every point for the detail).
    fn touched_points(&self, on: &Polygons) -> Result<Vec<bool>, GeometryError> {
        self.validate(on)?;
        let topology = on.topology();
        let mut mask = vec![false; topology.points()];
        let mut mark = |p: u32| mask[p as usize] = true;
        let ids: Box<dyn Iterator<Item = u32>> = if self.is_all() {
            Box::new(0..on.count(self.domain()).unwrap_or(0) as u32)
        } else {
            Box::new(self.indices().iter().copied())
        };
        for id in ids {
            let id = id as usize;
            match self.domain() {
                Domain::Point => mark(id as u32),
                Domain::Edge => topology.edges()[id].points.into_iter().for_each(&mut mark),
                Domain::Primitive => topology.primitive(id).iter().copied().for_each(&mut mark),
                Domain::Vertex => mark(topology.vertices()[id]),
                _ => topology.vertices().iter().copied().for_each(&mut mark),
            }
        }
        Ok(mask)
    }
}

/// How many elements of `domain` `on` has, refusing a domain it has none of.
fn target(on: &Polygons, domain: Domain) -> Result<usize, GeometryError> {
    on.count(domain).ok_or_else(|| {
        GeometryError::whole(
            ErrorKind::Selection,
            format!("polygons have no {} domain", domain.name()),
        )
    })
}
