//! Primitives and points added, rewired, removed, welded and split.
use super::fixture::*;
use kinogaki_edit::editor::{Corner, Editor};
use kinogaki_geometry::{
    Attributes, Basis, CurveData, Domain, Geometry, Kind, Knots, Primitives, names,
};

#[test]
fn a_new_primitive_brings_its_vertices_and_leaves_the_point_columns_shared() {
    let value = ruled(2, 1);
    let mut edit = Editor::new(&value);
    let last_vertex = value.primitives().vertex_count() as u32 - 1;
    let from = [(last_vertex, 1.0)];
    let triangle = [
        Corner {
            point: 0,
            from: &from,
        },
        Corner {
            point: 1,
            from: &[(0, 0.5), (1, 0.5)],
        },
        Corner {
            point: 3,
            from: &[],
        },
    ];
    let t = edit.add_primitive(Kind::POLYGON, &triangle, &[(1, 1.0)]);
    assert_eq!(edit.corners(t), [0, 1, 3]);
    assert_eq!(edit.kind(t), Kind::POLYGON);
    let (out, change) = edit.finish().unwrap();
    assert!(change.topology);
    assert_eq!(change.primitives.added, 2..3);
    assert_eq!(out.primitives().primitive(2), [0, 1, 3]);
    // Points: the same columns.
    for name in [names::ID, names::CD, "tag", "label"] {
        let (new, old) = (
            key(&out, Domain::Point, name),
            key(&value, Domain::Point, name),
        );
        assert_eq!(new, old, "{name}");
    }
    assert_eq!(out.positions().key(), value.positions().key());
    // The primitive's attributes from the second face; a fresh id.
    let cd = column::<[f32; 3]>(&out, Domain::Primitive, names::CD);
    assert_eq!(cd[2], [0.0, 1.0, 0.0]);
    assert_eq!(column::<i64>(&out, Domain::Primitive, "count")[2], 1);
    let name = out.attributes().text(Domain::Primitive, names::NAME);
    assert_eq!(name.unwrap().get(2), Some("f1"));
    let ids = column::<i64>(&out, Domain::Primitive, names::ID);
    assert_eq!(ids, [500, 501, 502]);
    assert_eq!(
        members(&out, Domain::Primitive, "even"),
        [0],
        "the odd face's isn't"
    );
    // Each corner's vertex attributes as declared, none for the last.
    let uv = column::<[f32; 2]>(&out, Domain::Vertex, names::UV);
    assert_eq!(uv[8], [7.0, 1.0]);
    assert_eq!(uv[9], [0.5, 1.0]);
    assert_eq!(uv[10], [0.0, 0.0]);
    assert_eq!(
        &uv[..8],
        column::<[f32; 2]>(&value, Domain::Vertex, names::UV)
    );
    assert_eq!(column::<i32>(&out, Domain::Vertex, "corner")[8], 7);
    // Edges keep theirs by their points; a new one starts at the default.
    let crease = column::<f32>(&out, Domain::Edge, "crease");
    let before = column::<f32>(&value, Domain::Edge, "crease");
    let sharp = members(&out, Domain::Edge, "sharp");
    let was_sharp = members(&value, Domain::Edge, "sharp");
    for (e, edge) in out.primitives().edges().enumerate() {
        let [a, b] = edge.points;
        match value.primitives().edge_of(a, b) {
            Some(old) => {
                assert_eq!(crease[e], before[old as usize], "{a}-{b}");
                let in_group = sharp.contains(&(e as u32));
                assert_eq!(in_group, was_sharp.contains(&old), "{a}-{b}");
            }
            None => {
                assert_eq!(crease[e], 0.0, "{a}-{b}");
                assert!(!sharp.contains(&(e as u32)), "{a}-{b}");
            }
        }
    }
    let tint = |v: &Geometry| key(v, Domain::Detail, "tint");
    assert_eq!(tint(&out), tint(&value));
}

#[test]
fn a_polyline_among_polygons_starts_a_kind_column() {
    let value = sheet(1, 1);
    let mut edit = Editor::new(&value);
    edit.add_primitive(Kind::POLYLINE, &corners(&[0, 2]), &[]);
    let (out, _) = edit.finish().unwrap();
    assert_eq!(out.primitives().kind(0), Kind::POLYGON);
    assert_eq!(out.primitives().kind(1), Kind::POLYLINE);
    let polygons: Vec<u32> = out.primitives().polygons().iter().collect();
    assert_eq!(polygons, [0]);
}

#[test]
fn rewiring_with_a_corners_own_vertex_keeps_its_values_shared() {
    let value = ruled(2, 1);
    let table = value.primitives().clone();
    let mut edit = Editor::new(&value);
    let q = edit.add_point([0.0, 0.0, 1.0], &[(0, 1.0)]);
    let vertices = table.primitive_vertices(0);
    let sources: Vec<[(u32, f64); 1]> = vertices.map(|v| [(v as u32, 1.0)]).collect();
    let row: Vec<Corner<'_>> = (table.primitive(0).iter().zip(&sources))
        .map(|(&point, from)| Corner {
            point: if point == 0 { q } else { point },
            from,
        })
        .collect();
    edit.rewire(0, &row);
    assert_eq!(edit.corners(0)[0], q);
    let (out, change) = edit.finish().unwrap();
    assert_eq!(out.primitives().primitive(0)[0], q);
    assert_eq!(change.rewired, [0]);
    let shared = |d, name| key(&out, d, name) == key(&value, d, name);
    assert!(shared(Domain::Vertex, names::UV));
    assert!(shared(Domain::Primitive, names::CD));
    assert_eq!(
        column::<i64>(&out, Domain::Primitive, names::ID),
        [500, 501]
    );
    // A new corner is a new vertex: the columns grow and the next row's
    // values follow it.
    let mut edit = Editor::new(&value);
    let fifth: Vec<Corner<'_>> = (0..4)
        .map(|k| Corner {
            point: table.primitive(0)[k],
            from: &sources[k],
        })
        .chain([Corner {
            point: 5,
            from: &[],
        }])
        .collect();
    edit.rewire(0, &fifth);
    let (out, _) = edit.finish().unwrap();
    assert_eq!(out.primitives().primitive(0).len(), 5);
    let corner = column::<i32>(&out, Domain::Vertex, "corner");
    assert_eq!(&corner[..6], &[0, 1, 2, 3, 0, 4]);
}

#[test]
fn a_removed_primitive_takes_its_values_and_the_last_row_fills_its_number() {
    let value = ruled(2, 2);
    let mut edit = Editor::new(&value);
    edit.remove_primitive(1);
    let (out, change) = edit.finish().unwrap();
    assert_eq!(out.primitives().len(), 3);
    assert_eq!(out.primitives().points(), 9);
    assert_eq!(change.primitives.removed, [1]);
    assert_eq!(change.primitives.of(0), Some(0));
    assert_eq!(change.primitives.of(1), None);
    assert_eq!(change.primitives.of(2), Some(2));
    assert_eq!(change.primitives.of(3), Some(1));
    assert!(!change.primitives.same_numbers());
    let ids = column::<i64>(&out, Domain::Primitive, names::ID);
    assert_eq!(ids, [500, 503, 502]);
    let name = out.attributes().text(Domain::Primitive, names::NAME);
    assert_eq!(name.unwrap().get(1), Some("f3"));
    assert_eq!(
        members(&out, Domain::Primitive, "even"),
        [0, 2],
        "0 and 2, in their places"
    );
    let corner = column::<i32>(&out, Domain::Vertex, "corner");
    assert_eq!(&corner[..8], &[0, 1, 2, 3, 12, 13, 14, 15]);
    assert_eq!(out.positions().key(), value.positions().key());
    let tag = |v: &Geometry| key(v, Domain::Point, "tag");
    assert_eq!(tag(&out), tag(&value));
    // Loose points go when asked: removing the first face leaves point 0.
    let mut edit = Editor::new(&value);
    edit.remove_primitive(0);
    edit.remove_loose_points();
    let (out, change) = edit.finish().unwrap();
    assert_eq!(change.points.removed, [0]);
    assert_eq!(change.points.of(0), None);
    assert_eq!(change.points.of(1), Some(1));
    assert_eq!(change.points.of(8), Some(0), "the last point fills it");
    assert_eq!(out.positions().len(), 8);
    assert_eq!(column::<i64>(&out, Domain::Point, names::ID)[0], 108);
    assert_eq!(
        out.primitives().primitive(0),
        [4, 5, 0, 7],
        "the last row, which names that point"
    );
    // Points no row named before the edit stay.
    let lone = Geometry::new(
        vec![[0.0; 3]; 5].into(),
        Primitives::of_polygons(5, vec![0, 3].into(), vec![0, 1, 2].into()).unwrap(),
        Attributes::new(),
    )
    .unwrap();
    let mut edit = Editor::new(&lone);
    edit.remove_primitive(0);
    edit.remove_loose_points();
    let (out, _) = edit.finish().unwrap();
    assert_eq!(out.positions().len(), 2);
}

/// A removal on a value whose index is built: the new index answers as one
/// built whole, past the blocks a shorter table leaves.
#[test]
fn a_removal_patches_the_index_to_what_a_whole_build_makes() {
    let value = sheet(40, 40);
    let last_block = 1024;
    let points = value.primitives().points() as u32;
    value.primitives().index();
    value.primitives().fingerprint();
    let mut edit = Editor::new(&value);
    (last_block..points - 1)
        .step_by(7)
        .for_each(|p| edit.remove_point(p));
    edit.weld(0, 1);
    edit.remove_primitive(3);
    edit.remove_loose_points();
    let (out, _) = edit.finish().unwrap();
    let t = out.primitives();
    let whole = Primitives::of_polygons(t.points(), t.offsets().clone(), t.vertices().clone());
    let whole = whole.unwrap();
    assert_eq!(t.fingerprint(), whole.fingerprint());
    assert_eq!(t.edge_count(), whole.edge_count());
    for p in 0..t.points() {
        assert_eq!(
            t.index().point_faces(p),
            whole.index().point_faces(p),
            "point {p}"
        );
    }
    for e in 0..t.edge_count() {
        assert_eq!(t.edge(e), whole.edge(e), "edge {e}");
        assert_eq!(
            t.index().edge_faces(e),
            whole.index().edge_faces(e),
            "edge {e}"
        );
    }
}

#[test]
fn removing_a_point_takes_its_corners_and_a_row_too_short_goes() {
    let value = ruled(2, 1);
    let mut edit = Editor::new(&value);
    edit.remove_point(1);
    edit.remove_point(4);
    let (out, change) = edit.finish().unwrap();
    // The old rows 0 1 4 3 and 1 2 5 4 each lost two corners.
    assert_eq!(out.primitives().len(), 0);
    assert_eq!(change.primitives.removed, [0, 1]);
    assert_eq!(out.positions().len(), 4);
    // One corner gone: the quads become triangles, their vertices drop.
    let mut edit = Editor::new(&value);
    edit.remove_point(1);
    let (out, change) = edit.finish().unwrap();
    assert_eq!(out.primitives().primitive(0), [0, 4, 3], "the old 0 4 3");
    assert_eq!(out.primitives().primitive(1), [2, 1, 4], "the old 2 5 4");
    assert_eq!(change.rewired, [0, 1]);
    let corner = column::<i32>(&out, Domain::Vertex, "corner");
    assert_eq!(corner, [0, 2, 3, 5, 6, 7]);
    let ids = column::<i64>(&out, Domain::Point, names::ID);
    assert_eq!(ids, [100, 105, 102, 103, 104]);
}

#[test]
fn welding_joins_points_and_collapses_what_became_one() {
    let value = ruled(2, 1);
    // Points 0 and 1 are an edge of the first quad: it becomes a triangle.
    let mut edit = Editor::new(&value);
    edit.weld(0, 1);
    let (out, change) = edit.finish().unwrap();
    assert_eq!(out.positions().len(), 5);
    assert_eq!(out.primitives().primitive(0), [0, 4, 3], "the old 0 4 3");
    assert_eq!(
        out.primitives().primitive(1),
        [0, 2, 1, 4],
        "the old 1 2 5 4"
    );
    assert_eq!(change.points.of(1), Some(0), "where the survivor is");
    assert_eq!(change.points.of(2), Some(2));
    assert_eq!(change.points.of(5), Some(1), "the last point fills it");
    let ids = column::<i64>(&out, Domain::Point, names::ID);
    assert_eq!(ids, [100, 105, 102, 103, 104]);
    let corner = column::<i32>(&out, Domain::Vertex, "corner");
    assert_eq!(corner, [0, 2, 3, 4, 5, 6, 7]);
    assert_eq!(change.rewired, [0, 1]);
    // A triangle's two corners welded: it goes.
    let t = Primitives::of_polygons(3, vec![0, 3].into(), vec![0, 1, 2].into()).unwrap();
    let triangle = Geometry::new(vec![[0.0; 3]; 3].into(), t, Attributes::new()).unwrap();
    let mut edit = Editor::new(&triangle);
    edit.weld(0, 1);
    let (out, change) = edit.finish().unwrap();
    assert!(out.primitives().is_empty());
    assert_eq!(change.primitives.removed, [0]);
    // Across a quad: a polygon repeating a point, refused.
    let mut edit = Editor::new(&sheet(1, 1));
    edit.weld(0, 3);
    let refused = edit.finish().unwrap_err();
    assert!(refused.message().contains("repeats a point"), "{refused}");
    // A welded point's own move goes with it; its survivor stays put.
    let mut edit = Editor::new(&value);
    edit.set_position(1, [9.0; 3]);
    edit.weld(0, 1);
    let (out, change) = edit.finish().unwrap();
    assert_eq!(out.positions()[0], value.positions()[0]);
    assert!(change.moved.is_empty());
    // Welds that would go round stay one point.
    let mut edit = Editor::new(&value);
    edit.weld(0, 1);
    edit.weld(1, 0);
    let (_, change) = edit.finish().unwrap();
    assert_eq!(change.points.of(1), Some(0));
    // A chain ends at its last survivor.
    let mut edit = Editor::new(&value);
    edit.weld(1, 2);
    edit.weld(0, 1);
    let (_, change) = edit.finish().unwrap();
    assert_eq!(change.points.of(2), Some(0));
}

#[test]
fn splitting_an_edge_reaches_every_row_on_it_and_blends_their_vertices() {
    let value = ruled(2, 1);
    let table = value.primitives().clone();
    let mut edit = Editor::new(&value);
    // The shared edge 1–4, between the two quads.
    let q = edit.split_edge(1, 4, 0.25);
    assert_eq!(edit.position(q), [1.0, 0.25, 0.0]);
    assert_eq!(edit.corners(0), [0, 1, q, 4, 3]);
    assert_eq!(edit.corners(1), [1, 2, 5, 4, q]);
    let (out, change) = edit.finish().unwrap();
    assert_eq!(change.rewired, [0, 1]);
    assert_eq!(change.points.added, 6..7);
    let rest = column::<[f64; 3]>(&out, Domain::Point, names::REST);
    assert_eq!(rest[q as usize], [1.0, 0.25, 0.0]);
    // First quad's corners 1 and 4 are vertices 1 and 2: 0.25 of the way.
    let uv = column::<[f32; 2]>(&out, Domain::Vertex, names::UV);
    assert_eq!(uv[2], [1.25, 1.0]);
    // The second quad runs 4 → 1 across its last corner, wrapping: its
    // vertices 7 (point 4) and 4 (point 1).
    let second = out.primitives().primitive_vertices(1);
    assert_eq!(
        uv[second.end - 1],
        [4.75, 1.0],
        "a quarter of the way from 1's vertex to 4's"
    );
    // Both halves keep the edge's crease, other edges keep theirs.
    let creases = column::<f32>(&value, Domain::Edge, "crease");
    let was = creases[table.edge_of(1, 4).unwrap() as usize];
    let crease = column::<f32>(&out, Domain::Edge, "crease");
    let new_table = out.primitives();
    assert_eq!(crease[new_table.edge_of(1, q).unwrap() as usize], was);
    assert_eq!(crease[new_table.edge_of(q, 4).unwrap() as usize], was);

    // Twice on one edge: each new corner blends from the base's values.
    let mut edit = Editor::new(&value);
    let mid = edit.split_edge(0, 1, 0.5);
    let quarter = edit.split_edge(0, mid, 0.5);
    assert_eq!(edit.corners(0)[..3], [0, quarter, mid]);
    let (out, _) = edit.finish().unwrap();
    let uv = column::<[f32; 2]>(&out, Domain::Vertex, names::UV);
    assert_eq!(uv[1], [0.25, 1.0]);
    assert_eq!(uv[2], [0.5, 1.0]);
    let crease = column::<f32>(&out, Domain::Edge, "crease");
    let was = creases[table.edge_of(0, 1).unwrap() as usize];
    let at = out.primitives().edge_of(0, quarter).unwrap();
    assert_eq!(crease[at as usize], was, "through both splits");
}

#[test]
fn a_polygon_splits_between_two_corners_and_a_point_splits_for_some_rows() {
    let value = ruled(1, 1);
    let mut edit = Editor::new(&value);
    let second = edit.split_polygon(0, 0, 2);
    assert_eq!(second, 1);
    let (out, change) = edit.finish().unwrap();
    assert_eq!(out.primitives().primitive(0), [0, 1, 3]);
    assert_eq!(out.primitives().primitive(1), [3, 2, 0]);
    assert_eq!(change.primitives.added, 1..2);
    assert_eq!(
        column::<i64>(&out, Domain::Primitive, names::ID),
        [500, 501]
    );
    let name = out.attributes().text(Domain::Primitive, names::NAME);
    assert_eq!(name.unwrap().get(1), Some("f0"));
    let corner = column::<i32>(&out, Domain::Vertex, "corner");
    assert_eq!(corner, [0, 1, 2, 2, 3, 0]);
    // Neighbouring corners don't cut.
    let mut edit = Editor::new(&value);
    edit.split_polygon(0, 0, 1);
    assert!(edit.finish().is_err());

    let strip = ruled(2, 1);
    let mut edit = Editor::new(&strip);
    let q = edit.split_point(1, &[1]);
    assert_eq!(edit.corners(1)[0], q);
    assert_eq!(edit.corners(0)[1], 1);
    let (out, _) = edit.finish().unwrap();
    assert_eq!(out.positions()[q as usize], strip.positions()[1]);
    assert_eq!(
        column::<i32>(&out, Domain::Point, "tag")[q as usize],
        1,
        "a copy"
    );
    let ids = column::<i64>(&out, Domain::Point, names::ID);
    assert_eq!(ids[q as usize], 106, "a new id");
}

/// A polygon, a rational NURBS curve (corners 3 4 5 6) and a polyline.
fn with_curve() -> Geometry {
    let kinds = vec![
        Kind::POLYGON,
        Kind::curve(Basis::Nurbs, false),
        Kind::POLYLINE,
    ];
    let knots = vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0];
    let weights = vec![1.0, 1.0, 1.0, 2.0, 3.0, 4.0, 5.0, 1.0, 1.0];
    let data = CurveData::new(
        vec![1, 3, 1].into(),
        Knots::new(vec![0, 0, 8, 8].into(), knots.into()),
        Some(weights.into()),
    );
    let table = Primitives::new(
        9,
        vec![0, 3, 7, 9].into(),
        vec![0, 1, 2, 3, 4, 5, 6, 7, 8].into(),
        kinds.into(),
        Some(data),
    )
    .unwrap();
    Geometry::new(vec![[0.0; 3]; 9].into(), table, Attributes::new()).unwrap()
}

#[test]
fn curves_stay_as_they_are_around_what_changes() {
    let value = with_curve();
    let mut edit = Editor::new(&value);
    edit.remove_primitive(0);
    edit.add_primitive(Kind::POLYGON, &corners(&[0, 1, 2]), &[]);
    edit.remove_point(3); // a corner of the curve, too few for its degree
    assert!(edit.finish().is_err());
    let mut edit = Editor::new(&value);
    edit.remove_primitive(0);
    edit.add_primitive(Kind::POLYGON, &corners(&[0, 1, 2]), &[]);
    let (out, _) = edit.finish().unwrap();
    let table = out.primitives();
    assert_eq!(table.kind(0), Kind::POLYLINE);
    assert_eq!(table.kind(1), Kind::curve(Basis::Nurbs, false));
    assert_eq!(table.kind(2), Kind::POLYGON);
    let data = table.curves().unwrap();
    assert_eq!(data.degrees().as_slice(), &[1, 3, 1]);
    assert_eq!(data.knots().of(1).len(), 8);
    assert!(data.knots().of(2).is_empty());
    let weights = data.weights().unwrap();
    assert_eq!(&weights.as_slice()[2..6], &[2.0, 3.0, 4.0, 5.0]);
    assert_eq!(&weights.as_slice()[6..], &[1.0, 1.0, 1.0]);
    // The curve removed: no curve data.
    let mut edit = Editor::new(&value);
    edit.remove_primitive(1);
    let (out, _) = edit.finish().unwrap();
    assert!(out.primitives().curves().is_none());
    assert_eq!(out.primitives().primitive(1), [7, 8]);
}

/// A bad number read is remembered as a bad number written is: the reader
/// answers a neutral value and the finish reports the first fault.
#[test]
fn reading_a_bad_number_answers_neutrally_and_the_finish_reports_it() {
    let edit = Editor::new(&sheet(1, 1));
    assert_eq!(edit.position(4), [0.0; 3]);
    assert_eq!((edit.corners(1), edit.kind(1)), (&[][..], Kind::POLYGON));
    let refused = edit.finish().unwrap_err();
    assert_eq!(refused.message(), "point 4 is past the last of 4");
    let edit = Editor::new(&sheet(1, 1));
    assert_eq!(edit.corners(0).len(), 4);
    assert!(edit.finish().is_ok());
}
