//! The SDK's consumers: [`Codecs`] and [`Invariants`].
use super::Consumer;
use kinogaki_geometry::{Curves, Domain, Polygons, Selection, Topology};
use kinogaki_studio_api::codec::DataCodec;
use kinogaki_studio_api::data::Data;
use kinogaki_studio_api::element::MemoryElement;
use kinogaki_studio_api::standard::{self, BrepCodec, CurvesCodec, PolygonsCodec};

/// Saves each standard value with its codec and reads it back: the read
/// value equals it bit for bit, and a selection of it round-trips too.
#[derive(Clone, Copy, Debug, Default)]
pub struct Codecs;

impl Consumer for Codecs {
    fn name(&self) -> &str {
        "codec round trip"
    }
    fn check(&self, _fixture: &str, data: &Data) -> Result<(), String> {
        let (codec, count): (&dyn DataCodec, Option<usize>) =
            if let Some(p) = standard::polygons().get(data) {
                (&PolygonsCodec, p.count(Domain::Point))
            } else if let Some(c) = standard::curves().get(data) {
                (&CurvesCodec, c.count(Domain::Point))
            } else if let Some(b) = standard::brep().get(data) {
                (&BrepCodec, b.count(Domain::Point))
            } else {
                return Ok(());
            };
        let mut saved = MemoryElement::new("result");
        saved
            .write(|w| codec.write(data, None, w))
            .map_err(|e| format!("write: {e}"))?;
        let read = saved
            .read(|r| codec.read(r, None))
            .map_err(|e| format!("read: {e}"))?;
        if !same_bits(data, &read) {
            return Err("the value read back differs".into());
        }
        let points = count.unwrap_or(0);
        let picked: Vec<u32> = (0..points as u32).step_by(2).collect();
        let active = picked.last().copied();
        let selection =
            Selection::new(Domain::Point, picked, active, None).map_err(|e| e.to_string())?;
        let components = standard::components(data.ty(), selection, points);
        let mut saved = MemoryElement::new("components");
        saved
            .write(|w| codec.write_components(&components, data, w))
            .map_err(|e| format!("write components: {e}"))?;
        let back = saved
            .read(|r| codec.read_components(r, data))
            .map_err(|e| format!("read components: {e}"))?;
        if back.selection() != components.selection() || back.len() != components.len() {
            return Err("the selection read back differs".into());
        }
        Ok(())
    }
}

/// Whether `a` and `b` hold equal standard values, comparing reals by
/// their bits.
fn same_bits(a: &Data, b: &Data) -> bool {
    let bits = |p: &[[f64; 3]]| p.iter().flatten().map(|v| v.to_bits()).collect::<Vec<_>>();
    match (standard::polygons().get(a), standard::polygons().get(b)) {
        (Some(x), Some(y)) => x == y && bits(x.positions()) == bits(y.positions()),
        _ => match (standard::curves().get(a), standard::curves().get(b)) {
            (Some(x), Some(y)) => x == y && bits(x.positions()) == bits(y.positions()),
            _ => match (standard::brep().get(a), standard::brep().get(b)) {
                // Equal values whose saves are equal: the same bits.
                (Some(x), Some(y)) => x == y && saved(a) == saved(b),
                _ => false,
            },
        },
    }
}

/// What the B-rep codec writes of `data`.
fn saved(data: &Data) -> Option<MemoryElement> {
    let mut out = MemoryElement::new("result");
    out.write(|w| BrepCodec.write(data, None, w)).ok()?;
    Some(out)
}

/// Rebuilds each standard value from its parts through the validating
/// constructors (so a producer's value obeys every rule) and checks the
/// derived numbering: equal values, canonical edges, consistent counts.
#[derive(Clone, Copy, Debug, Default)]
pub struct Invariants;

impl Consumer for Invariants {
    fn name(&self) -> &str {
        "invariants"
    }
    fn check(&self, _fixture: &str, data: &Data) -> Result<(), String> {
        if let Some(p) = standard::polygons().get(data) {
            return polygons(p);
        }
        if let Some(c) = standard::curves().get(data) {
            return curves(c);
        }
        Ok(())
    }
}

fn polygons(p: &Polygons) -> Result<(), String> {
    let t = p.topology();
    let rebuilt = Topology::new(
        t.points(),
        t.offsets().clone(),
        t.vertices().clone(),
        t.polygons(),
    )
    .map_err(|e| e.to_string())?;
    let again = Polygons::new(p.positions().clone(), rebuilt, (**p.attributes()).clone())
        .map_err(|e| e.to_string())?;
    if &again != p {
        return Err("rebuilt from its parts, the value differs".into());
    }
    if again.topology().fingerprint() != t.fingerprint() {
        return Err("an equal topology has another fingerprint".into());
    }
    let edges = t.edges();
    if edges.windows(2).any(|w| w[0] >= w[1]) || edges.iter().any(|e| e.points[0] >= e.points[1]) {
        return Err("the edges are not in canonical order".into());
    }
    let index = t.index();
    for prim in 0..t.primitives() {
        let ids = t.primitive(prim);
        for (i, &e) in index.primitive_edges(prim).iter().enumerate() {
            let next = ids[(i + 1) % ids.len()];
            if t.edge_of(ids[i], next) != Some(e) {
                return Err(format!("primitive {prim}'s edge {i} is misnumbered"));
            }
        }
    }
    Ok(())
}

fn curves(c: &Curves) -> Result<(), String> {
    let mut builder = Curves::builder(c.basis(), c.positions().clone(), c.offsets().clone())
        .degrees(c.degrees().clone())
        .periodic(c.periodic().clone())
        .shared_attributes(c.attributes().clone());
    if let Some(points) = c.points() {
        builder = builder.points(points.clone());
    }
    if let Some(knots) = c.knots() {
        builder = builder.knots(knots.offsets().clone(), knots.values().clone());
    }
    if let Some(weights) = c.weights() {
        builder = builder.weights(weights.clone());
    }
    let again = builder.build().map_err(|e| e.to_string())?;
    if &again != c {
        return Err("rebuilt from its parts, the value differs".into());
    }
    Ok(())
}
