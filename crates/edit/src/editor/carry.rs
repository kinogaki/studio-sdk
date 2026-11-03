//! How attributes and groups follow an edit: the declared rule of each
//! attribute type, and one [`Layout`] applied to every column of a domain.
use super::layout::{Blends, Layout, Run, Source, strongest};
use crate::ids;
use kinogaki_geometry::{
    Attribute, AttributeType, Column, ColumnWriter, Elements, Error, Meaning, Text, Values, names,
};

/// How a new element's value comes from its sources.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rule {
    /// The weighted mean of each component: positions, vectors, colors,
    /// texture coordinates, matrices and plain reals.
    Lerp,
    /// [`Lerp`](Rule::Lerp), then scaled to unit length: normals.
    Normalize,
    /// Spherical interpolation, along the shorter arc: rotations.
    Slerp,
    /// The value of the source with the largest weight (the first among
    /// equals): integers, text and arrays of anything.
    Strongest,
}

/// The rule values of `ty` blend by.
///
/// ```
/// use kinogaki_edit::editor::{Rule, rule};
/// use kinogaki_geometry::{AttributeType, Meaning, Storage};
///
/// assert_eq!(rule(AttributeType::new(Storage::F32, 3, Meaning::Normal)), Rule::Normalize);
/// assert_eq!(rule(AttributeType::new(Storage::F64, 4, Meaning::Rotation)), Rule::Slerp);
/// assert_eq!(rule(AttributeType::new(Storage::F32, 2, Meaning::TextureCoord)), Rule::Lerp);
/// assert_eq!(rule(AttributeType::new(Storage::I32, 1, Meaning::None)), Rule::Strongest);
/// assert_eq!(rule(AttributeType::TEXT), Rule::Strongest);
/// ```
pub fn rule(ty: AttributeType) -> Rule {
    if ty.array || !ty.storage.is_real() {
        return Rule::Strongest;
    }
    match ty.meaning {
        Meaning::Normal => Rule::Normalize,
        Meaning::Rotation => Rule::Slerp,
        _ => Rule::Lerp,
    }
}

/// A new element's value when it has no sources: zero and no text, but
/// white for a color and the identity for a rotation or a matrix, as a
/// Merge fills what an input lacks (docs/ATTRIBUTES.md).
fn default_item(ty: AttributeType) -> Vec<f64> {
    let width = usize::from(ty.width);
    let mut item = vec![0.0; width];
    match ty.meaning {
        Meaning::Color => item.fill(1.0),
        Meaning::Rotation => item[3] = 1.0,
        Meaning::Transform => {
            let side = if width == 16 { 4 } else { 3 };
            (0..side).for_each(|i| item[i * side + i] = 1.0);
        }
        _ => {}
    }
    item
}

/// A real storage's type.
trait Real: Copy {
    fn to_f64(self) -> f64;
    fn from_f64(x: f64) -> Self;
}

impl Real for f32 {
    fn to_f64(self) -> f64 {
        f64::from(self)
    }
    fn from_f64(x: f64) -> Self {
        x as f32
    }
}

impl Real for f64 {
    fn to_f64(self) -> f64 {
        self
    }
    fn from_f64(x: f64) -> Self {
        x
    }
}

/// Every element of `layout` from `values`, `width` values an element: a
/// kept run shares its cells, `new` writes each new element.
fn rows<T: Copy>(
    values: &Column<T>,
    width: usize,
    layout: &Layout,
    blends: &Blends,
    mut new: impl FnMut(&[Source], &mut ColumnWriter<T>),
) -> Column<T> {
    let mut out = ColumnWriter::new();
    for run in layout.runs() {
        match run {
            Run::Kept(range) => {
                let (start, end) = (range.start as usize, range.end as usize);
                out.keep(values, start * width..end * width);
            }
            Run::New(span) => new(blends.of(*span), &mut out),
        }
    }
    out.finish()
}

/// Blends `sources`' items of `values` into `out` by `rule`.
fn blend_real<T: Real>(
    values: &Column<T>,
    width: usize,
    rule: Rule,
    sources: &[Source],
    default: &[f64],
    out: &mut ColumnWriter<T>,
) {
    let item = |element: u32| {
        (element as usize * width..(element as usize + 1) * width).map(|at| values[at])
    };
    let total: f64 = sources.iter().map(|s| s.1).sum();
    // One source (or one with all the weight) is its value, every bit.
    let single = (sources.iter()).find(|s| s.1 == total && total > 0.0);
    if let Some(&(element, _)) = single {
        item(element).for_each(|x| out.push(x));
        return;
    }
    if total <= 0.0 {
        default.iter().for_each(|&x| out.push(T::from_f64(x)));
        return;
    }
    let mut mean = vec![0.0; width];
    match rule {
        Rule::Slerp => {
            let mut sum = 0.0;
            for &(element, weight) in sources.iter().filter(|s| s.1 > 0.0) {
                sum += weight;
                let q: Vec<f64> = item(element).map(Real::to_f64).collect();
                if sum == weight {
                    mean = q;
                } else {
                    slerp(&mut mean, &q, weight / sum);
                }
            }
        }
        _ => {
            for &(element, weight) in sources {
                for (m, x) in mean.iter_mut().zip(item(element)) {
                    *m += x.to_f64() * (weight / total);
                }
            }
            if rule == Rule::Normalize {
                normalize(&mut mean);
            }
        }
    }
    mean.iter().for_each(|&x| out.push(T::from_f64(x)));
}

fn normalize(v: &mut [f64]) {
    let length = v.iter().map(|x| x * x).sum::<f64>().sqrt();
    if length > 0.0 {
        v.iter_mut().for_each(|x| *x /= length);
    }
}

/// `q` turned `t` of the way to `to`, along the shorter arc.
fn slerp(q: &mut [f64], to: &[f64], t: f64) {
    let dot: f64 = q.iter().zip(to).map(|(a, b)| a * b).sum();
    let sign = if dot < 0.0 { -1.0 } else { 1.0 };
    let cos = (dot * sign).min(1.0);
    let (a, b) = if cos > 0.9995 {
        // Nearly the same: a line is as good, and doesn't divide by zero.
        (1.0 - t, t)
    } else {
        let angle = cos.acos();
        let sin = angle.sin();
        (((1.0 - t) * angle).sin() / sin, (t * angle).sin() / sin)
    };
    for (x, y) in q.iter_mut().zip(to) {
        *x = *x * a + y * b * sign;
    }
    normalize(q);
}

/// The strongest source's item of `values`, or `default` without sources.
fn strongest_item<T: Copy>(
    values: &Column<T>,
    width: usize,
    sources: &[Source],
    default: T,
    out: &mut ColumnWriter<T>,
) {
    match strongest(sources) {
        Some(e) => {
            (e as usize * width..(e as usize + 1) * width).for_each(|at| out.push(values[at]))
        }
        None => (0..width).for_each(|_| out.push(default)),
    }
}

/// `offsets` and `items` (arrays of `width` values an item) laid out by
/// `layout`: each new element holds its strongest source's array.
fn arrays<T: Copy>(
    offsets: &[u32],
    items: &[T],
    width: usize,
    layout: &Layout,
    blends: &Blends,
) -> (Vec<u32>, Vec<T>) {
    let mut new_offsets = Vec::with_capacity(layout.len() + 1);
    new_offsets.push(0u32);
    let mut out = Vec::new();
    let mut push = |range: std::ops::Range<usize>, offsets_out: &mut Vec<u32>| {
        for e in range {
            let (a, b) = (offsets[e] as usize, offsets[e + 1] as usize);
            out.extend_from_slice(&items[a * width..b * width]);
            offsets_out.push((out.len() / width) as u32);
        }
    };
    for run in layout.runs() {
        match run {
            Run::Kept(range) => {
                push(range.start as usize..range.end as usize, &mut new_offsets);
            }
            Run::New(span) => match strongest(blends.of(*span)) {
                Some(e) => push(e as usize..e as usize + 1, &mut new_offsets),
                None => {
                    let last = *new_offsets.last().expect("offsets start at 0");
                    new_offsets.push(last);
                }
            },
        }
    }
    (new_offsets, out)
}

/// `values` of type `ty` laid out by `layout`.
fn carried_values(
    values: &Values,
    ty: AttributeType,
    layout: &Layout,
    blends: &Blends,
) -> Result<Values, Error> {
    let width = usize::from(ty.width);
    let rule = rule(ty);
    let default = default_item(ty);
    Ok(match values {
        Values::F32(c) => Values::F32(rows(c, width, layout, blends, |s, out| {
            blend_real(c, width, rule, s, &default, out);
        })),
        Values::F64(c) => Values::F64(rows(c, width, layout, blends, |s, out| {
            blend_real(c, width, rule, s, &default, out);
        })),
        Values::I32(c) => Values::I32(rows(c, width, layout, blends, |s, out| {
            strongest_item(c, width, s, 0, out);
        })),
        Values::I64(c) => Values::I64(rows(c, width, layout, blends, |s, out| {
            strongest_item(c, width, s, 0, out);
        })),
        Values::Text(t) => {
            let indices = rows(t.indices(), 1, layout, blends, |s, out| {
                strongest_item(t.indices(), 1, s, Text::NONE, out);
            });
            Values::Text(t.with_indices(indices)?)
        }
        other => {
            return Err(Error::refused(format!(
                "no rule carries {} values",
                other.storage().name()
            )));
        }
    })
}

/// `attribute` (named `name`) laid out by `layout`: kept elements keep
/// their values; a new element's value comes by the attribute's [`rule`],
/// and an `id` is fresh.
pub(super) fn carried_attribute(
    name: &str,
    attribute: &Attribute,
    layout: &Layout,
    blends: &Blends,
) -> Result<Attribute, Error> {
    let ty = attribute.ty();
    if let (names::ID, Values::I64(old)) = (name, attribute.values()) {
        return fresh_ids(old, attribute, layout);
    }
    if let Some(offsets) = attribute.offsets() {
        let width = usize::from(ty.width);
        return match attribute.values() {
            Values::F32(c) => {
                let (o, v) = arrays(offsets.as_slice(), c.as_slice(), width, layout, blends);
                Attribute::arrays(ty, o.into(), Values::F32(v.into()))
            }
            Values::F64(c) => {
                let (o, v) = arrays(offsets.as_slice(), c.as_slice(), width, layout, blends);
                Attribute::arrays(ty, o.into(), Values::F64(v.into()))
            }
            Values::I32(c) => {
                let (o, v) = arrays(offsets.as_slice(), c.as_slice(), width, layout, blends);
                Attribute::arrays(ty, o.into(), Values::I32(v.into()))
            }
            Values::I64(c) => {
                let (o, v) = arrays(offsets.as_slice(), c.as_slice(), width, layout, blends);
                Attribute::arrays(ty, o.into(), Values::I64(v.into()))
            }
            Values::Text(t) => {
                let (o, v) = arrays(
                    offsets.as_slice(),
                    t.indices().as_slice(),
                    1,
                    layout,
                    blends,
                );
                let text = Text::new(v.into(), t.table().clone())?;
                Attribute::arrays(ty, o.into(), Values::Text(text))
            }
            other => Err(Error::refused(format!(
                "no rule carries {} values",
                other.storage().name()
            ))),
        };
    }
    match carried_values(attribute.values(), ty, layout, blends)? {
        Values::Text(text) => Ok(Attribute::text(text)),
        values => attribute.with_values(values),
    }
}

/// Kept elements' ids and fresh ones for the rest.
fn fresh_ids(
    old: &Column<i64>,
    attribute: &Attribute,
    layout: &Layout,
) -> Result<Attribute, Error> {
    let mut next = ids::fresh(old, layout.added())
        .ok_or_else(|| Error::refused("the new ids pass the largest id"))?;
    let mut out = ColumnWriter::new();
    for run in layout.runs() {
        match run {
            Run::Kept(range) => out.keep(old, range.start as usize..range.end as usize),
            Run::New(_) => out.push(next.next().expect("one fresh id a new element")),
        }
    }
    attribute.with_values(Values::I64(out.finish()))
}

/// `group` laid out by `layout`: kept elements stay members; a new element
/// is one when its strongest source is.
pub(super) fn carried_group(group: &Elements, layout: &Layout, blends: &Blends) -> Elements {
    let members: Vec<u32> = group.iter().collect();
    let mut out = Vec::new();
    let mut at = 0u32;
    for run in layout.runs() {
        match run {
            Run::Kept(range) => {
                let from = members.partition_point(|&m| m < range.start);
                let to = members.partition_point(|&m| m < range.end);
                out.extend(members[from..to].iter().map(|&m| at + (m - range.start)));
                at += range.len() as u32;
            }
            Run::New(span) => {
                if strongest(blends.of(*span)).is_some_and(|e| group.contains(e)) {
                    out.push(at);
                }
                at += 1;
            }
        }
    }
    Elements::from_indices(layout.len(), out).expect("members of the laid out elements")
}
