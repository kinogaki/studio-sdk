//! [`Instances`]: placements of prototypes of any type, the standard value
//! `kinogaki.core.instances`, and [`InstancesCodec`], how it saves.
use super::{attributes, instances, selection};
use crate::Error;
use crate::codec::DataCodec;
use crate::components::Components;
use crate::data::Data;
use crate::element::{ElementReader, ElementWriter, Field};
use kinogaki_edit::editor::{Placed, gather, gather_table, picked, spread};
use kinogaki_geometry::{
    Attribute, Attributes, Column, Domain, Elements, Geometry, Meaning, Text, Values, Xform, names,
};
use std::sync::Arc;

/// The Primitive integer a realize numbers each copy's primitives by.
pub const COPYNUM: &str = "copynum";

/// How deep instances of instances nest: a value holding instances of
/// itself this many times over is refused.
pub const MAX_DEPTH: usize = 8;

/// Instance `i`'s [`MATERIAL_MAP`](names::MATERIAL_MAP) in `attributes`:
/// pairs of a material its prototype's primitives name and the one it
/// renders them with, a pair naming a blank left out; empty when it maps
/// none.
///
/// ```
/// use kinogaki_studio_api::geometry::{Attribute, AttributeType, Attributes, Domain, Text, Values, names};
/// use kinogaki_studio_api::standard::material_map;
///
/// let ty = AttributeType { array: true, ..AttributeType::TEXT };
/// let pairs = Text::from_strs(["/m/Red", "/m/Blue"]);
/// let map = Attribute::arrays(ty, vec![0, 0, 2].into(), Values::Text(pairs)).unwrap();
/// let attributes = Attributes::new().with(Domain::Instance, names::MATERIAL_MAP, map).unwrap();
/// assert!(material_map(&attributes, 0).is_empty());
/// assert_eq!(material_map(&attributes, 1), [("/m/Red", "/m/Blue")]);
/// ```
pub fn material_map(attributes: &Attributes, i: usize) -> Vec<(&str, &str)> {
    let map = attributes.get(Domain::Instance, names::MATERIAL_MAP);
    let (Some(offsets), Some(Values::Text(text))) =
        (map.and_then(Attribute::offsets), map.map(Attribute::values))
    else {
        return Vec::new();
    };
    let run = |k: usize| offsets.as_slice().get(k).map(|&at| at as usize);
    let (Some(from), Some(to)) = (run(i), run(i + 1)) else {
        return Vec::new();
    };
    ((from..to).step_by(2))
        .filter(|&k| k + 1 < to)
        .filter_map(|k| Some((text.get(k)?, text.get(k + 1)?)))
        .collect()
}

/// `value` as instance `i` of `attributes` renders it: its primitives'
/// `material` the instance's own `material`, else its [`material_map`]'s in
/// place of theirs ([`materials_mapped`]); none when it renders them as they
/// are.
///
/// # Errors
/// None for a value as attributes check it; the table's refusal otherwise.
pub fn instance_materials(
    value: &Geometry,
    attributes: &Attributes,
    i: usize,
) -> Result<Option<Geometry>, Error> {
    match (attributes.text(Domain::Instance, names::MATERIAL)).and_then(|t| t.get(i)) {
        Some(whole) => {
            let every = Text::from_strs(std::iter::repeat_n(whole, value.primitives().len()));
            let named = (value.attributes()).with(
                Domain::Primitive,
                names::MATERIAL,
                Attribute::text(every),
            )?;
            value.with_attributes(named).map(Some)
        }
        None => materials_mapped(value, &material_map(attributes, i)),
    }
}

/// `value` with each `material` its primitives name that `map` pairs with
/// another named that other instead; none when `map` is empty or the value
/// names no materials.
///
/// # Errors
/// As [`instance_materials`].
pub fn materials_mapped(value: &Geometry, map: &[(&str, &str)]) -> Result<Option<Geometry>, Error> {
    let own = value.attributes().text(Domain::Primitive, names::MATERIAL);
    let (Some(own), false) = (own, map.is_empty()) else {
        return Ok(None);
    };
    let to = |name: &Arc<str>| (map.iter()).find(|(from, _)| **from == **name);
    let table: Arc<[Arc<str>]> = (own.table().iter())
        .map(|name| to(name).map_or_else(|| name.clone(), |&(_, to)| Arc::from(to)))
        .collect();
    let text = Text::new(own.indices().clone(), table)?;
    let named =
        (value.attributes()).with(Domain::Primitive, names::MATERIAL, Attribute::text(text))?;
    value.with_attributes(named).map(Some)
}

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
        finite(xforms.as_slice())?;
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
        finite(xforms.as_slice())?;
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

    /// The instances `kept` names, in its order, each with its placement and
    /// its Instance attributes and groups; the prototypes they place, each
    /// still held once (one no kept instance places goes), and the Detail
    /// as it was. What a Delete of the others leaves.
    ///
    /// ```
    /// use kinogaki_studio_api::geometry::{Attributes, Geometry, Vec3, Xform};
    /// use kinogaki_studio_api::standard::{self, Instances};
    ///
    /// let (a, b) = (Geometry::empty(), Geometry::empty());
    /// let two = vec![standard::geometry().wrap(a), standard::geometry().wrap(b)];
    /// let at = (0..3).map(|i| Xform::translation(Vec3::new(i as f64, 0.0, 0.0))).collect();
    /// let copies = Instances::new(two, at, Some(vec![0, 1, 0].into()), Attributes::new()).unwrap();
    /// let left = copies.kept(&[2, 0]).unwrap();
    /// assert_eq!((left.len(), left.prototypes().len()), (2, 1));
    /// assert_eq!(left.xforms()[0], copies.xforms()[2]);
    /// assert!(left.prototypes()[0].ptr_eq(&copies.prototypes()[0]));
    /// ```
    ///
    /// # Errors
    /// An index past the instances.
    pub fn kept(&self, kept: &[u32]) -> Result<Self, Error> {
        if let Some(&i) = kept.iter().find(|&&i| i as usize >= self.len()) {
            return Err(Error::invalid(format_args!(
                "instance {i} of {}",
                self.len()
            )));
        }
        // The prototypes the kept instances place, in their order, renumbered.
        let mut number = vec![u32::MAX; self.prototypes.len()];
        let mut prototypes = Vec::new();
        let mut which = Vec::with_capacity(kept.len());
        for &i in kept {
            let k = self.prototype_index(i as usize);
            if number[k] == u32::MAX {
                number[k] = prototypes.len() as u32;
                prototypes.push(self.prototypes[k].clone());
            }
            which.push(number[k]);
        }
        let xforms = kept.iter().map(|&i| self.xforms[i as usize]).collect();
        let mut table = Attributes::new();
        for (domain, name, a) in self.attributes.iter() {
            let a = match domain {
                Domain::Instance => picked(a, kept)?,
                _ => a.clone(),
            };
            table.set(domain, name, a)?;
        }
        for (domain, name, group) in self.attributes.groups() {
            let group = match domain {
                Domain::Instance => {
                    let held = (kept.iter().enumerate()).filter(|&(_, &i)| group.contains(i));
                    Elements::from_indices(kept.len(), held.map(|(j, _)| j as u32))?
                }
                _ => group.clone(),
            };
            table.set_group(domain, name, group)?;
        }
        let which = (prototypes.len() > 1).then(|| which.into());
        Self::new(prototypes, xforms, which, table)
    }

    /// `values` as one value: each of Instances as it is, and the rest (each
    /// [`geometry`](super::geometry)'s gathered first) as one instance where
    /// it is. Attributes gather as a Merge's do; each prototype is held once,
    /// shared.
    ///
    /// ```
    /// use kinogaki_studio_api::geometry::{Attributes, Geometry, Xform};
    /// use kinogaki_studio_api::standard::{self, Instances};
    ///
    /// let leaf = standard::geometry().wrap(Geometry::empty());
    /// let at = vec![Xform::IDENTITY; 2].into();
    /// let leaves = Instances::new(vec![leaf], at, None, Attributes::new()).unwrap();
    /// let trunk = standard::geometry().wrap(Geometry::empty());
    /// let tree = Instances::gathered(&[trunk, standard::instances().wrap(leaves)]).unwrap();
    /// assert_eq!((tree.len(), tree.prototypes().len()), (3, 2));
    /// assert_eq!(tree.prototype_index(2), 1);
    /// ```
    ///
    /// # Errors
    /// Ids past the largest, or nesting too deep.
    pub fn gathered(values: &[Data]) -> Result<Self, Error> {
        let (mut prototypes, mut which, mut xforms) = (Vec::<Data>::new(), Vec::new(), Vec::new());
        let plain = Self::loose(values)?;
        let parts: Vec<&Instances> = (plain.iter())
            .chain(values.iter().filter_map(|d| instances().get(d)))
            .collect();
        for part in &parts {
            let at = prototypes.len() as u32;
            prototypes.extend(part.prototypes.iter().cloned());
            xforms.extend(part.xforms.iter().copied());
            which.extend((0..part.len()).map(|i| at + part.prototype_index(i) as u32));
        }
        let tables: Vec<_> = (parts.iter()).map(|p| (&*p.attributes, p.len())).collect();
        let mut table = gather_table(&tables, Domain::Instance)?;
        let details: Vec<_> = (parts.iter()).map(|p| (&*p.attributes, 1)).collect();
        for (_, name, a) in gather_table(&details, Domain::Detail)?.iter() {
            table = table.with(Domain::Detail, name, a.clone())?;
        }
        Self::new(prototypes, xforms.into(), Some(which.into()), table)
    }

    /// One instance holding `values`' other than Instances where they are.
    fn loose(values: &[Data]) -> Result<Vec<Instances>, Error> {
        let geometries: Vec<Placed<'_>> = (values.iter())
            .filter_map(|d| super::geometry().get(d).map(Placed::new))
            .collect();
        let mut loose = Vec::new();
        if !geometries.is_empty() {
            loose.push(super::geometry().wrap(gather(&geometries)?));
        }
        let other = |d: &&Data| instances().get(d).is_none() && super::geometry().get(d).is_none();
        loose.extend(values.iter().filter(other).cloned());
        (loose.into_iter())
            .map(|one| {
                let at = vec![Xform::IDENTITY].into();
                Self::new(vec![one], at, None, Attributes::new())
            })
            .collect()
    }

    /// These instances as one [`Geometry`]: each its prototype's elements at
    /// its placement ([`gather`]), instances of instances followed, what
    /// isn't [`Geometry`] made polygons by `polygons`. A copy's primitives
    /// name the materials its instance renders them with
    /// ([`instance_materials`]). Another Instance
    /// attribute lands on the copy's points, under its name (its placing
    /// ones, used up, stay out), and the Primitive integer `copynum` numbers
    /// the instances. Instances `polygons` makes nothing of give nothing, and
    /// the count of them is the second value.
    ///
    /// # Errors
    /// What [`gather`] refuses.
    pub fn realized(
        &self,
        polygons: &dyn Fn(&Data) -> Option<Geometry>,
    ) -> Result<(Geometry, usize), Error> {
        let mut left_out = 0;
        let made: Vec<Option<Geometry>> = (self.prototypes.iter())
            .map(|p| match (super::geometry().get(p), instances().get(p)) {
                (Some(value), _) => Some(value.clone()),
                (None, Some(nested)) => {
                    let (value, nested_out) = nested.realized(polygons).ok()?;
                    left_out += nested_out;
                    Some(value)
                }
                (None, None) => polygons(p),
            })
            .collect();
        // Each copy as its instance renders it, where it names materials.
        let rendered: Vec<Option<Geometry>> = (0..self.len())
            .map(|i| match &made[self.prototype_index(i)] {
                Some(value) => instance_materials(value, &self.attributes, i),
                None => Ok(None),
            })
            .collect::<Result<_, Error>>()?;
        let mut placed = Vec::with_capacity(self.len());
        let mut kept = Vec::with_capacity(self.len());
        let (mut points, mut primitives) = (Vec::new(), Vec::new());
        for i in 0..self.len() {
            let value = rendered[i]
                .as_ref()
                .or(made[self.prototype_index(i)].as_ref());
            let (p, q) = value.map_or((0, 0), |v| (v.positions().len(), v.primitives().len()));
            points.push(p);
            primitives.push(q);
            match value {
                Some(value) => {
                    placed.push(Placed {
                        value,
                        at: self.xforms[i],
                    });
                    kept.push(i);
                }
                None => left_out += 1,
            }
        }
        let value = gather(&placed)?;
        let numbers: Vec<i32> = (0..self.len())
            .flat_map(|i| std::iter::repeat_n(i as i32, primitives[i]))
            .collect();
        let copynum = Attribute::tuples(numbers, Meaning::None)?;
        let mut table = value
            .attributes()
            .with(Domain::Primitive, COPYNUM, copynum)?;
        for (domain, name, a) in self.attributes.iter() {
            let placing = [
                names::ID,
                names::PSCALE,
                names::SCALE,
                names::ORIENT,
                names::TRANSFORM,
                names::MATERIAL,
            ];
            if domain == Domain::Instance
                && !placing.contains(&name)
                && let Some(column) = spread(a, &points)?
            {
                table = table.with(Domain::Point, name, column)?;
            }
        }
        Ok((value.with_attributes(table)?, left_out))
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
/// Components save as the standard [`Selection`](kinogaki_geometry::Selection)
/// of the Instance domain.
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
        let value = (instances().get(data)).ok_or_else(|| Error::invalid("not instances"))?;
        selection::read(element, data.ty(), |domain| value.count(domain))
    }
}
