//! What stays shared, and how attributes, groups, ids and selections
//! follow.
use super::fixture::*;
use kinogaki_edit::deform::{Deform, Move};
use kinogaki_edit::editor::{Corner, Editor, Rule, rule};
use kinogaki_edit::select;
use kinogaki_geometry::{
    AttributeType, Basis, Domain, Elements, Kind, Meaning, Selection, Storage, Vec3, names,
};
use std::f32::consts::FRAC_PI_2;
use std::sync::Arc;

#[test]
fn nothing_changed_is_the_value_itself() {
    let value = ruled(2, 2);
    let (same, change) = Editor::new(&value).finish().unwrap();
    assert!(same.ptr_eq(&value));
    assert!(!change.topology);
    assert!(change.moved.is_empty());
}

#[test]
fn moving_points_copies_the_positions_alone() {
    let value = ruled(2, 2);
    let index: *const _ = value.primitives().index();
    let mut edit = Editor::new(&value);
    edit.set_position(4, [9.0, 9.0, 9.0]);
    edit.set_position(1, value.positions()[1]); // no move at all
    assert_eq!(edit.position(4), [9.0, 9.0, 9.0]);
    let (moved, change) = edit.finish().unwrap();
    assert_eq!(moved.positions()[4], [9.0, 9.0, 9.0]);
    assert_eq!(change.moved, [4]);
    assert!(!change.topology);
    assert!(Arc::ptr_eq(moved.primitives(), value.primitives()));
    assert!(Arc::ptr_eq(moved.attributes(), value.attributes()));
    assert_eq!(
        index,
        moved.primitives().index() as *const _,
        "its caches too"
    );
    // What a display patches, both ways, known without comparing.
    assert_eq!(moved.moved_points(&value), [4]);
    assert_eq!(value.moved_points(&moved), [4]);
    // A bad position is refused, and only the moved points were read.
    let mut edit = Editor::new(&value);
    edit.set_position(0, [f64::NAN; 3]);
    assert!(edit.finish().is_err());
}

#[test]
fn a_deform_places_each_point_by_its_weight_where_a_preview_shows_it() {
    let value = sheet(1, 1);
    let lift = Move::new(
        Vec3::new(0.0, 0.0, 2.0),
        [0.0, 0.0, 0.0, 1.0],
        Vec3::new(1.0, 1.0, 1.0),
        Vec3::ZERO,
    )
    .unwrap();
    let deform = Deform {
        step: lift,
        weights: Some(Arc::from([1.0, 0.5])),
        symmetry: None,
    };
    let mut edit = Editor::new(&value);
    edit.deform(&[1, 3], &deform);
    let (out, change) = edit.finish().unwrap();
    assert_eq!(change.moved, [1, 3]);
    assert_eq!(out.positions()[1], [1.0, 0.0, 2.0]);
    assert_eq!(out.positions()[3], [1.0, 1.0, 1.0]);
    assert_eq!(out.positions()[0], value.positions()[0]);
    assert_eq!(
        out.positions()[3],
        deform.place(value.positions()[3].into(), 0.5).to_array()
    );
    // Without weights every point moves fully, from where the edit has it.
    let mut edit = Editor::new(&value);
    edit.set_position(0, [5.0, 5.0, 5.0]);
    edit.deform(&[0, 2], &Deform::rigid(lift));
    let (out, _) = edit.finish().unwrap();
    assert_eq!(out.positions()[0], [5.0, 5.0, 7.0]);
    assert_eq!(out.positions()[2], [0.0, 1.0, 2.0]);
}

#[test]
fn a_new_point_takes_each_attribute_by_its_rule() {
    let value = ruled(2, 2);
    let mut edit = Editor::new(&value);
    let q = edit.add_point([5.0, 5.0, 5.0], &[(0, 0.25), (1, 0.75)]);
    assert_eq!(q, 9);
    let (out, change) = edit.finish().unwrap();
    assert_eq!(change.points.added, 9..10);
    assert!(change.points.removed.is_empty() && change.points.same_numbers());
    assert_eq!(change.primitives.added, 4..4);
    let at = q as usize;
    assert_eq!(out.positions()[at], [5.0, 5.0, 5.0]);
    // A mean, kept exactly for kept points.
    let cd = column::<[f32; 3]>(&out, Domain::Point, names::CD);
    assert_eq!(cd[at], [0.75, 0.0, 0.0]);
    assert_eq!(
        &cd[..at],
        &column::<[f32; 3]>(&value, Domain::Point, names::CD)[..]
    );
    let rest = column::<[f64; 3]>(&out, Domain::Point, names::REST)[at];
    assert_eq!(rest, [0.75, 0.0, 0.0]);
    // A normal blends to unit length.
    let n = column::<[f32; 3]>(&out, Domain::Point, names::N)[at];
    let length = n.iter().map(|x| x * x).sum::<f32>().sqrt();
    assert!((length - 1.0).abs() < 1e-5 && n[1] > n[0], "{n:?}");
    // A rotation along the arc: a half turn's 0.75 is 135° about Z.
    let turn = column::<[f32; 4]>(&out, Domain::Point, names::ORIENT)[at];
    let half = 0.75 * FRAC_PI_2;
    let want = [0.0, 0.0, half.sin(), half.cos()];
    for (a, b) in turn.iter().zip(want) {
        assert!((a - b).abs() < 1e-5, "{turn:?}");
    }
    // Integers, text and arrays from the strongest source.
    assert_eq!(column::<i32>(&out, Domain::Point, "tag")[at], 1);
    let label = out.attributes().text(Domain::Point, "label").unwrap();
    assert_eq!(label.get(at), Some("p1"));
    let tags = out.attributes().get(Domain::Point, "tags").unwrap();
    let offsets = tags.offsets().unwrap();
    let last = offsets.len() - 1;
    let items: &[i32] = tags.as_tuples().unwrap();
    assert_eq!(&items[offsets[last - 1] as usize..], &[2, 3]);
    // A fresh id above them all; the group from the strongest source (1 is odd).
    assert_eq!(column::<i64>(&out, Domain::Point, names::ID)[at], 109);
    assert!(!members(&out, Domain::Point, "even").contains(&q));
    // Other domains are the same columns.
    for (domain, name) in [
        (Domain::Primitive, names::CD),
        (Domain::Vertex, names::UV),
        (Domain::Edge, "crease"),
        (Domain::Detail, "tint"),
    ] {
        assert_eq!(key(&out, domain, name), key(&value, domain, name), "{name}");
    }
    let (new, old) = (out.primitives(), value.primitives());
    assert_eq!(new.offsets().key(), old.offsets().key());
    assert_eq!(new.vertices().key(), old.vertices().key());
}

#[test]
fn a_new_element_without_sources_takes_its_types_default() {
    let value = ruled(1, 1);
    let mut edit = Editor::new(&value);
    let q = edit.add_point([0.0; 3], &[]) as usize;
    let (out, _) = edit.finish().unwrap();
    let cd = column::<[f32; 3]>(&out, Domain::Point, names::CD);
    assert_eq!(cd[q], [1.0; 3], "white");
    let orient = column::<[f32; 4]>(&out, Domain::Point, names::ORIENT);
    assert_eq!(orient[q], [0.0, 0.0, 0.0, 1.0]);
    let rest = column::<[f64; 3]>(&out, Domain::Point, names::REST);
    assert_eq!(rest[q], [0.0; 3]);
    assert_eq!(column::<i32>(&out, Domain::Point, "tag")[q], 0);
    let label = out.attributes().text(Domain::Point, "label").unwrap();
    assert_eq!(label.get(q), None);
    assert_eq!(column::<i64>(&out, Domain::Point, names::ID)[q], 104);
    assert!(!members(&out, Domain::Point, "even").contains(&(q as u32)));
}

#[test]
fn sources_may_name_elements_the_same_edit_added() {
    let value = ruled(1, 1);
    let mut edit = Editor::new(&value);
    let mid = edit.add_point([0.0; 3], &[(0, 0.5), (1, 0.5)]);
    let quarter = edit.add_point([0.0; 3], &[(0, 0.5), (mid, 0.5)]);
    let (out, _) = edit.finish().unwrap();
    let rest = column::<[f64; 3]>(&out, Domain::Point, names::REST);
    assert_eq!(rest[mid as usize], [0.5, 0.0, 0.0]);
    assert_eq!(rest[quarter as usize], [0.25, 0.0, 0.0]);
    let ids = column::<i64>(&out, Domain::Point, names::ID);
    assert_eq!(&ids[4..], &[104, 105], "fresh ids in order");
}

#[test]
fn numbers_stay_put_during_an_edit_and_the_change_says_where_they_went() {
    let value = ruled(2, 2);
    let mut edit = Editor::new(&value);
    let q = edit.add_point([7.0; 3], &[(8, 1.0)]);
    edit.remove_point(0);
    edit.set_position(8, [8.0; 3]);
    let removed_new = edit.add_point([6.0; 3], &[]);
    edit.remove_point(removed_new);
    let f = edit.add_primitive(Kind::POLYGON, &corners(&[8, 7, q]), &[(3, 1.0)]);
    let gone = edit.add_primitive(Kind::POLYGON, &corners(&[8, 7, q]), &[]);
    edit.remove_primitive(gone);
    assert_eq!((edit.point_count(), edit.primitive_count()), (11, 6));
    let (out, change) = edit.finish().unwrap();
    assert_eq!(change.points.of(q), Some(8));
    assert_eq!(change.points.of(0), None);
    assert_eq!(change.points.of(removed_new), None);
    assert_eq!(change.points.added, 8..9);
    assert_eq!(change.moved, [7], "point 8 is 7 now");
    assert_eq!(out.positions()[7], [8.0; 3]);
    assert_eq!(out.positions()[8], [7.0; 3]);
    assert_eq!(change.primitives.of(f), Some(4));
    assert_eq!(change.primitives.of(gone), None);
    assert_eq!(change.primitives.added, 4..5);
    assert_eq!(change.primitives.removed, [gone]);
    assert_eq!(out.primitives().primitive(4), [7, 6, 8]);
    let ids = column::<i64>(&out, Domain::Point, names::ID);
    assert_eq!(ids.last(), Some(&109), "the survivor's");
    assert_eq!(column::<i32>(&out, Domain::Point, "tag")[8], 8);
}

#[test]
fn selections_find_their_elements_again_through_ids() {
    let value = ruled(2, 2);
    let members = Elements::from_indices(9, [3, 5]).unwrap();
    let s = Selection::on(&value, Domain::Point, members, Some(5)).unwrap();
    let mut edit = Editor::new(&value);
    edit.remove_point(0);
    edit.add_point([0.0; 3], &[(5, 1.0)]);
    let (out, _) = edit.finish().unwrap();
    assert!(s.validate(&out).is_err(), "numbers mean others now");
    let found = select::reresolve(&s, &out).unwrap();
    assert_eq!(found.members().iter().collect::<Vec<_>>(), [2, 4]);
    assert_eq!(found.active(), Some(4));
    // A points-only edit leaves the selection itself valid.
    let mut edit = Editor::new(&value);
    edit.set_position(3, [1.0; 3]);
    let (out, _) = edit.finish().unwrap();
    assert!(s.validate(&out).is_ok());
}

#[test]
fn a_bad_number_or_source_is_reported_at_the_finish() {
    let value = ruled(1, 1);
    type Case = Box<dyn Fn(&mut Editor)>;
    let bad: [Case; 8] = [
        Box::new(|e| e.set_position(99, [0.0; 3])),
        Box::new(|e| {
            e.add_point([0.0; 3], &[(99, 1.0)]);
        }),
        Box::new(|e| {
            e.add_point([0.0; 3], &[(0, -1.0)]);
        }),
        Box::new(|e| e.remove_point(99)),
        Box::new(|e| e.remove_primitive(99)),
        Box::new(|e| e.rewire(99, &corners(&[0, 1, 2]))),
        Box::new(|e| {
            e.add_primitive(Kind::POLYGON, &corners(&[0, 1, 99]), &[]);
        }),
        Box::new(|e| {
            let corner = Corner {
                point: 0,
                from: &[(99, 1.0)],
            };
            e.add_primitive(Kind::POLYGON, &[corner], &[]);
        }),
    ];
    for (i, f) in bad.iter().enumerate() {
        let mut edit = Editor::new(&value);
        f(&mut edit);
        assert!(edit.finish().is_err(), "case {i}");
    }
    // A curve lands with a kernel that makes them.
    let mut edit = Editor::new(&value);
    edit.add_primitive(Kind::curve(Basis::Bezier, false), &corners(&[0, 1]), &[]);
    assert!(edit.finish().is_err());
}

#[test]
fn each_rule_is_declared_by_storage_and_meaning() {
    let ty = |s, w, m| AttributeType::new(s, w, m);
    assert_eq!(rule(ty(Storage::F32, 3, Meaning::Vector)), Rule::Lerp);
    assert_eq!(rule(ty(Storage::F64, 3, Meaning::Point)), Rule::Lerp);
    assert_eq!(rule(ty(Storage::F32, 3, Meaning::Color)), Rule::Lerp);
    assert_eq!(rule(ty(Storage::F32, 16, Meaning::Transform)), Rule::Lerp);
    assert_eq!(rule(ty(Storage::F32, 3, Meaning::Normal)), Rule::Normalize);
    assert_eq!(rule(ty(Storage::F32, 4, Meaning::Rotation)), Rule::Slerp);
    assert_eq!(rule(ty(Storage::I64, 1, Meaning::None)), Rule::Strongest);
    assert_eq!(rule(AttributeType::TEXT), Rule::Strongest);
    let array = AttributeType {
        array: true,
        ..ty(Storage::F32, 3, Meaning::Vector)
    };
    assert_eq!(rule(array), Rule::Strongest);
}
