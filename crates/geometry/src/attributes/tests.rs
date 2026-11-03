//! The table's checks: a value re-checks only the entries it doesn't
//! already hold.
use super::{Attribute, Attributes, Domain, Group, Meaning};

#[test]
fn only_replaced_entries_are_checked() {
    let floats = |n: usize| Attribute::tuples(vec![0.5f32; n], Meaning::None).unwrap();
    let mut table = Attributes::new();
    for name in ["a", "b", "c", "d"] {
        table = table.with(Domain::Point, name, floats(4)).unwrap();
    }
    table = table
        .with_group(Domain::Point, "g", Group::none(4))
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
