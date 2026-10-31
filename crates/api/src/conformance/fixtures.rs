//! [`Fixtures`]: hand-built standard values covering every shape the
//! types allow and every attribute kind on every class.
use super::Producer;
use crate::data::Data;
use crate::standard;
use kinogaki_geometry::{
    Attribute, Attributes, Basis, Class, Column, Curves, Group, Polygons, Text, Topology, TypeInfo,
    names,
};

/// The SDK's fixtures: polygons (a quad grid; mixed triangles, quads and
/// n-gons; concave polygons; polylines only; polygons and polylines
/// sharing points; points only; every attribute kind on every class;
/// groups with `.hidden` and `sharp`) and curves of every basis. With
/// `large`, a 1000 × 1000 quad grid too (1M faces).
#[derive(Clone, Copy, Debug, Default)]
pub struct Fixtures {
    /// Adds the 1M-face grid.
    pub large: bool,
}

impl Fixtures {
    /// The polygons fixtures, by name.
    pub fn polygons(&self) -> Vec<(String, Polygons)> {
        let mut out = vec![
            ("quad grid".to_owned(), grid(4)),
            ("mixed polygons".to_owned(), mixed()),
            ("concave polygons".to_owned(), concave()),
            ("polylines only".to_owned(), polylines_only()),
            ("polygons and polylines".to_owned(), shared_points()),
            ("points only".to_owned(), points_only()),
            ("every attribute".to_owned(), every_attribute()),
            ("groups".to_owned(), groups()),
            ("empty".to_owned(), Polygons::empty()),
        ];
        if self.large {
            out.push(("1M-face grid".to_owned(), grid(1000)));
        }
        out
    }

    /// The curves fixtures, by name: one value of each basis.
    pub fn curves(&self) -> Vec<(String, Curves)> {
        let points = |n: usize| -> Column<[f64; 3]> {
            (0..n)
                .map(|i| [i as f64, (i % 2) as f64, 0.0])
                .collect::<Vec<_>>()
                .into()
        };
        let build = |b: kinogaki_geometry::CurvesBuilder| b.build().expect("a valid fixture");
        let widths = Attributes::new()
            .with(Class::Point, names::WIDTH, Attribute::float(vec![0.1; 7]))
            .expect("width");
        vec![
            (
                "linear curves".into(),
                build(
                    Curves::builder(Basis::Linear, points(7), vec![0, 3, 7].into())
                        .periodic(Group::from_indices(2, [1]).expect("periodic"))
                        .attributes(widths),
                ),
            ),
            (
                "bezier curve".into(),
                build(Curves::builder(Basis::Bezier, points(7), vec![0, 7].into()).degree(3)),
            ),
            (
                "bspline curves".into(),
                build(Curves::builder(
                    Basis::BSpline,
                    points(9),
                    vec![0, 4, 9].into(),
                )),
            ),
            (
                "catmull-rom curve".into(),
                build(Curves::builder(
                    Basis::CatmullRom,
                    points(5),
                    vec![0, 5].into(),
                )),
            ),
            (
                "nurbs curve".into(),
                build(
                    Curves::builder(Basis::Nurbs, points(4), vec![0, 4].into())
                        .degree(2)
                        .knots(
                            vec![0, 7].into(),
                            vec![0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0].into(),
                        )
                        .weights(vec![1.0, 0.7, 0.7, 1.0].into()),
                ),
            ),
            (
                "indexed curves".into(),
                build(
                    Curves::builder(Basis::Linear, points(4), vec![0, 3, 5].into())
                        .points(vec![0, 1, 2, 2, 3].into()),
                ),
            ),
            (
                "polylines as curves".into(),
                Curves::from_polylines(&shared_points()),
            ),
        ]
    }
}

impl Producer for Fixtures {
    fn name(&self) -> &str {
        "SDK fixtures"
    }
    fn fixtures(&self) -> Vec<(String, Data)> {
        let polygons = self
            .polygons()
            .into_iter()
            .map(|(n, p)| (n, standard::polygons().wrap(p)));
        let curves = self
            .curves()
            .into_iter()
            .map(|(n, c)| (n, standard::curves().wrap(c)));
        polygons.chain(curves).collect()
    }
}

/// Primitives as lists of points, the first `polygons` polygons.
fn value(positions: Vec<[f64; 3]>, prims: &[Vec<u32>], polygons: usize) -> Polygons {
    let mut offsets = vec![0];
    let mut vertices = Vec::new();
    for p in prims {
        vertices.extend_from_slice(p);
        offsets.push(vertices.len() as u32);
    }
    let t = Topology::new(positions.len(), offsets.into(), vertices.into(), polygons)
        .expect("a valid fixture");
    Polygons::new(positions.into(), t, Attributes::new()).expect("a valid fixture")
}

/// A `side` × `side` grid of quads.
fn grid(side: u32) -> Polygons {
    let row = side + 1;
    let positions: Vec<[f64; 3]> = (0..row * row)
        .map(|i| [(i % row) as f64, (i / row) as f64, 0.0])
        .collect();
    let mut offsets = Vec::with_capacity((side * side + 1) as usize);
    let mut vertices = Vec::with_capacity((side * side * 4) as usize);
    offsets.push(0);
    for y in 0..side {
        for x in 0..side {
            let p = y * row + x;
            vertices.extend([p, p + 1, p + row + 1, p + row]);
            offsets.push(vertices.len() as u32);
        }
    }
    let faces = offsets.len() - 1;
    let t = Topology::new((row * row) as usize, offsets.into(), vertices.into(), faces)
        .expect("a grid");
    Polygons::new(positions.into(), t, Attributes::new()).expect("a grid")
}

fn mixed() -> Polygons {
    let p = vec![
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [2.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [1.0, 1.0, 0.0],
        [2.0, 1.0, 0.0],
        [3.0, 0.5, 0.0],
        [2.5, 2.0, 0.0],
    ];
    value(
        p,
        &[
            vec![0, 1, 4],
            vec![0, 4, 3],
            vec![1, 2, 5, 4],
            vec![2, 6, 7, 5],
        ],
        4,
    )
}

fn concave() -> Polygons {
    // An L and an arrow head.
    let p = vec![
        [0.0, 0.0, 0.0],
        [2.0, 0.0, 0.0],
        [2.0, 1.0, 0.0],
        [1.0, 1.0, 0.0],
        [1.0, 2.0, 0.0],
        [0.0, 2.0, 0.0],
        [3.0, 0.0, 0.0],
        [5.0, 1.0, 0.0],
        [3.0, 2.0, 0.0],
        [3.5, 1.0, 0.0],
    ];
    value(p, &[vec![0, 1, 2, 3, 4, 5], vec![6, 7, 8, 9]], 2)
}

fn polylines_only() -> Polygons {
    let p = (0..6).map(|i| [i as f64, 0.0, (i % 2) as f64]).collect();
    value(p, &[vec![0, 1, 2], vec![3, 4, 5, 3]], 0)
}

fn shared_points() -> Polygons {
    let p = vec![
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [1.0, 1.0, 0.0],
        [0.0, 1.0, 0.0],
        [2.0, 2.0, 0.0],
    ];
    value(p, &[vec![0, 1, 2, 3], vec![2, 4], vec![3, 2, 1]], 1)
}

fn points_only() -> Polygons {
    value(vec![[0.0; 3], [1.0, 2.0, 3.0], [-1.0, 0.5, 2.0]], &[], 0)
}

fn every_attribute() -> Polygons {
    let base = mixed();
    let mut a = Attributes::new();
    for class in [
        Class::Detail,
        Class::Primitive,
        Class::Piece,
        Class::Edge,
        Class::Point,
        Class::Vertex,
    ] {
        let n = base.count(class).expect("a polygons class");
        let floats: Vec<f64> = (0..n).map(|i| i as f64 * 0.5).collect();
        let ints: Vec<i64> = (0..n as i64).map(|i| i - 2).collect();
        let vectors: Vec<[f64; 3]> = (0..n).map(|i| [i as f64, 1.0, -1.0]).collect();
        let texts: Vec<&str> = (0..n).map(|i| ["a", "", "b"][i % 3]).collect();
        a = a
            .with(class, "f", Attribute::float(floats))
            .and_then(|a| a.with(class, "i", Attribute::integer(ints)))
            .and_then(|a| {
                a.with(
                    class,
                    "v",
                    Attribute::vector(vectors.clone(), TypeInfo::Vector),
                )
            })
            .and_then(|a| a.with(class, "c", Attribute::vector(vectors, TypeInfo::Color)))
            .and_then(|a| a.with(class, "t", Attribute::text(Text::from_strs(texts))))
            .expect("fixture attributes");
    }
    let n = base.count(Class::Primitive).expect("primitives");
    a = a
        .with(
            Class::Primitive,
            names::MATERIAL,
            Attribute::text(Text::from_strs((0..n).map(|i| ["/M/A", "/M/B"][i % 2]))),
        )
        .expect("material");
    base.with_attributes(a).expect("sized")
}

fn groups() -> Polygons {
    let base = grid(3);
    let edges = base.count(Class::Edge).expect("edges");
    let a = Attributes::new()
        .with_group(
            Class::Primitive,
            names::HIDDEN,
            Group::from_indices(9, [4]).expect("hidden"),
        )
        .and_then(|a| {
            a.with_group(
                Class::Edge,
                names::SHARP,
                Group::from_indices(edges, [0, 1, 2]).expect("sharp"),
            )
        })
        .and_then(|a| {
            a.with_group(
                Class::Point,
                "corners",
                Group::from_indices(16, [0, 3, 12, 15]).expect("corners"),
            )
        })
        .expect("groups");
    base.with_attributes(a).expect("sized")
}
