//! [`gather`]: values into one, each at a placement: what a Merge and a
//! realize of instances make, the one rule for both.
use super::carry::default_item;
use super::place::{Placing, turns};
use crate::ids::{self, Input};
use kinogaki_geometry::{
    Attribute, AttributeType, Attributes, Column, CurveData, Domain, Elements, Error, Geometry,
    Knots, Meaning, Primitives, Storage, Text, Values, Xform, names,
};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::ops::Range;
use std::sync::Arc;

/// One value of a [`gather`] and where it goes.
#[derive(Clone, Copy, Debug)]
pub struct Placed<'a> {
    /// The value.
    pub value: &'a Geometry,
    /// Where it goes.
    pub at: Xform,
}

impl<'a> Placed<'a> {
    /// `value` where it is.
    pub fn new(value: &'a Geometry) -> Self {
        Self {
            value,
            at: Xform::IDENTITY,
        }
    }
}

/// The domains a value's attributes and groups gather on.
const DOMAINS: [Domain; 5] = [
    Domain::Point,
    Domain::Vertex,
    Domain::Primitive,
    Domain::Edge,
    Domain::Detail,
];

/// `values` in one value:
///
/// - **Elements** append in input order; only the numbers before them shift
///   an element's number.
/// - **Attributes** gather by name. The first input holding a name fixes its
///   type; an input lacking it, or holding it as another type, gives its
///   elements the type's default (zero and no text, white for a color, the
///   identity for a rotation). A Detail value is the first holder's.
/// - **Ids:** the first input's stay, colliding and missing ones are
///   [fresh](crate::ids::fresh).
/// - **Groups** gather by name, an element a member when its input's group
///   holds it.
/// - **A placement** moves positions and what turns with them, by each
///   attribute's [`Meaning`]: points move, vectors and matrices follow,
///   normals take the inverse transpose, rotations compose.
///
/// One value at no placement is that value, shared.
///
/// ```
/// use kinogaki_edit::editor::{Placed, gather};
/// use kinogaki_geometry::{Attributes, Geometry, Primitives, Vec3, Xform};
///
/// let t = Primitives::of_polygons(3, vec![0, 3].into(), vec![0, 1, 2].into()).unwrap();
/// let positions = vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
/// let triangle = Geometry::new(positions.into(), t, Attributes::new()).unwrap();
/// let moved = Placed { value: &triangle, at: Xform::translation(Vec3::new(5.0, 0.0, 0.0)) };
/// let both = gather(&[Placed::new(&triangle), moved]).unwrap();
/// assert_eq!(both.positions().len(), 6);
/// assert_eq!(both.positions()[4], [6.0, 0.0, 0.0]);
/// assert_eq!(both.primitives().primitive(1), [3, 4, 5]);
/// ```
///
/// # Errors
/// Ids past the largest, or a table the limits refuse.
pub fn gather(values: &[Placed<'_>]) -> Result<Geometry, Error> {
    match values {
        [] => return Ok(Geometry::empty()),
        [one] if one.at == Xform::IDENTITY => return Ok(one.value.clone()),
        _ => {}
    }
    let positions = positions(values);
    let table = table(values)?;
    let mut attributes = Attributes::new();
    for domain in DOMAINS {
        let sources: Vec<Source<'_>> = (values.iter())
            .map(|v| Source {
                attributes: v.value.attributes(),
                count: v.value.count(domain).unwrap_or(0),
                at: v.at,
            })
            .collect();
        attributes = gather_domain(attributes, domain, &sources)?;
    }
    Geometry::new(positions.into(), table, attributes)
}

fn positions(values: &[Placed<'_>]) -> Vec<[f64; 3]> {
    let total = values.iter().map(|v| v.value.positions().len()).sum();
    let mut out = Vec::with_capacity(total);
    for Placed { value, at } in values {
        if *at == Xform::IDENTITY {
            out.extend(value.positions().iter().copied());
        } else {
            let moved = value.positions().iter();
            out.extend(moved.map(|&p| at.apply_point(p.into()).to_array()));
        }
    }
    out
}

/// The primitives of `values`, end to end.
fn table(values: &[Placed<'_>]) -> Result<Primitives, Error> {
    let tables = || values.iter().map(|v| v.value.primitives());
    let (mut points, mut offsets, mut vertices) = (0u32, vec![0u32], Vec::new());
    let mut kinds = Vec::new();
    for t in tables() {
        let at = *offsets.last().expect("offsets start at 0");
        offsets.extend(t.offsets().as_slice()[1..].iter().map(|&o| o + at));
        vertices.extend(t.vertices().iter().map(|&p| p + points));
        kinds.extend(t.kinds().iter().copied());
        points += t.points() as u32;
    }
    let curves = tables()
        .any(|t| t.curves().is_some())
        .then(|| curve_data(values));
    Primitives::new(
        points as usize,
        offsets.into(),
        vertices.into(),
        kinds.into(),
        curves,
    )
}

/// The curve data of `values`, end to end: a table without any counts as
/// linear, weighing 1.
fn curve_data(values: &[Placed<'_>]) -> CurveData {
    let tables = || values.iter().map(|v| v.value.primitives());
    let rational = tables().any(|t| t.curves().is_some_and(|c| c.weights().is_some()));
    let (mut degrees, mut knot_offsets, mut knots, mut weights) =
        (Vec::new(), vec![0u32], Vec::new(), Vec::new());
    for t in tables() {
        let at = *knot_offsets.last().expect("knot offsets start at 0");
        if let Some(c) = t.curves() {
            degrees.extend(c.degrees().iter().copied());
            let k = c.knots();
            knot_offsets.extend(k.offsets().as_slice()[1..].iter().map(|&o| o + at));
            knots.extend(k.values().iter().copied());
        } else {
            degrees.extend(std::iter::repeat_n(1, t.len()));
            knot_offsets.extend(std::iter::repeat_n(at, t.len()));
        }
        if rational {
            match t.curves().and_then(|c| c.weights()) {
                Some(w) => weights.extend(w.iter().copied()),
                None => weights.extend(std::iter::repeat_n(1.0, t.vertex_count())),
            }
        }
    }
    let knots = Knots::new(knot_offsets.into(), knots.into());
    CurveData::new(degrees.into(), knots, rational.then(|| weights.into()))
}

/// One input's table on one domain: how many elements it has there, and
/// where it goes.
#[derive(Clone, Copy)]
struct Source<'a> {
    attributes: &'a Attributes,
    count: usize,
    at: Xform,
}

/// `tables` (each an attributes table and its element count on `domain`) as
/// one table, by [`gather`]'s rules: what gathers Instance attributes.
///
/// ```
/// use kinogaki_edit::editor::gather_table;
/// use kinogaki_geometry::{Attribute, Attributes, Domain, Meaning};
///
/// let sizes = Attribute::tuples(vec![2.0f32], Meaning::None).unwrap();
/// let some = Attributes::new().with(Domain::Instance, "size", sizes).unwrap();
/// let none = Attributes::new();
/// let table = gather_table(&[(&none, 2), (&some, 1)], Domain::Instance).unwrap();
/// let got = table.tuples::<f32>(Domain::Instance, "size").unwrap();
/// assert_eq!(got, [0.0, 0.0, 2.0]);
/// ```
///
/// # Errors
/// Ids past the largest.
pub fn gather_table(tables: &[(&Attributes, usize)], domain: Domain) -> Result<Attributes, Error> {
    let sources: Vec<Source<'_>> = (tables.iter())
        .map(|&(attributes, count)| Source {
            attributes,
            count,
            at: Xform::IDENTITY,
        })
        .collect();
    gather_domain(Attributes::new(), domain, &sources)
}

/// The number [`picked`] reads as no element.
pub const NOTHING: u32 = u32::MAX;

/// `attribute`, one value an element, with element `e` held by `counts[e]`
/// elements: an instance's value over its copy's points. An array stays out.
///
/// ```
/// use kinogaki_edit::editor::spread;
/// use kinogaki_geometry::{Attribute, Meaning};
///
/// let a = Attribute::tuples(vec![[1.0f32, 0.0], [0.0, 1.0]], Meaning::None).unwrap();
/// let got = spread(&a, &[1, 2]).unwrap().unwrap();
/// assert_eq!(got.as_tuples::<[f32; 2]>().unwrap(), [[1.0, 0.0], [0.0, 1.0], [0.0, 1.0]]);
/// ```
///
/// # Errors
/// What the attribute refuses.
pub fn spread(attribute: &Attribute, counts: &[usize]) -> Result<Option<Attribute>, Error> {
    fn each<T: Copy>(column: &Column<T>, width: usize, counts: &[usize]) -> Column<T> {
        let mut out = Vec::with_capacity(counts.iter().sum::<usize>() * width);
        for (e, &n) in counts.iter().enumerate() {
            for _ in 0..n {
                out.extend((e * width..(e + 1) * width).map(|at| column[at]));
            }
        }
        out.into()
    }
    let ty = attribute.ty();
    if ty.array {
        return Ok(None);
    }
    let width = usize::from(ty.width);
    let values = match attribute.values() {
        Values::F32(c) => Values::F32(each(c, width, counts)),
        Values::F64(c) => Values::F64(each(c, width, counts)),
        Values::I32(c) => Values::I32(each(c, width, counts)),
        Values::I64(c) => Values::I64(each(c, width, counts)),
        Values::Text(t) => {
            return Ok(Some(Attribute::text(t.with_indices(each(
                t.indices(),
                1,
                counts,
            ))?)));
        }
        _ => return Ok(None),
    };
    attribute.with_values(values).map(Some)
}

/// `attribute`'s elements in another order, each new element taking element
/// `from[i]`'s value exactly, whatever its storage, width or array;
/// [`NOTHING`] takes the type's default (zero and no text, white for a color,
/// the identity for a rotation or a matrix, no items for an array). What
/// sorting, copying by number and taking the strongest sample come to.
///
/// ```
/// use kinogaki_edit::editor::{NOTHING, picked};
/// use kinogaki_geometry::{Attribute, Meaning};
///
/// let a = Attribute::tuples(vec![[1.0f32, 0.0, 0.0], [0.0, 1.0, 0.0]], Meaning::Color).unwrap();
/// let got = picked(&a, &[1, NOTHING, 0]).unwrap();
/// assert_eq!(got.as_tuples::<[f32; 3]>().unwrap(), [[0.0, 1.0, 0.0], [1.0; 3], [1.0, 0.0, 0.0]]);
/// ```
///
/// # Errors
/// A number past its elements, or what the attribute refuses.
pub fn picked(attribute: &Attribute, from: &[u32]) -> Result<Attribute, Error> {
    fn items<T: Copy>(
        column: &Column<T>,
        width: usize,
        spans: &[Option<Range<usize>>],
        fill: &[T],
    ) -> Column<T> {
        let mut out = Vec::new();
        for span in spans {
            match span {
                Some(span) => {
                    out.extend((span.start * width..span.end * width).map(|at| column[at]))
                }
                None => out.extend_from_slice(fill),
            }
        }
        out.into()
    }
    let ty = attribute.ty();
    let width = usize::from(ty.width);
    let count = attribute.elements();
    let span = |e: usize| match attribute.offsets() {
        Some(o) => o[e] as usize..o[e + 1] as usize,
        None => e..e + 1,
    };
    let mut spans = Vec::with_capacity(from.len());
    for &e in from {
        spans.push(match e {
            NOTHING => None,
            e if (e as usize) < count => Some(span(e as usize)),
            e => {
                return Err(Error::invalid(format!(
                    "element {e} is past the last of {count}"
                )));
            }
        });
    }
    let fill = if ty.array {
        Vec::new()
    } else {
        default_item(ty)
    };
    let reals32: Vec<f32> = fill.iter().map(|&x| x as f32).collect();
    let ints32: Vec<i32> = fill.iter().map(|&x| x as i32).collect();
    let ints64: Vec<i64> = fill.iter().map(|&x| x as i64).collect();
    let none = if ty.array {
        Vec::new()
    } else {
        vec![Text::NONE]
    };
    let values = match attribute.values() {
        Values::F32(c) => Values::F32(items(c, width, &spans, &reals32)),
        Values::F64(c) => Values::F64(items(c, width, &spans, &fill)),
        Values::I32(c) => Values::I32(items(c, width, &spans, &ints32)),
        Values::I64(c) => Values::I64(items(c, width, &spans, &ints64)),
        Values::Text(t) => Values::Text(t.with_indices(items(t.indices(), 1, &spans, &none))?),
        _ => return Err(Error::invalid("a storage this edit doesn't know")),
    };
    if !ty.array {
        return Attribute::new(ty, values);
    }
    let mut offsets = Vec::with_capacity(from.len() + 1);
    let mut sum = 0_u32;
    offsets.push(0);
    for s in &spans {
        sum += s.as_ref().map_or(0, |s| s.len() as u32);
        offsets.push(sum);
    }
    Attribute::arrays(ty, offsets.into(), values)
}

/// `attributes` with `sources`' attributes and groups of `domain` gathered.
fn gather_domain(
    mut attributes: Attributes,
    domain: Domain,
    sources: &[Source<'_>],
) -> Result<Attributes, Error> {
    let mut types: BTreeMap<&str, AttributeType> = BTreeMap::new();
    let mut groups: BTreeSet<&str> = BTreeSet::new();
    for source in sources {
        for (_, name, a) in source.attributes.iter().filter(|e| e.0 == domain) {
            types.entry(name).or_insert(a.ty());
        }
        let held = source.attributes.groups().filter(|e| e.0 == domain);
        groups.extend(held.map(|g| g.1));
    }
    for (name, ty) in types {
        let column = match domain {
            Domain::Detail => first_detail(sources, name),
            _ if name == names::ID => merged_ids(sources, domain)?,
            _ => gather_column(sources, domain, name, ty)?,
        };
        attributes = attributes.with(domain, name, column)?;
    }
    for name in groups {
        attributes = attributes.with_group(domain, name, gather_group(sources, domain, name))?;
    }
    Ok(attributes)
}

/// The first holder's Detail value of `name`.
fn first_detail(sources: &[Source<'_>], name: &str) -> Attribute {
    (sources.iter())
        .find_map(|s| s.attributes.get(Domain::Detail, name))
        .expect("a type comes from a holder")
        .clone()
}

fn merged_ids(sources: &[Source<'_>], domain: Domain) -> Result<Attribute, Error> {
    let each: Vec<Input<'_>> = (sources.iter())
        .map(|s| match s.attributes.tuples::<i64>(domain, names::ID) {
            Some(ids) => Input::Ids(ids),
            None => Input::Without(s.count),
        })
        .collect();
    let merged =
        ids::merged(&each).ok_or_else(|| Error::refused("the merged ids pass the largest id"))?;
    Attribute::tuples(merged, Meaning::None)
}

fn gather_group(sources: &[Source<'_>], domain: Domain, name: &str) -> Elements {
    let (mut at, mut members) = (0u32, Vec::new());
    for source in sources {
        if let Some(group) = source.attributes.group(domain, name) {
            members.extend(group.iter().map(|m| m + at));
        }
        at += source.count as u32;
    }
    Elements::from_indices(at as usize, members).expect("members of the gathered elements")
}

/// Each input's share of one column: its attribute when it holds it as the
/// column's type, how many elements it gives, and where it goes.
struct Share<'a> {
    held: Option<&'a Attribute>,
    elements: usize,
    at: Xform,
}

fn gather_column(
    sources: &[Source<'_>],
    domain: Domain,
    name: &str,
    ty: AttributeType,
) -> Result<Attribute, Error> {
    let shares: Vec<Share<'_>> = (sources.iter())
        .map(|s| Share {
            held: (s.attributes.get(domain, name)).filter(|a| a.ty() == ty),
            elements: s.count,
            at: s.at,
        })
        .collect();
    let default = default_item(ty);
    match ty.storage {
        Storage::F32 => {
            let item: Vec<f32> = default.iter().map(|&x| x as f32).collect();
            let (offsets, mut out) = flat(&shares, ty, as_f32, &item);
            place(&mut out, &shares, ty, |x| f64::from(*x), |x| x as f32);
            finish(ty, offsets, Values::F32(out.into()))
        }
        Storage::F64 => {
            let (offsets, mut out) = flat(&shares, ty, as_f64, &default);
            place(&mut out, &shares, ty, |x| *x, |x| x);
            finish(ty, offsets, Values::F64(out.into()))
        }
        Storage::I32 => {
            let item = vec![0; default.len()];
            let (offsets, out) = flat(&shares, ty, as_i32, &item);
            finish(ty, offsets, Values::I32(out.into()))
        }
        Storage::I64 => {
            let item = vec![0; default.len()];
            let (offsets, out) = flat(&shares, ty, as_i64, &item);
            finish(ty, offsets, Values::I64(out.into()))
        }
        _ => gather_text(&shares, ty),
    }
}

fn as_f32(v: &Values) -> Option<&Column<f32>> {
    match v {
        Values::F32(c) => Some(c),
        _ => None,
    }
}

fn as_f64(v: &Values) -> Option<&Column<f64>> {
    match v {
        Values::F64(c) => Some(c),
        _ => None,
    }
}

fn as_i32(v: &Values) -> Option<&Column<i32>> {
    match v {
        Values::I32(c) => Some(c),
        _ => None,
    }
}

fn as_i64(v: &Values) -> Option<&Column<i64>> {
    match v {
        Values::I64(c) => Some(c),
        _ => None,
    }
}

/// The attribute of `values`, arrays when `offsets` says.
fn finish(
    ty: AttributeType,
    offsets: Option<Vec<u32>>,
    values: Values,
) -> Result<Attribute, Error> {
    match offsets {
        Some(offsets) => Attribute::arrays(ty, offsets.into(), values),
        None => Attribute::new(ty, values),
    }
}

/// `shares`' values end to end, `item` for each element without one (an
/// empty array in an array column), with the array offsets.
fn flat<'a, T: Copy + 'a>(
    shares: &[Share<'a>],
    ty: AttributeType,
    read: impl Fn(&'a Values) -> Option<&'a Column<T>>,
    item: &[T],
) -> (Option<Vec<u32>>, Vec<T>) {
    let mut out = Vec::new();
    let mut offsets = ty.array.then(|| vec![0u32]);
    let width = usize::from(ty.width);
    for share in shares {
        let held = share.held.and_then(|a| Some((a, read(a.values())?)));
        match (held, &mut offsets) {
            (Some((a, column)), Some(offsets)) => {
                let at = (out.len() / width) as u32;
                let own = a.offsets().expect("an array column has offsets");
                offsets.extend(own.as_slice()[1..].iter().map(|&o| o + at));
                out.extend(column.iter().copied());
            }
            (Some((_, column)), None) => out.extend(column.iter().copied()),
            (None, Some(offsets)) => {
                let last = *offsets.last().expect("offsets start at 0");
                offsets.extend(std::iter::repeat_n(last, share.elements));
            }
            (None, None) => (0..share.elements).for_each(|_| out.extend_from_slice(item)),
        }
    }
    (offsets, out)
}

/// `out`'s items placed: each share's by its placement, as the type means.
fn place<T: Copy>(
    out: &mut [T],
    shares: &[Share<'_>],
    ty: AttributeType,
    to: impl Fn(&T) -> f64,
    from: impl Fn(f64) -> T,
) {
    if !turns(ty) {
        return;
    }
    let width = usize::from(ty.width);
    let mut start = 0;
    let mut item = vec![0.0; width];
    for share in shares {
        let end = start + share.elements * width;
        if share.held.is_some() && share.at != Xform::IDENTITY {
            let placing = Placing::new(ty.meaning, share.at);
            for chunk in out[start..end].chunks_exact_mut(width) {
                item.iter_mut().zip(&*chunk).for_each(|(i, x)| *i = to(x));
                placing.apply(&mut item);
                chunk.iter_mut().zip(&item).for_each(|(x, i)| *x = from(*i));
            }
        }
        start = end;
    }
}

/// Text of `shares`, its tables joined: a string once, whichever input gave it.
fn gather_text(shares: &[Share<'_>], ty: AttributeType) -> Result<Attribute, Error> {
    let mut table: Vec<Arc<str>> = Vec::new();
    let mut seen: HashMap<Arc<str>, u32> = HashMap::new();
    let mut indices = Vec::new();
    let mut offsets = ty.array.then(|| vec![0u32]);
    for share in shares {
        let text = share.held.map(Attribute::values);
        let Some(Values::Text(text)) = text else {
            match &mut offsets {
                Some(o) => {
                    let last = *o.last().expect("offsets start at 0");
                    o.extend(std::iter::repeat_n(last, share.elements));
                }
                None => indices.extend(std::iter::repeat_n(Text::NONE, share.elements)),
            }
            continue;
        };
        let map: Vec<u32> = (text.table().iter())
            .map(|s| {
                *seen.entry(s.clone()).or_insert_with(|| {
                    table.push(s.clone());
                    table.len() as u32 - 1
                })
            })
            .collect();
        if let (Some(o), Some(own)) = (&mut offsets, share.held.and_then(Attribute::offsets)) {
            let at = indices.len() as u32;
            o.extend(own.as_slice()[1..].iter().map(|&k| k + at));
        }
        let mapped = text.indices().iter();
        indices.extend(mapped.map(|&i| map.get(i as usize).copied().unwrap_or(Text::NONE)));
    }
    let text = Text::new(indices.into(), table.into())?;
    match offsets {
        Some(offsets) => Attribute::arrays(ty, offsets.into(), Values::Text(text)),
        None => Ok(Attribute::text(text)),
    }
}

#[cfg(test)]
mod tests;
