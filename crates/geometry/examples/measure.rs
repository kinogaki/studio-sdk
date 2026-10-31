//! Times the standard values' hot paths on a 1000 × 1000 quad grid
//! (1,002,001 points, 1M faces, 4M vertices; point `N`, vertex `uv`, face
//! `Cd` and `material`): best of 20 runs each.
//!
//! ```sh
//! cargo run --release -p kinogaki-geometry --example measure
//! ```
use kinogaki_geometry::{
    Attribute, Attributes, Class, Column, Polygons, Text, TextBuilder, Topology, TypeInfo, names,
};
use std::hint::black_box;
use std::time::{Duration, Instant};

const SIDE: u32 = 1000;
const RUNS: usize = 20;

fn grid() -> (Vec<[f64; 3]>, Vec<u32>, Vec<u32>) {
    let row = SIDE + 1;
    let positions = (0..row * row)
        .map(|i| [(i % row) as f64, (i / row) as f64, 0.0])
        .collect();
    let mut offsets = Vec::with_capacity((SIDE * SIDE + 1) as usize);
    let mut vertices = Vec::with_capacity((SIDE * SIDE * 4) as usize);
    offsets.push(0);
    for y in 0..SIDE {
        for x in 0..SIDE {
            let p = y * row + x;
            vertices.extend([p, p + 1, p + row + 1, p + row]);
            offsets.push(vertices.len() as u32);
        }
    }
    (positions, offsets, vertices)
}

fn attributes(points: usize, vertices: usize, faces: usize) -> Attributes {
    let mut materials = TextBuilder::new();
    for f in 0..faces {
        materials.push(if f % 2 == 0 {
            "/Materials/A"
        } else {
            "/Materials/B"
        });
    }
    let text: Text = materials.finish();
    Attributes::new()
        .with(
            Class::Point,
            names::N,
            Attribute::vector(vec![[0.0, 0.0, 1.0]; points], TypeInfo::Normal),
        )
        .and_then(|a| {
            a.with(
                Class::Vertex,
                names::UV,
                Attribute::vector(vec![[0.5; 3]; vertices], TypeInfo::TextureCoord),
            )
        })
        .and_then(|a| {
            a.with(
                Class::Primitive,
                names::CD,
                Attribute::vector(vec![[1.0; 3]; faces], TypeInfo::Color),
            )
        })
        .and_then(|a| a.with(Class::Primitive, names::MATERIAL, Attribute::text(text)))
        .expect("standard attributes")
}

/// The best of [`RUNS`] timings of `run`, each after `setup`.
fn best<S, R>(mut setup: impl FnMut() -> S, mut run: impl FnMut(S) -> R) -> Duration {
    (0..RUNS)
        .map(|_| {
            let input = setup();
            let start = Instant::now();
            black_box(run(input));
            start.elapsed()
        })
        .min()
        .expect("runs")
}

fn report(what: &str, time: Duration) {
    println!("{what:<46} {:>9.3} ms", time.as_secs_f64() * 1e3);
}

fn main() {
    let (positions, offsets, vertices) = grid();
    let (points, faces) = (positions.len(), offsets.len() - 1);
    let offsets = Column::new(offsets);
    let vertices = Column::new(vertices);
    println!(
        "{points} points, {faces} faces, {} vertices; best of {RUNS}",
        vertices.len()
    );

    report(
        "Topology::new (validation)",
        best(
            || (),
            |_| Topology::new(points, offsets.clone(), vertices.clone(), faces).unwrap(),
        ),
    );
    let topology = Topology::new(points, offsets.clone(), vertices.clone(), faces).unwrap();
    report(
        "Topology::index (edges and adjacency)",
        best(
            || Topology::new(points, offsets.clone(), vertices.clone(), faces).unwrap(),
            |t| t.index().edges().len(),
        ),
    );
    report(
        "Topology::fingerprint",
        best(
            || Topology::new(points, offsets.clone(), vertices.clone(), faces).unwrap(),
            |t| t.fingerprint(),
        ),
    );
    let attributes = attributes(points, vertices.len(), faces);
    let positions = Column::new(positions);
    report(
        "Polygons::new (topology, positions, attributes)",
        best(
            || {
                (
                    Topology::new(points, offsets.clone(), vertices.clone(), faces).unwrap(),
                    attributes.clone(),
                )
            },
            |(t, a)| Polygons::new(positions.clone(), t, a).unwrap(),
        ),
    );
    let value = Polygons::new(positions.clone(), topology, attributes).unwrap();
    value.topology().index();
    report(
        "with_positions (check 1M positions)",
        best(
            || positions.to_vec().into(),
            |p| value.with_positions(p).unwrap(),
        ),
    );
    report(
        "drag frame: to_vec, move 10k points, with_positions",
        best(
            || (),
            |_| {
                let mut moved = value.positions().to_vec();
                for p in moved.iter_mut().skip(250_000).take(10_000) {
                    p[2] += 0.5;
                }
                value.with_positions(moved.into()).unwrap()
            },
        ),
    );
    report(
        "Polygons::with_topology (shared topology)",
        best(
            || positions.to_vec().into(),
            |p| {
                Polygons::with_topology(p, value.topology().clone(), value.attributes().clone())
                    .unwrap()
            },
        ),
    );
}
