//! The values the editor's tests edit.
use kinogaki_edit::editor::Corner;
use kinogaki_geometry::{
    Attribute, AttributeType, Attributes, Domain, Elements, Geometry, Meaning, Primitives, Storage,
    Text, Tuple, Values, names,
};

/// `n` × `m` quads: point `(i, j)` is `j * (n + 1) + i`, at `(i, j, 0)`.
pub fn sheet(n: u32, m: u32) -> Geometry {
    let at = |i: u32, j: u32| j * (n + 1) + i;
    let mut corners = Vec::new();
    for j in 0..m {
        for i in 0..n {
            corners.extend([at(i, j), at(i + 1, j), at(i + 1, j + 1), at(i, j + 1)]);
        }
    }
    let offsets: Vec<u32> = (0..=n * m).map(|f| 4 * f).collect();
    let points = ((n + 1) * (m + 1)) as usize;
    let table = Primitives::of_polygons(points, offsets.into(), corners.into()).unwrap();
    let positions: Vec<[f64; 3]> = (0..=m)
        .flat_map(|j| (0..=n).map(move |i| [f64::from(i), f64::from(j), 0.0]))
        .collect();
    Geometry::new(positions.into(), table, Attributes::new()).unwrap()
}

fn text_of(count: usize, f: impl Fn(usize) -> String) -> Attribute {
    let values: Vec<String> = (0..count).map(f).collect();
    Attribute::text(Text::from_strs(values.iter().map(String::as_str)))
}

fn tuples<T: Tuple>(values: impl IntoIterator<Item = T>, meaning: Meaning) -> Attribute {
    Attribute::tuples(values.into_iter().collect::<Vec<_>>(), meaning).unwrap()
}

/// `sheet` with, on points, ids from 100, a position-like `rest`, `N`,
/// `orient`, `Cd`, an integer `tag`, a text `label` and a list `tags`; on
/// primitives ids from 500, `Cd`, an integer and `name`; on vertices `uv`
/// and an integer; on edges a `crease`; a detail value; and a group of each.
pub fn ruled(n: u32, m: u32) -> Geometry {
    let value = sheet(n, m);
    let points = value.count(Domain::Point).unwrap();
    let primitives = value.count(Domain::Primitive).unwrap();
    let vertices = value.count(Domain::Vertex).unwrap();
    let edges = value.count(Domain::Edge).unwrap();
    let mut a = Attributes::new();
    let mut add = |domain, name: &str, attribute: Attribute| {
        a = a.with(domain, name, attribute).unwrap();
    };
    let ids = |from: i64, count: usize| tuples((0..count as i64).map(|i| from + i), Meaning::None);
    let none = Meaning::None;
    add(Domain::Point, names::ID, ids(100, points));
    add(
        Domain::Point,
        names::REST,
        tuples(value.positions().iter().copied(), Meaning::Point),
    );
    let normal = |i: usize| match i % 2 {
        0 => [1.0f32, 0.0, 0.0],
        _ => [0.0, 1.0, 0.0],
    };
    add(
        Domain::Point,
        names::N,
        tuples((0..points).map(normal), Meaning::Normal),
    );
    let half_turn = |i: usize| match i % 2 {
        0 => [0.0f32, 0.0, 0.0, 1.0],
        _ => [0.0, 0.0, 1.0, 0.0],
    };
    add(
        Domain::Point,
        names::ORIENT,
        tuples((0..points).map(half_turn), Meaning::Rotation),
    );
    add(
        Domain::Point,
        names::CD,
        tuples((0..points).map(|i| [i as f32, 0.0, 0.0]), Meaning::Color),
    );
    add(Domain::Point, "tag", tuples(0..points as i32, none));
    add(Domain::Point, "label", text_of(points, |i| format!("p{i}")));
    let list = AttributeType {
        array: true,
        ..AttributeType::new(Storage::I32, 1, Meaning::None)
    };
    let offsets: Vec<u32> = (0..=points as u32).map(|i| i * 2).collect();
    let items: Vec<i32> = (0..2 * points as i32).collect();
    add(
        Domain::Point,
        "tags",
        Attribute::arrays(list, offsets.into(), Values::I32(items.into())).unwrap(),
    );
    add(Domain::Primitive, names::ID, ids(500, primitives));
    add(
        Domain::Primitive,
        names::CD,
        tuples(
            (0..primitives).map(|i| [0.0f32, i as f32, 0.0]),
            Meaning::Color,
        ),
    );
    add(
        Domain::Primitive,
        "count",
        tuples(0..primitives as i64, none),
    );
    add(
        Domain::Primitive,
        names::NAME,
        text_of(primitives, |i| format!("f{i}")),
    );
    add(
        Domain::Vertex,
        names::UV,
        tuples(
            (0..vertices).map(|i| [i as f32, 1.0]),
            Meaning::TextureCoord,
        ),
    );
    add(Domain::Vertex, "corner", tuples(0..vertices as i32, none));
    add(
        Domain::Edge,
        "crease",
        tuples((0..edges).map(|i| i as f32), none),
    );
    add(
        Domain::Detail,
        "tint",
        tuples([[0.5f32, 0.5, 0.5]], Meaning::Color),
    );
    let group = |count: usize| Elements::from_fn(count, |i| i % 2 == 0);
    let a = a
        .with_group(Domain::Point, "even", group(points))
        .and_then(|a| a.with_group(Domain::Primitive, "even", group(primitives)))
        .and_then(|a| a.with_group(Domain::Vertex, "even", group(vertices)))
        .and_then(|a| a.with_group(Domain::Edge, "sharp", group(edges)))
        .unwrap();
    value.with_attributes(a).unwrap()
}

pub fn column<T: Tuple + Copy>(v: &Geometry, d: Domain, name: &str) -> Vec<T> {
    v.attributes().tuples::<T>(d, name).unwrap().to_vec()
}

pub fn key(v: &Geometry, d: Domain, name: &str) -> u64 {
    v.attributes().get(d, name).unwrap().values().key()
}

pub fn members(v: &Geometry, d: Domain, name: &str) -> Vec<u32> {
    v.attributes().group(d, name).unwrap().iter().collect()
}

pub fn corners(points: &[u32]) -> Vec<Corner<'static>> {
    points
        .iter()
        .map(|&point| Corner { point, from: &[] })
        .collect()
}
