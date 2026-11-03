//! [`Fixtures`]: hand-built standard values covering every shape the
//! types allow and every attribute type on every domain.
use super::Producer;
use kinogaki_geometry::{
    Attribute, AttributeType, Attributes, Basis, Column, CurveData, Domain, Elements, Geometry,
    Kind, Knots, Meaning, Primitives, Storage, Text, Values, names,
};
use kinogaki_studio_api::data::Data;
use kinogaki_studio_api::standard;

/// The SDK's fixtures: geometry (a quad grid; mixed triangles, quads and
/// n-gons; concave polygons; polylines only; polygons and polylines
/// sharing points; curves of every basis, open and closed; a quad, a
/// polyline and a NURBS curve sharing points; points only; every attribute
/// type on every domain; groups with `.hidden` and `sharp`), and B-reps
/// (a box, a cylinder, a cone, a sphere, a trimmed NURBS patch, and a
/// value with every part the type has). With
/// `large`, a 1000 × 1000 quad grid too (1M faces).
#[derive(Clone, Copy, Debug, Default)]
pub struct Fixtures {
    /// Adds the 1M-face grid.
    pub large: bool,
}

impl Fixtures {
    /// The B-rep fixtures, by name.
    pub fn breps(&self) -> Vec<(String, kinogaki_geometry::brep::Brep)> {
        super::brep::breps()
    }

    /// The geometry fixtures, by name.
    pub fn geometry(&self) -> Vec<(String, Geometry)> {
        let mut out = vec![
            ("quad grid".to_owned(), grid(4)),
            ("mixed polygons".to_owned(), mixed()),
            ("concave polygons".to_owned(), concave()),
            ("polylines only".to_owned(), polylines_only()),
            ("polygons and polylines".to_owned(), shared_points()),
            ("curves of every basis".to_owned(), every_basis()),
            ("polygon, polyline and curve".to_owned(), mixed_primitives()),
            ("points only".to_owned(), points_only()),
            ("every attribute".to_owned(), every_attribute()),
            ("groups".to_owned(), groups()),
            ("empty".to_owned(), Geometry::empty()),
        ];
        if self.large {
            out.push(("1M-face grid".to_owned(), grid(1000)));
        }
        out
    }
}

impl Producer for Fixtures {
    fn name(&self) -> &str {
        "SDK fixtures"
    }
    fn fixtures(&self) -> Vec<(String, Data)> {
        let geometry = self
            .geometry()
            .into_iter()
            .map(|(n, p)| (n, standard::geometry().wrap(p)));
        let breps = self
            .breps()
            .into_iter()
            .map(|(n, b)| (n, standard::brep().wrap(b)));
        geometry.chain(breps).collect()
    }
}

/// Primitives as lists of points, `polygon` saying which close (polygons;
/// the others are polylines).
fn value(
    positions: Vec<[f64; 3]>,
    prims: &[Vec<u32>],
    polygon: impl Fn(usize) -> bool,
) -> Geometry {
    let mut offsets = vec![0];
    let mut vertices = Vec::new();
    for p in prims {
        vertices.extend_from_slice(p);
        offsets.push(vertices.len() as u32);
    }
    let kinds = (0..prims.len())
        .map(|p| {
            if polygon(p) {
                Kind::POLYGON
            } else {
                Kind::POLYLINE
            }
        })
        .collect();
    let t = Primitives::new(
        positions.len(),
        offsets.into(),
        vertices.into(),
        kinds,
        None,
    )
    .expect("a valid fixture");
    Geometry::new(positions.into(), t, Attributes::new()).expect("a valid fixture")
}

/// A `side` × `side` grid of quads.
fn grid(side: u32) -> Geometry {
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
    let t = Primitives::of_polygons((row * row) as usize, offsets.into(), vertices.into())
        .expect("a grid");
    Geometry::new(positions.into(), t, Attributes::new()).expect("a grid")
}

fn mixed() -> Geometry {
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
        |_| true,
    )
}

fn concave() -> Geometry {
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
    value(p, &[vec![0, 1, 2, 3, 4, 5], vec![6, 7, 8, 9]], |_| true)
}

fn polylines_only() -> Geometry {
    let p = (0..6).map(|i| [i as f64, 0.0, (i % 2) as f64]).collect();
    value(p, &[vec![0, 1, 2], vec![3, 4, 5, 3]], |_| false)
}

fn shared_points() -> Geometry {
    let p = vec![
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [1.0, 1.0, 0.0],
        [0.0, 1.0, 0.0],
        [2.0, 2.0, 0.0],
    ];
    // A polyline first, the quad between two of them: any order.
    value(p, &[vec![2, 4], vec![0, 1, 2, 3], vec![3, 2, 1]], |p| {
        p == 1
    })
}

/// Primitives over `positions`' points, a row each (its kind, degree,
/// points and, for NURBS, knots), with a weight a vertex when `weights`.
pub(super) fn curves(
    positions: Vec<[f64; 3]>,
    rows: &[(Kind, u8, Vec<u32>, Vec<f64>)],
    weights: Option<Vec<f64>>,
) -> Primitives {
    let (mut offsets, mut vertices) = (vec![0], Vec::new());
    let (mut knot_offsets, mut knots) = (vec![0], Vec::new());
    for (_, _, ids, k) in rows {
        vertices.extend_from_slice(ids);
        offsets.push(vertices.len() as u32);
        knots.extend_from_slice(k);
        knot_offsets.push(knots.len() as u32);
    }
    let degrees: Vec<u8> = rows.iter().map(|r| r.1).collect();
    let data = CurveData::new(
        degrees.into(),
        Knots::new(knot_offsets.into(), knots.into()),
        weights.map(Into::into),
    );
    let kinds: Vec<Kind> = rows.iter().map(|r| r.0).collect();
    Primitives::new(
        positions.len(),
        offsets.into(),
        vertices.into(),
        kinds.into(),
        Some(data),
    )
    .expect("a valid fixture")
}

/// Points along x, alternating in y.
fn zigzag(n: usize) -> Vec<[f64; 3]> {
    (0..n).map(|i| [i as f64, (i % 2) as f64, 0.0]).collect()
}

/// One curve of every basis, open and closed, with a Point `width`: a
/// polyline, a cubic Bézier, an open and a closed B-spline, a
/// Catmull-Rom and a rational NURBS curve.
fn every_basis() -> Geometry {
    let run = |from: u32, n: u32| (from..from + n).collect::<Vec<u32>>();
    let curve = |b, closed| Kind::curve(b, closed);
    let rows = [
        (Kind::POLYLINE, 1, run(0, 3), vec![]),
        (curve(Basis::Bezier, false), 3, run(3, 7), vec![]),
        (curve(Basis::BSpline, false), 3, run(10, 4), vec![]),
        (curve(Basis::BSpline, true), 2, run(14, 5), vec![]),
        (curve(Basis::CatmullRom, false), 3, run(19, 5), vec![]),
        (
            curve(Basis::Nurbs, false),
            2,
            run(24, 4),
            vec![0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0],
        ),
    ];
    let mut weights = vec![1.0; 28];
    weights[25..27].fill(0.7);
    let t = curves(zigzag(28), &rows, Some(weights));
    let widths = Attributes::new()
        .with(
            Domain::Point,
            names::WIDTH,
            tuples(vec![0.1f32; 28], Meaning::None),
        )
        .expect("width");
    Geometry::new(zigzag(28).into(), t, widths).expect("a valid fixture")
}

/// A quad, a polyline from its corner and a NURBS curve from another,
/// sharing those points; the curve first (any order).
fn mixed_primitives() -> Geometry {
    let mut p = vec![
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [1.0, 1.0, 0.0],
        [0.0, 1.0, 0.0],
    ];
    p.extend([
        [2.0, 2.0, 0.0],
        [2.0, 3.0, 1.0],
        [3.0, 3.0, 0.0],
        [-1.0, 2.0, 0.0],
    ]);
    let rows = [
        (
            Kind::curve(Basis::Nurbs, false),
            3,
            vec![2, 4, 5, 6],
            vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0],
        ),
        (Kind::POLYGON, 1, vec![0, 1, 2, 3], vec![]),
        (Kind::POLYLINE, 1, vec![3, 7], vec![]),
    ];
    Geometry::new(p.clone().into(), curves(p, &rows, None), Attributes::new())
        .expect("a valid fixture")
}

fn points_only() -> Geometry {
    value(
        vec![[0.0; 3], [1.0, 2.0, 3.0], [-1.0, 0.5, 2.0]],
        &[],
        |_| true,
    )
}

/// `values` as an attribute (a fixture's: always valid).
fn tuples<T: kinogaki_geometry::Tuple>(values: Vec<T>, meaning: Meaning) -> Attribute {
    Attribute::tuples(values, meaning).expect("a valid fixture attribute")
}

/// Element `e` holds `e % 4` reals (empty arrays among them).
fn reals_lists(n: usize) -> Attribute {
    let offsets: Vec<u32> = (0..=n)
        .map(|e| (0..e).map(|k| (k % 4) as u32).sum())
        .collect();
    let items = *offsets.last().unwrap_or(&0) as usize;
    let ty = AttributeType {
        array: true,
        ..AttributeType::new(Storage::F64, 1, Meaning::None)
    };
    let values = Values::F64(Column::new((0..items).map(|i| i as f64 - 0.5).collect()));
    Attribute::arrays(ty, offsets.into(), values).expect("a valid fixture attribute")
}

/// Element `e` holds `e % 3` names.
fn text_lists(n: usize) -> Attribute {
    let offsets: Vec<u32> = (0..=n)
        .map(|e| (0..e).map(|k| (k % 3) as u32).sum())
        .collect();
    let items = *offsets.last().unwrap_or(&0) as usize;
    let ty = AttributeType {
        array: true,
        ..AttributeType::TEXT
    };
    let text = Text::from_strs((0..items).map(|i| ["x", "y", ""][i % 3]));
    Attribute::arrays(ty, offsets.into(), Values::Text(text)).expect("a valid fixture attribute")
}

fn every_attribute() -> Geometry {
    let base = mixed();
    let mut a = Attributes::new();
    for domain in [
        Domain::Detail,
        Domain::Primitive,
        Domain::Edge,
        Domain::Point,
        Domain::Vertex,
    ] {
        let n = base.count(domain).expect("a polygons domain");
        let at = |i: usize| i as f32 * 0.5;
        let typed = [
            ("f", tuples((0..n).map(at).collect(), Meaning::None)),
            (
                "d",
                tuples((0..n).map(|i| i as f64 / 3.0).collect(), Meaning::None),
            ),
            (
                "i",
                tuples((0..n as i32).map(|i| i - 2).collect(), Meaning::None),
            ),
            (
                "l",
                tuples(
                    (0..n as i64).map(|i| (i << 40) - 3).collect(),
                    Meaning::None,
                ),
            ),
            (
                "pair",
                tuples((0..n as i32).map(|i| [i, -i]).collect(), Meaning::None),
            ),
            (
                "dir",
                tuples(
                    (0..n).map(|i| [i as f64, 1.0, -1.0]).collect(),
                    Meaning::Vector,
                ),
            ),
            (
                "c",
                tuples(
                    (0..n).map(|i| [at(i), 1.0, 0.25, 0.5]).collect(),
                    Meaning::Color,
                ),
            ),
            (
                "q",
                tuples(
                    (0..n).map(|i| [0.0, at(i), 0.0, 1.0]).collect(),
                    Meaning::Rotation,
                ),
            ),
            (
                "m",
                tuples((0..n).map(|i| [i as f64; 16]).collect(), Meaning::Transform),
            ),
            (
                "t",
                Attribute::text(Text::from_strs((0..n).map(|i| ["a", "", "b"][i % 3]))),
            ),
            ("list", reals_lists(n)),
            ("names", text_lists(n)),
        ];
        for (name, attribute) in typed {
            a = a.with(domain, name, attribute).expect("fixture attributes");
        }
    }
    let n = base.count(Domain::Primitive).expect("primitives");
    a = a
        .with(
            Domain::Primitive,
            names::MATERIAL,
            Attribute::text(Text::from_strs((0..n).map(|i| ["/M/A", "/M/B"][i % 2]))),
        )
        .expect("material");
    base.with_attributes(a).expect("sized")
}

fn groups() -> Geometry {
    let base = grid(3);
    let edges = base.count(Domain::Edge).expect("edges");
    let a = Attributes::new()
        .with_group(
            Domain::Primitive,
            names::HIDDEN,
            Elements::from_indices(9, [4]).expect("hidden"),
        )
        .and_then(|a| {
            a.with_group(
                Domain::Edge,
                names::SHARP,
                Elements::from_indices(edges, [0, 1, 2]).expect("sharp"),
            )
        })
        .and_then(|a| {
            a.with_group(
                Domain::Point,
                "corners",
                Elements::from_indices(16, [0, 3, 12, 15]).expect("corners"),
            )
        })
        .expect("groups");
    base.with_attributes(a).expect("sized")
}
