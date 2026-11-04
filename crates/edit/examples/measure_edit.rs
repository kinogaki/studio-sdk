//! What a small edit of a 1M-point value costs, in the release profile:
//! `cargo run --release -p kinogaki-edit --example measure_edit`. Each row
//! is the median of 21 runs.
use kinogaki_edit::editor::{Corner, Editor};
use kinogaki_geometry::{
    Attribute, Attributes, Domain, Elements, Geometry, Kind, Meaning, Primitives, Text, names,
};
use std::time::Instant;

fn sheet(n: u32) -> Geometry {
    let at = |i: u32, j: u32| j * (n + 1) + i;
    let mut corners = Vec::with_capacity((4 * n * n) as usize);
    for j in 0..n {
        for i in 0..n {
            corners.extend([at(i, j), at(i + 1, j), at(i + 1, j + 1), at(i, j + 1)]);
        }
    }
    let offsets: Vec<u32> = (0..=n * n).map(|f| 4 * f).collect();
    let points = ((n + 1) * (n + 1)) as usize;
    let table = Primitives::of_polygons(points, offsets.into(), corners.into()).unwrap();
    let positions: Vec<[f64; 3]> = (0..=n)
        .flat_map(|j| (0..=n).map(move |i| [f64::from(i), f64::from(j), 0.0]))
        .collect();
    Geometry::new(positions.into(), table, Attributes::new()).unwrap()
}

fn attributed(value: &Geometry) -> Geometry {
    let points = value.count(Domain::Point).unwrap();
    let primitives = value.count(Domain::Primitive).unwrap();
    let vertices = value.count(Domain::Vertex).unwrap();
    let a = Attributes::new()
        .with(
            Domain::Point,
            names::ID,
            Attribute::tuples((0..points as i64).collect::<Vec<_>>(), Meaning::None).unwrap(),
        )
        .unwrap()
        .with(
            Domain::Point,
            names::CD,
            Attribute::tuples(vec![[0.5f32; 3]; points], Meaning::Color).unwrap(),
        )
        .unwrap()
        .with(
            Domain::Primitive,
            names::ID,
            Attribute::tuples((0..primitives as i64).collect::<Vec<_>>(), Meaning::None).unwrap(),
        )
        .unwrap()
        .with(
            Domain::Primitive,
            names::NAME,
            Attribute::text(Text::from_strs(std::iter::repeat_n("part", primitives))),
        )
        .unwrap()
        .with(
            Domain::Vertex,
            names::UV,
            Attribute::tuples(vec![[0.5f32; 2]; vertices], Meaning::TextureCoord).unwrap(),
        )
        .unwrap()
        .with_group(
            Domain::Primitive,
            "some",
            Elements::from_fn(primitives, |p| p % 64 == 0),
        )
        .unwrap();
    value.with_attributes(a).unwrap()
}

fn median(mut run: impl FnMut()) -> f64 {
    let mut times: Vec<f64> = (0..21)
        .map(|_| {
            let start = Instant::now();
            run();
            start.elapsed().as_secs_f64() * 1e3
        })
        .collect();
    times.sort_by(f64::total_cmp);
    times[10]
}

/// `edit` of `value` as a row.
fn row(label: &str, value: &Geometry, edit: impl Fn(&mut Editor)) {
    let time = median(|| {
        let mut editor = Editor::new(value);
        edit(&mut editor);
        std::hint::black_box(editor.finish().unwrap());
    });
    println!("{label:<52} {time:>8.2} ms");
}

fn extrude_face(edit: &mut Editor, value: &Geometry, f: u32) {
    let table = value.primitives().clone();
    let row: Vec<u32> = table.primitive(f as usize).to_vec();
    let vertices = table.primitive_vertices(f as usize).start as u32;
    let caps: Vec<u32> = row
        .iter()
        .map(|&p| {
            let [x, y, z] = edit.position(p);
            edit.add_point([x, y, z + 1.0], &[(p, 1.0)])
        })
        .collect();
    let v = |k: usize| [(vertices + k as u32, 1.0)];
    for k in 0..row.len() {
        let next = (k + 1) % row.len();
        let (a, b) = (v(k), v(next));
        let wall = [
            Corner {
                point: row[k],
                from: &a,
            },
            Corner {
                point: row[next],
                from: &b,
            },
            Corner {
                point: caps[next],
                from: &b,
            },
            Corner {
                point: caps[k],
                from: &a,
            },
        ];
        edit.add_primitive(Kind::POLYGON, &wall, &[(f, 1.0)]);
    }
    let sources: Vec<_> = (0..row.len()).map(v).collect();
    let cap: Vec<Corner<'_>> = (caps.iter().zip(&sources))
        .map(|(&point, from)| Corner { point, from })
        .collect();
    edit.rewire(f, &cap);
}

fn main() {
    let bare = sheet(1000);
    // What every tool has: the index, patched by the edit.
    bare.primitives().index();
    let full = attributed(&bare);
    println!(
        "{} points, {} quads, {} vertices",
        bare.count(Domain::Point).unwrap(),
        bare.primitives().len(),
        bare.primitives().vertex_count()
    );
    let copy = median(|| {
        std::hint::black_box(bare.positions().to_vec());
    });
    println!(
        "{:<52} {copy:>8.2} ms",
        "(a copy of the positions alone: 24 MB)"
    );
    let check = median(|| {
        std::hint::black_box(bare.with_positions(bare.positions().clone()).unwrap());
    });
    println!("{:<52} {check:>8.2} ms", "(checking every position)");
    let table = bare.primitives().clone();
    let validate = median(|| {
        let _ = std::hint::black_box(
            Primitives::of_polygons(
                table.points(),
                table.offsets().clone(),
                table.vertices().clone(),
            )
            .unwrap(),
        );
    });
    println!("{:<52} {validate:>8.2} ms", "(checking every row)");

    for (label, value) in [("bare", &bare), ("attributed", &full)] {
        println!("-- {label}");
        row("move 8 points", value, |e| {
            for p in 0..8 {
                e.set_position(p, [0.0, 0.0, 1.0]);
            }
        });
        row("add 8 points", value, |e| {
            for p in 0..8 {
                e.add_point([0.0; 3], &[(p, 1.0)]);
            }
        });
        row("add 4 quads over existing points", value, |e| {
            for f in 0..4u32 {
                let corners: Vec<Corner<'_>> = [f, f + 1, f + 2, f + 3]
                    .map(|point| Corner { point, from: &[] })
                    .to_vec();
                e.add_primitive(Kind::POLYGON, &corners, &[(f, 1.0)]);
            }
        });
        row(
            "extrude one face (4 points, 4 walls, a rewire)",
            value,
            |e| extrude_face(e, value, 500),
        );
        row("remove one face", value, |e| e.remove_primitive(500));
        row("remove one point", value, |e| e.remove_point(500));
        row("weld two points", value, |e| e.weld(0, 1));
        row("split one edge", value, |e| {
            e.split_edge(0, 1, 0.5);
        });
    }
}
