//! [`Instances`]: placements of prototypes of any type, the standard value
//! `kinogaki.core.instances`, and [`InstancesCodec`], how it saves.
use super::{attributes, instances};
use crate::codec::DataCodec;
use crate::data::Data;
use crate::element::{ElementReader, ElementWriter, Field};
use crate::error::Error;
use kinogaki_geometry::{Attributes, Column, Domain, Xform};
use std::sync::Arc;

/// How deep instances of instances nest: a value holding instances of
/// itself this many times over is refused.
pub const MAX_DEPTH: usize = 8;

/// Placements of prototypes of any registered type (Geometry, a B-rep,
/// instances of instances…): the standard value `kinogaki.core.instances`.
///
/// Each instance is a placement ([`Xform`], relative to the object) of one
/// prototype. The prototypes are held once each, however many instances
/// place them: a move of the instances ([`with_xforms`](Self::with_xforms))
/// shares them, so the viewport and renderers reuse what they made of
/// each prototype. Per-instance and whole-value attributes live on the
/// [`Instance`](Domain::Instance) and [`Detail`](Domain::Detail) domains
/// (`id`, `name`, `Cd`, `material`, `part`, `v`…).
///
/// ```
/// use kinogaki_studio_api::geometry::{Attributes, Geometry, Vec3, Xform};
/// use kinogaki_studio_api::standard::{self, Instances};
///
/// let prototype = standard::geometry().wrap(Geometry::empty());
/// let xforms = (0..3).map(|i| Xform::translation(Vec3::new(i as f64, 0.0, 0.0))).collect();
/// let copies = Instances::new(vec![prototype], xforms, None, Attributes::new()).unwrap();
/// assert_eq!((copies.len(), copies.prototypes().len()), (3, 1));
/// let moved = copies.with_xforms(vec![Xform::IDENTITY; 3].into()).unwrap();
/// assert!(moved.prototypes()[0].ptr_eq(&copies.prototypes()[0]));
/// ```
#[derive(Clone)]
pub struct Instances {
    prototypes: Arc<[Data]>,
    xforms: Column<Xform>,
    prototype: Option<Column<u32>>,
    attributes: Arc<Attributes>,
    depth: usize,
}

impl Instances {
    /// `xforms` placing `prototypes`: instance `i` places prototype
    /// `prototype[i]`, or the one prototype when `prototype` is `None`.
    ///
    /// # Errors
    /// Instances without a prototype, a prototype index past the list (or
    /// no index column for several prototypes), a placement that isn't
    /// finite, nesting deeper than [`MAX_DEPTH`], or an attribute on
    /// another domain than Instance and Detail or sized for another count.
    pub fn new(
        prototypes: Vec<Data>,
        xforms: Column<Xform>,
        prototype: Option<Column<u32>>,
        attributes: Attributes,
    ) -> Result<Self, Error> {
        let prototypes: Arc<[Data]> = prototypes.into();
        let depth = 1
            + (prototypes.iter())
                .filter_map(|p| instances().get(p).map(|i| i.depth))
                .max()
                .unwrap_or(0);
        if depth > MAX_DEPTH {
            return Err(Error::invalid(format_args!(
                "instances nest {depth} deep, more than {MAX_DEPTH}"
            )));
        }
        match &prototype {
            None if prototypes.len() > 1 => {
                return Err(Error::invalid(
                    "instances of several prototypes name each one's prototype",
                ));
            }
            None if prototypes.is_empty() && !xforms.is_empty() => {
                return Err(Error::invalid("instances without a prototype"));
            }
            Some(column) if column.len() != xforms.len() => {
                return Err(Error::invalid(format_args!(
                    "{} prototype indices for {} instances",
                    column.len(),
                    xforms.len()
                )));
            }
            Some(column) => {
                if let Some(i) = column.iter().position(|&k| k as usize >= prototypes.len()) {
                    return Err(Error::invalid(format_args!(
                        "instance {i} names prototype {}, of {}",
                        column[i],
                        prototypes.len()
                    )));
                }
            }
            None => {}
        }
        finite(&xforms)?;
        let value = Self {
            prototypes,
            xforms,
            prototype,
            attributes: Arc::new(Attributes::new()),
            depth,
        };
        value.with_attributes(attributes)
    }

    /// These instances at `xforms` (one per instance), sharing the
    /// prototypes, their indices and the attributes.
    ///
    /// # Errors
    /// Another count of placements, or one that isn't finite.
    pub fn with_xforms(&self, xforms: Column<Xform>) -> Result<Self, Error> {
        if xforms.len() != self.xforms.len() {
            return Err(Error::invalid(format_args!(
                "{} placements for {} instances",
                xforms.len(),
                self.xforms.len()
            )));
        }
        finite(&xforms)?;
        Ok(Self {
            xforms,
            ..self.clone()
        })
    }

    /// These instances with `attributes`, the entries it shares with the
    /// current ones unchecked.
    ///
    /// # Errors
    /// An attribute or group on another domain than Instance and Detail,
    /// or sized for another count.
    pub fn with_attributes(&self, attributes: Attributes) -> Result<Self, Error> {
        let before = &self.attributes;
        let count = |domain| self.count(domain);
        for (domain, name, a) in attributes.iter() {
            let shared = before.get(domain, name).is_some_and(|b| std::ptr::eq(b, a));
            check(
                domain,
                name,
                if shared { None } else { Some(a.elements()) },
                count,
            )?;
        }
        for (domain, name, g) in attributes.groups() {
            check(domain, name, Some(g.count()), count)?;
        }
        Ok(Self {
            attributes: Arc::new(attributes),
            ..self.clone()
        })
    }

    /// How many instances there are.
    pub fn len(&self) -> usize {
        self.xforms.len()
    }

    /// Whether there are none.
    pub fn is_empty(&self) -> bool {
        self.xforms.is_empty()
    }

    /// The prototypes, each held once.
    pub fn prototypes(&self) -> &[Data] {
        &self.prototypes
    }

    /// The index of the prototype instance `i` places.
    pub fn prototype_index(&self, i: usize) -> usize {
        self.prototype.as_ref().map_or(0, |p| p[i] as usize)
    }

    /// Each instance's prototype index; `None` with one prototype.
    pub fn prototype_indices(&self) -> Option<&Column<u32>> {
        self.prototype.as_ref()
    }

    /// Each instance's placement, relative to the object.
    pub fn xforms(&self) -> &Column<Xform> {
        &self.xforms
    }

    /// The Instance and Detail attributes and groups.
    pub fn attributes(&self) -> &Attributes {
        &self.attributes
    }

    /// How deep it nests: 1, or one more than its deepest prototype that
    /// is instances.
    pub fn depth(&self) -> usize {
        self.depth
    }

    /// How many elements `domain` has: the instances, and one Detail.
    pub fn count(&self, domain: Domain) -> Option<usize> {
        match domain {
            Domain::Instance => Some(self.len()),
            Domain::Detail => Some(1),
            _ => None,
        }
    }
}

/// An error unless every one of `xforms` is finite.
fn finite(xforms: &[Xform]) -> Result<(), Error> {
    let finite = |x: &Xform| x.rows().iter().flatten().all(|c| c.is_finite());
    match xforms.iter().position(|x| !finite(x)) {
        Some(i) => Err(Error::invalid(format_args!(
            "instance {i}'s placement isn't finite"
        ))),
        None => Ok(()),
    }
}

/// An error unless an entry `name` of `domain` (`elements` of them, when
/// not shared with what was checked before) fits `count`.
fn check(
    domain: Domain,
    name: &str,
    elements: Option<usize>,
    count: impl Fn(Domain) -> Option<usize>,
) -> Result<(), Error> {
    let Some(expected) = count(domain) else {
        return Err(Error::invalid(format_args!(
            "instances have no {} domain ({name})",
            domain.name()
        )));
    };
    match elements {
        Some(n) if n != expected => Err(Error::invalid(format_args!(
            "{} {name} has {n} values for {expected} elements",
            domain.name()
        ))),
        _ => Ok(()),
    }
}

/// Saves [`Instances`]: the field `xforms` (rows of 12 reals, an
/// [`Xform`]'s rows), `prototype` (one index an instance) with several
/// prototypes, each prototype as the child `i<k>` through its own type's
/// codec ([`ElementWriter::data`], once per save for every value sharing
/// it), and the attributes as children. A round trip is bit-exact.
#[derive(Clone, Copy, Debug, Default)]
pub struct InstancesCodec;

impl DataCodec for InstancesCodec {
    fn write(
        &self,
        data: &Data,
        _base: Option<&Data>,
        out: &mut dyn ElementWriter,
    ) -> Result<(), Error> {
        let value = (instances().get(data)).ok_or_else(|| Error::invalid("not instances"))?;
        let rows = value
            .xforms
            .iter()
            .flat_map(|x| x.rows().as_flattened().to_vec());
        out.set_rows("xforms", 12, Field::F64s(rows.collect()))?;
        if let Some(p) = &value.prototype {
            out.set("prototype", Field::U32s(p.to_vec()))?;
        }
        for (k, prototype) in value.prototypes.iter().enumerate() {
            out.data(&format!("i{k}"), prototype)?;
        }
        attributes::write(&value.attributes, out)
    }

    fn read(&self, element: &dyn ElementReader, _base: Option<&Data>) -> Result<Data, Error> {
        let at = element.location();
        let (rows, rest) = element.f64s("xforms")?.as_chunks::<12>();
        if !rest.is_empty() {
            return Err(Error::invalid(format_args!("{at}: xforms are rows of 12")));
        }
        let xforms = (rows.iter())
            .map(|r| {
                Xform::from_rows([
                    [r[0], r[1], r[2], r[3]],
                    [r[4], r[5], r[6], r[7]],
                    [r[8], r[9], r[10], r[11]],
                ])
            })
            .collect();
        let prototype = match element.field("prototype") {
            Some(_) => Some(element.u32s("prototype")?.to_vec().into()),
            None => None,
        };
        let mut prototypes = Vec::new();
        while let Some(prototype) = element.data(&format!("i{}", prototypes.len())) {
            prototypes.push(prototype?);
        }
        let attributes = attributes::read(element)?;
        let value = Instances::new(prototypes, xforms, prototype, attributes)
            .map_err(|e| Error::invalid(format_args!("{at}: {e}")))?;
        Ok(instances().wrap(value))
    }
}
