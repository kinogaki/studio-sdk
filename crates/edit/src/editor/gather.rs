//! [`gather`]: values into one, each at a placement: what a Merge and a
//! realize of instances make, the one rule for both.
use super::carry::default_item;
use crate::ids::{self, Input};
use kinogaki_geometry::{
    Attribute, AttributeType, Attributes, Column, CurveData, Domain, Elements, Error, Geometry,
    Incidence, Knots, Meaning, Primitives, Storage, Text, Values, Xform, names,
};
use std::collections::{BTreeMap, HashMap};
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
        attributes = gather_domain(attributes, domain, values)?;
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

/// `attributes` with `values`' attributes and groups of `domain` gathered.
fn gather_domain(
    mut attributes: Attributes,
    domain: Domain,
    values: &[Placed<'_>],
) -> Result<Attributes, Error> {
    let mut types: BTreeMap<&str, AttributeType> = BTreeMap::new();
    let mut groups: BTreeMap<&str, ()> = BTreeMap::new();
    for Placed { value, .. } in values {
        let held = value.attributes();
        for (d, name, a) in held.iter().filter(|e| e.0 == domain) {
            debug_assert_eq!(d, domain);
            types.entry(name).or_insert(a.ty());
        }
        for (_, name, _) in held.groups().filter(|e| e.0 == domain) {
            groups.insert(name, ());
        }
    }
    for (name, ty) in types {
        let column = match domain {
            Domain::Detail => first_detail(values, name),
            _ if name == names::ID => merged_ids(values, domain)?,
            _ => gather_column(values, domain, name, ty)?,
        };
        attributes = attributes.with(domain, name, column)?;
    }
    for name in groups.into_keys() {
        attributes = attributes.with_group(domain, name, gather_group(values, domain, name))?;
    }
    Ok(attributes)
}

/// The first holder's Detail value of `name`.
fn first_detail(values: &[Placed<'_>], name: &str) -> Attribute {
    (values.iter())
        .find_map(|v| v.value.attributes().get(Domain::Detail, name))
        .expect("a type comes from a holder")
        .clone()
}

fn merged_ids(values: &[Placed<'_>], domain: Domain) -> Result<Attribute, Error> {
    let held: Vec<_> = values.iter().map(|v| v.value.ids(domain)).collect();
    let each: Vec<Input<'_>> = (held.iter().zip(values))
        .map(|(ids, v)| match ids {
            Some(ids) => Input::Ids(ids.as_slice()),
            None => Input::Without(v.value.count(domain).unwrap_or(0)),
        })
        .collect();
    let merged =
        ids::merged(&each).ok_or_else(|| Error::refused("the merged ids pass the largest id"))?;
    Attribute::tuples(merged, Meaning::None)
}

fn gather_group(values: &[Placed<'_>], domain: Domain, name: &str) -> Elements {
    let (mut at, mut members) = (0u32, Vec::new());
    for Placed { value, .. } in values {
        if let Some(group) = value.attributes().group(domain, name) {
            members.extend(group.iter().map(|m| m + at));
        }
        at += value.count(domain).unwrap_or(0) as u32;
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
    values: &[Placed<'_>],
    domain: Domain,
    name: &str,
    ty: AttributeType,
) -> Result<Attribute, Error> {
    let shares: Vec<Share<'_>> = (values.iter())
        .map(|v| Share {
            held: (v.value.attributes().get(domain, name)).filter(|a| a.ty() == ty),
            elements: v.value.count(domain).unwrap_or(0),
            at: v.at,
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
    let turns = matches!(
        ty.meaning,
        Meaning::Point | Meaning::Vector | Meaning::Normal | Meaning::Rotation | Meaning::Transform
    );
    if !turns || ty.array {
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

/// How one placement acts on the values of one meaning.
enum Placing {
    Point(Xform),
    Vector([[f64; 3]; 3]),
    Normal([[f64; 3]; 3]),
    Rotation([f64; 4]),
    Transform(Xform),
}

impl Placing {
    fn new(meaning: Meaning, at: Xform) -> Self {
        let linear = |x: &Xform| {
            let r = x.rows();
            [
                [r[0][0], r[0][1], r[0][2]],
                [r[1][0], r[1][1], r[1][2]],
                [r[2][0], r[2][1], r[2][2]],
            ]
        };
        match meaning {
            Meaning::Point => Self::Point(at),
            Meaning::Normal => {
                // The inverse's transpose: rows of the inverse read as columns.
                let inverse = at.inverse().map_or_else(|| linear(&at), |i| linear(&i));
                Self::Normal(std::array::from_fn(|i| {
                    std::array::from_fn(|j| inverse[j][i])
                }))
            }
            Meaning::Rotation => Self::Rotation(rotation_of(&at)),
            Meaning::Transform => Self::Transform(at),
            _ => Self::Vector(linear(&at)),
        }
    }

    fn apply(&self, item: &mut [f64]) {
        let turned = |m: &[[f64; 3]; 3], v: &[f64]| -> [f64; 3] {
            std::array::from_fn(|i| m[i][0] * v[0] + m[i][1] * v[1] + m[i][2] * v[2])
        };
        match self {
            Self::Point(at) => {
                let p = at
                    .apply_point([item[0], item[1], item[2]].into())
                    .to_array();
                item.copy_from_slice(&p);
            }
            Self::Vector(m) => item.copy_from_slice(&turned(m, item)),
            Self::Normal(m) => {
                let n = turned(m, item);
                let length = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
                let scale = if length > 0.0 { 1.0 / length } else { 1.0 };
                item.copy_from_slice(&n.map(|c| c * scale));
            }
            Self::Rotation(q) => {
                let q2 = [item[0], item[1], item[2], item[3]];
                item.copy_from_slice(&multiplied(*q, q2));
            }
            Self::Transform(at) => composed(at, item),
        }
    }
}

/// `m`, row-major of side 3 or 4, after `at`.
fn composed(at: &Xform, m: &mut [f64]) {
    let side = if m.len() == 16 { 4 } else { 3 };
    let rows = at.rows();
    let before = m.to_vec();
    for r in 0..3 {
        for c in 0..side {
            let by = |k: usize| rows[r][k] * before[k * side + c];
            m[r * side + c] =
                by(0) + by(1) + by(2) + if side == 4 && c == 3 { rows[r][3] } else { 0.0 };
        }
    }
}

/// The unit quaternion (x y z w) of `at`'s rotation, its scales taken out.
fn rotation_of(at: &Xform) -> [f64; 4] {
    let [x, y, z] = at.axes().map(|a| {
        let l = a.length();
        if l > 0.0 { a / l } else { a }
    });
    // Shepperd's method over the matrix whose columns are the axes.
    let (m00, m11, m22) = (x.x, y.y, z.z);
    let trace = m00 + m11 + m22;
    if trace > 0.0 {
        let s = (trace + 1.0).sqrt() * 2.0;
        [(y.z - z.y) / s, (z.x - x.z) / s, (x.y - y.x) / s, 0.25 * s]
    } else if m00 > m11 && m00 > m22 {
        let s = (1.0 + m00 - m11 - m22).sqrt() * 2.0;
        [0.25 * s, (y.x + x.y) / s, (z.x + x.z) / s, (y.z - z.y) / s]
    } else if m11 > m22 {
        let s = (1.0 + m11 - m00 - m22).sqrt() * 2.0;
        [(y.x + x.y) / s, 0.25 * s, (z.y + y.z) / s, (z.x - x.z) / s]
    } else {
        let s = (1.0 + m22 - m00 - m11).sqrt() * 2.0;
        [(z.x + x.z) / s, (z.y + y.z) / s, 0.25 * s, (x.y - y.x) / s]
    }
}

/// `a` after `b`, both x y z w.
fn multiplied(a: [f64; 4], b: [f64; 4]) -> [f64; 4] {
    let [ax, ay, az, aw] = a;
    let [bx, by, bz, bw] = b;
    [
        aw * bx + ax * bw + ay * bz - az * by,
        aw * by - ax * bz + ay * bw + az * bx,
        aw * bz + ax * by - ay * bx + az * bw,
        aw * bw - ax * bx - ay * by - az * bz,
    ]
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
