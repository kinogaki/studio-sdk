//! Times the standard values' hot paths on a 1000 × 1000 quad grid
//! (1,002,001 points, 1M faces, 4M vertices; point `N`, vertex `uv`, face
//! `Cd` and `material`): best of 20 runs each. Then, once, a 4472 × 4472
//! grid (20M points and quads), past the former 2²³ limit: built,
//! checked and its edges counted.
//!
//! ```sh
//! cargo run --release -p kinogaki-geometry --example measure
//! ```
use kinogaki_geometry::{
    Attribute, Attributes, Column, Domain, Geometry, Meaning, Primitives, Text, names,
};
use std::hint::black_box;
use std::time::{Duration, Instant};

const SIDE: u32 = 1000;
/// The large grid's side: 4472² is 20M quads.
const LARGE: u32 = 4472;
const RUNS: usize = 20;

fn grid(side: u32) -> (Vec<[f64; 3]>, Vec<u32>, Vec<u32>) {
    let row = side + 1;
    let positions = (0..row * row)
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
    (positions, offsets, vertices)
}

fn attributes(points: usize, vertices: usize, faces: usize) -> Attributes {
    let text = Text::from_strs((0..faces).map(|f| {
        if f % 2 == 0 {
            "/Materials/A"
        } else {
            "/Materials/B"
        }
    }));
    Attributes::new()
        .with(
            Domain::Point,
            names::N,
            Attribute::tuples(vec![[0.0f32, 0.0, 1.0]; points], Meaning::Normal).unwrap(),
        )
        .and_then(|a| {
            a.with(
                Domain::Vertex,
                names::UV,
                Attribute::tuples(vec![[0.5f32; 2]; vertices], Meaning::TextureCoord).unwrap(),
            )
        })
        .and_then(|a| {
            a.with(
                Domain::Primitive,
                names::CD,
                Attribute::tuples(vec![[1.0f32; 3]; faces], Meaning::Color).unwrap(),
            )
        })
        .and_then(|a| a.with(Domain::Primitive, names::MATERIAL, Attribute::text(text)))
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
    let (positions, offsets, vertices) = grid(SIDE);
    let (points, faces) = (positions.len(), offsets.len() - 1);
    let offsets = Column::new(offsets);
    let vertices = Column::new(vertices);
    println!(
        "{points} points, {faces} faces, {} vertices; best of {RUNS}",
        vertices.len()
    );

    report(
        "Primitives::new (validation)",
        best(
            || (),
            |_| Primitives::of_polygons(points, offsets.clone(), vertices.clone()).unwrap(),
        ),
    );
    let topology = Primitives::of_polygons(points, offsets.clone(), vertices.clone()).unwrap();
    report(
        "Primitives::index (edges and adjacency)",
        best(
            || Primitives::of_polygons(points, offsets.clone(), vertices.clone()).unwrap(),
            |t| t.index().edge_count(),
        ),
    );
    report(
        "Primitives::edge_count",
        best(
            || Primitives::of_polygons(points, offsets.clone(), vertices.clone()).unwrap(),
            |t| t.edge_count(),
        ),
    );
    report(
        "Primitives::fingerprint",
        best(
            || Primitives::of_polygons(points, offsets.clone(), vertices.clone()).unwrap(),
            |t| t.fingerprint(),
        ),
    );
    let attributes = attributes(points, vertices.len(), faces);
    let positions = Column::new(positions);
    report(
        "Geometry::new (positions, attributes checked)",
        best(
            || {
                (
                    Primitives::of_polygons(points, offsets.clone(), vertices.clone()).unwrap(),
                    attributes.clone(),
                )
            },
            |(t, a)| Geometry::new(positions.clone(), t, a).unwrap(),
        ),
    );
    let value = Geometry::new(positions.clone(), topology, attributes).unwrap();
    value.primitives().index();
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
        "Geometry::with_positions (shared table)",
        best(
            || positions.to_vec().into(),
            |p| value.with_positions(p).unwrap(),
        ),
    );
    large();
}

/// The 20M-quad grid, once: built and checked, its edges counted without
/// the index.
fn large() {
    let (positions, offsets, vertices) = grid(LARGE);
    let points = positions.len();
    let start = Instant::now();
    let t = Primitives::of_polygons(points, offsets.into(), vertices.into()).unwrap();
    let value = Geometry::new(positions.into(), t, Attributes::new()).unwrap();
    let built = start.elapsed();
    let start = Instant::now();
    let edges = value.primitives().edge_count();
    println!(
        "{points} points, {} quads: built {built:.1?}, {edges} edges counted {:.1?}",
        value.primitives().len(),
        start.elapsed()
    );
}
