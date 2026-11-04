//! [`extract`]: some of a value's elements as a value of their own, at the
//! cost of what it takes and not of what it leaves: the pieces of a loop.
use super::gather::picked;
use kinogaki_geometry::{
    Attributes, CurveData, Domain, Elements, Error, Geometry, Knots, Primitives,
};
use std::collections::HashMap;

/// A value of `primitives` of `value`, their points and `points` besides, in
/// the order of their numbers there: attributes picked, ids and groups kept,
/// a primitive's number its place in `primitives`.
///
/// # Errors
/// A number past its elements.
pub fn extract(value: &Geometry, primitives: &[u32], points: &[u32]) -> Result<Geometry, Error> {
    let table = value.primitives();
    let mut kept: Vec<u32> = points.to_vec();
    let mut corners = Vec::new();
    for &p in primitives {
        let range = table.primitive_vertices(p as usize);
        kept.extend(range.clone().map(|v| table.vertices()[v]));
        corners.extend(range.map(|v| v as u32));
    }
    kept.sort_unstable();
    kept.dedup();
    let numbers: HashMap<u32, u32> = (kept.iter().enumerate())
        .map(|(new, &old)| (old, new as u32))
        .collect();
    let positions = value.positions();
    let at: Vec<[f64; 3]> = kept.iter().map(|&p| positions[p as usize]).collect();
    let mut offsets = Vec::with_capacity(primitives.len() + 1);
    offsets.push(0u32);
    let mut count = 0;
    for &p in primitives {
        count += table.primitive_vertices(p as usize).len() as u32;
        offsets.push(count);
    }
    let vertices: Vec<u32> = (corners.iter())
        .map(|&v| numbers[&table.vertices()[v as usize]])
        .collect();
    let kinds: Vec<_> = primitives.iter().map(|&p| table.kind(p as usize)).collect();
    let curves = table.curves().map(|c| curves_of(c, table, primitives));
    let new = Primitives::new(
        kept.len(),
        offsets.into(),
        vertices.into(),
        kinds.into(),
        curves,
    )?;
    // Each kept edge's number there, once something on edges asks.
    let source = value.attributes();
    let on_edges = source.has(Domain::Edge) || source.groups().any(|g| g.0 == Domain::Edge);
    let edges: Vec<u32> = match on_edges {
        true => new
            .edges()
            .map(|e| {
                let [a, b] = e.points;
                table
                    .edge_of(kept[a as usize], kept[b as usize])
                    .expect("an edge of a primitive it keeps")
            })
            .collect(),
        false => Vec::new(),
    };
    let from = |domain: Domain| -> Option<&[u32]> {
        match domain {
            Domain::Point => Some(&kept),
            Domain::Vertex => Some(&corners),
            Domain::Primitive => Some(primitives),
            Domain::Edge => Some(&edges),
            _ => None,
        }
    };
    let mut attributes = Attributes::new();
    for (domain, name, attribute) in source.iter() {
        attributes = match from(domain) {
            Some(from) => attributes.with(domain, name, picked(attribute, from)?)?,
            None => attributes.with(domain, name, attribute.clone())?,
        };
    }
    for (domain, name, group) in source.groups() {
        let picks = from(domain);
        let members = match picks {
            Some(from) => Elements::from_fn(from.len(), |i| group.contains(from[i as usize])),
            None => group.clone(),
        };
        attributes = attributes.with_group(domain, name, members)?;
    }
    Geometry::new(at.into(), new, attributes)
}

/// `table`'s curve data for `primitives`.
fn curves_of(curves: &CurveData, table: &Primitives, primitives: &[u32]) -> CurveData {
    let (mut degrees, mut offsets, mut knots) = (Vec::new(), vec![0u32], Vec::new());
    let mut weights = curves.weights().map(|_| Vec::new());
    for &p in primitives {
        let p = p as usize;
        degrees.push(curves.degrees()[p]);
        knots.extend_from_slice(curves.knots().of(p));
        offsets.push(knots.len() as u32);
        if let (Some(out), Some(all)) = (&mut weights, curves.weights()) {
            out.extend(table.primitive_vertices(p).map(|v| all[v]));
        }
    }
    let knots = Knots::new(offsets.into(), knots.into());
    CurveData::new(degrees.into(), knots, weights.map(Into::into))
}

#[cfg(test)]
mod tests {
    use super::*;
    use kinogaki_geometry::{Attribute, Meaning};

    /// Two triangles sharing no point, a value per element: a piece keeps
    /// its own, renumbered, and its ids.
    #[test]
    fn a_piece_keeps_what_its_elements_held() {
        let t = Primitives::of_polygons(6, vec![0, 3, 6].into(), vec![0, 1, 2, 3, 4, 5].into());
        let positions: Vec<[f64; 3]> = (0..6).map(|i| [f64::from(i), 0.0, 0.0]).collect();
        let on_points = Attribute::tuples((0..6).map(|i| i as f32).collect(), Meaning::None);
        let on_faces = Attribute::tuples(vec![10.0f32, 20.0], Meaning::None);
        let attributes = Attributes::new()
            .with(Domain::Point, "a", on_points.unwrap())
            .unwrap()
            .with(Domain::Primitive, "b", on_faces.unwrap())
            .unwrap()
            .with_group(Domain::Point, "g", Elements::from_fn(6, |i| i == 4))
            .unwrap();
        let value = Geometry::new(positions.into(), t.unwrap(), attributes).unwrap();
        let piece = extract(&value, &[1], &[]).unwrap();
        assert_eq!(piece.positions().len(), 3);
        assert_eq!(piece.positions()[0], [3.0, 0.0, 0.0]);
        assert_eq!(piece.primitives().primitive(0), [0, 1, 2]);
        let a = piece.attributes().tuples::<f32>(Domain::Point, "a");
        assert_eq!(a.unwrap(), [3.0, 4.0, 5.0]);
        let b = piece.attributes().tuples::<f32>(Domain::Primitive, "b");
        assert_eq!(b.unwrap(), [20.0]);
        let g = piece.attributes().group(Domain::Point, "g").unwrap();
        assert_eq!(g.iter().collect::<Vec<_>>(), [1]);
        // A point alone: no primitive with it.
        let lone = extract(&value, &[], &[2]).unwrap();
        assert_eq!((lone.positions().len(), lone.primitives().len()), (1, 0));
    }
}
