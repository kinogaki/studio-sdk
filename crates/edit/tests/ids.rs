//! The stable-id rule: unique ids, fresh ones above the maximum, and how
//! a Merge keeps them.
use kinogaki_edit::ids;
use kinogaki_geometry::{Attribute, Attributes, Domain, Meaning, names};

#[test]
fn a_repeated_id_is_refused_and_fresh_ids_are_above_the_maximum() {
    let column = |ids: Vec<i64>| Attribute::tuples(ids, Meaning::None).unwrap();
    let table = Attributes::new();
    assert!(
        table
            .with(Domain::Point, names::ID, column(vec![3, 1, 2]))
            .is_ok()
    );
    let e = table
        .with(Domain::Primitive, names::ID, column(vec![3, 1, 3]))
        .unwrap_err();
    assert_eq!(e.message(), "primitive id 3 names two elements");
    // A column past the parallel block size is checked in buckets alike.
    let mut many: Vec<i64> = (0..300_000).map(|i| i * 7 - 1_000_000).collect();
    assert!(
        table
            .with(Domain::Point, names::ID, column(many.clone()))
            .is_ok()
    );
    many[250_000] = many[3];
    assert!(
        table
            .with(Domain::Point, names::ID, column(many.clone()))
            .is_err()
    );
    let fresh = ids::fresh(&many.clone().into(), 2).unwrap();
    assert!(fresh.start > *many.iter().max().unwrap() && fresh.end - fresh.start == 2);
}
