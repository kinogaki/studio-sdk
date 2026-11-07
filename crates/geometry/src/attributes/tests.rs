//! The table's checks: a value re-checks only the entries it doesn't
//! already hold.
use super::{Attribute, Attributes, Domain, Meaning};
use crate::Elements;

#[test]
fn only_replaced_entries_are_checked() {
    let floats = |n: usize| Attribute::tuples(vec![0.5f32; n], Meaning::None).unwrap();
    let mut table = Attributes::new();
    for name in ["a", "b", "c", "d"] {
        table = table.with(Domain::Point, name, floats(4)).unwrap();
    }
    table = table
        .with_group(Domain::Point, "g", Elements::none(4))
        .unwrap();
    let count = |d: Domain| (d == Domain::Point).then_some(4);
    assert_eq!(table.check("polygons", None, count), Ok(5));
    assert_eq!(table.check("polygons", Some(&table), count), Ok(0));
    let edited = table.with(Domain::Point, "c", floats(4)).unwrap();
    assert_eq!(edited.check("polygons", Some(&table), count), Ok(1));
    let wrong = table.with(Domain::Point, "c", floats(3)).unwrap();
    assert!(wrong.check("polygons", Some(&table), count).is_err());
    let elsewhere = table.with(Domain::Edge, "e", floats(4)).unwrap();
    assert!(elsewhere.check("polygons", Some(&table), count).is_err());
}

#[test]
fn a_table_being_made_sets_entries_in_place_and_checks_each() {
    let floats = Attribute::tuples(vec![0.5f32; 4], Meaning::None).unwrap();
    let mut table = Attributes::new();
    // Each set copies nothing: a hundred thousand take time in proportion.
    for k in 0..100_000 {
        table
            .set(Domain::Detail, &format!("a{k}"), floats.clone())
            .unwrap();
    }
    table
        .set_group(Domain::Point, "g", Elements::none(4))
        .unwrap();
    assert_eq!(table.iter().count(), 100_000);
    assert_eq!(
        table,
        table.with(Domain::Detail, "a0", floats.clone()).unwrap()
    );
    // Refused as `with` and `with_group` refuse, the table as it was.
    let before = table.clone();
    let reals = Attribute::tuples(vec![[0.0f64; 3]; 4], Meaning::Color).unwrap();
    assert!(table.set(Domain::Point, crate::names::CD, reals).is_err());
    assert!(
        table
            .set_group(Domain::Point, crate::names::SHARP, Elements::none(4))
            .is_err()
    );
    assert_eq!(table, before);
}
