//! The attribute model: types, standard names, adoption without copies,
//! arrays, indexed text, groups, and sharing between tables.
use kinogaki_geometry::{
    Attribute, AttributeType, Attributes, Column, Domain, Elements, ErrorKind, Geometry, Meaning,
    Primitives, Storage, Text, Tuple, Values, check_name, limits::MAX_NAME, names,
};
use std::sync::Arc;

fn f32s<const N: usize>(n: usize, meaning: Meaning) -> Attribute
where
    [f32; N]: Tuple,
{
    Attribute::tuples(vec![[0.5f32; N]; n], meaning).unwrap()
}

#[test]
fn names_follow_the_rule() {
    for good in ["P", "_x", ".hidden", "set:top", "a/b-c.d", "Größe", "x2"] {
        assert!(check_name(good).is_ok(), "{good}");
    }
    let message = |n: &str| check_name(n).unwrap_err().message().to_owned();
    assert_eq!(message(""), "an attribute needs a name");
    assert_eq!(message("2x"), "a name cannot start with `2`");
    assert_eq!(message(":x"), "a name cannot start with `:`");
    assert_eq!(message("a b"), "` ` cannot appear in a name");
    assert_eq!(message("a*"), "`*` cannot appear in a name");
    assert!(check_name(&"a".repeat(MAX_NAME)).is_ok());
    assert_eq!(
        message(&"a".repeat(MAX_NAME + 1)),
        "a name has at most 256 bytes"
    );
    let bad = Attributes::new().with(Domain::Point, "a b", f32s::<1>(1, Meaning::None));
    assert_eq!(bad.unwrap_err().kind, ErrorKind::Invalid);
}

#[test]
fn every_storage_width_and_meaning_is_checked() {
    for storage in Storage::ALL {
        for meaning in Meaning::ALL {
            for width in [0u8, 1, 2, 3, 4, 9, 16, 255] {
                for array in [false, true] {
                    let ty = AttributeType {
                        storage,
                        width,
                        meaning,
                        array,
                    };
                    let fits = width > 0
                        && match (storage, meaning) {
                            (Storage::Text, Meaning::None) => width == 1,
                            (Storage::Text, _) => false,
                            (_, Meaning::None) => true,
                            (s, m) => s.is_real() && m.widths().contains(&width),
                        };
                    assert_eq!(ty.check().is_ok(), fits, "{ty:?}");
                }
            }
        }
    }
    let message = |ty: AttributeType| ty.check().unwrap_err().message().to_owned();
    assert_eq!(
        message(AttributeType::new(Storage::F32, 2, Meaning::Color)),
        "color attributes have width 3 or 4, not 2"
    );
    assert_eq!(
        message(AttributeType::new(Storage::I64, 3, Meaning::Normal)),
        "normal attributes hold reals, not i64"
    );
    // A bad width for its meaning refuses the attribute too.
    let flat = Values::F32(Column::new(vec![0.0; 6]));
    let short = Attribute::new(AttributeType::new(Storage::F32, 2, Meaning::Color), flat);
    assert!(short.is_err());
}

#[test]
fn values_are_whole_items_of_their_storage_and_finite() {
    let ty = AttributeType::new(Storage::F64, 3, Meaning::None);
    let values = |v: Vec<f64>| Values::F64(Column::new(v));
    assert_eq!(
        Attribute::new(ty, values(vec![0.0; 6])).unwrap().elements(),
        2
    );
    let partial = Attribute::new(ty, values(vec![0.0; 7])).unwrap_err();
    assert_eq!(partial.message(), "7 values are not whole items of 3");
    let wrong = Attribute::new(ty, Values::F32(Column::new(vec![0.0; 6]))).unwrap_err();
    assert_eq!(wrong.message(), "f32 values for a f64 × 3 attribute");
    let mut bad = vec![0.0; 9];
    bad[7] = f64::NAN;
    let nan = Attribute::new(ty, values(bad)).unwrap_err();
    assert_eq!(
        (nan.diagnostic.element, nan.message()),
        (Some(2), "value 7 is not finite")
    );
    let infinite = Attribute::tuples(vec![[0.0f32, f32::INFINITY]], Meaning::None);
    assert!(infinite.is_err());
    // Whole numbers are any whole numbers.
    let ids = Attribute::tuples(vec![i64::MIN, i64::MAX, (1 << 53) + 1], Meaning::None).unwrap();
    assert_eq!(
        ids.as_tuples::<i64>(),
        Some(&[i64::MIN, i64::MAX, (1 << 53) + 1][..])
    );
}

#[test]
fn tuples_adopt_without_copying_and_read_back_typed() {
    let normals = vec![[0.0f32, 0.0, 1.0]; 1000];
    let address = normals.as_ptr() as usize;
    let n = Attribute::tuples(normals, Meaning::Normal).unwrap();
    assert_eq!(n.ty(), AttributeType::new(Storage::F32, 3, Meaning::Normal));
    let Values::F32(flat) = n.values() else {
        panic!("f32 values")
    };
    assert_eq!(flat.as_slice().as_ptr() as usize, address, "adopted");
    let view: &[[f32; 3]] = n.as_tuples().unwrap();
    assert_eq!(view.as_ptr() as usize, address, "viewed in place");
    assert_eq!((view.len(), n.elements(), n.items()), (1000, 1000, 1000));
    // The wrong storage or width is no view.
    assert!(n.as_tuples::<[f64; 3]>().is_none());
    assert!(n.as_tuples::<[f32; 4]>().is_none());
    assert!(n.as_tuples::<f32>().is_none());
    // Every width a tuple has: matrices and quaternions included.
    let m = Attribute::tuples(vec![[1.0f64; 16]; 2], Meaning::Transform).unwrap();
    assert_eq!((m.ty().width, m.elements()), (16, 2));
    let q = Attribute::tuples(vec![[0.0f32, 0.0, 0.0, 1.0]; 3], Meaning::Rotation).unwrap();
    assert_eq!(q.as_tuples::<[f32; 4]>().map(<[_]>::len), Some(3));
    let pairs = Attribute::tuples(vec![[7i32, 8]; 5], Meaning::None).unwrap();
    assert_eq!(pairs.as_tuples::<[i32; 2]>().unwrap()[4], [7, 8]);
    // A width past a tuple's is made from flat values.
    let wide = AttributeType::new(Storage::I32, 255, Meaning::None);
    let wide = Attribute::new(wide, Values::I32(Column::new(vec![1; 510]))).unwrap();
    assert_eq!(wide.elements(), 2);
}

#[test]
fn arrays_index_items_through_offsets() {
    let ty = AttributeType {
        array: true,
        ..AttributeType::new(Storage::I32, 2, Meaning::None)
    };
    let items = || Values::I32(Column::new(vec![1, 2, 3, 4, 5, 6]));
    // Three elements: one item, none, two.
    let a = Attribute::arrays(ty, vec![0, 1, 1, 3].into(), items()).unwrap();
    assert_eq!((a.elements(), a.items()), (3, 3));
    assert_eq!(a.offsets().map(|o| o.to_vec()), Some(vec![0, 1, 1, 3]));
    assert_eq!(a.as_tuples::<[i32; 2]>().unwrap()[2], [5, 6]);
    // Empty arrays, and no elements, are allowed.
    let none = Values::I32(Column::default());
    assert!(Attribute::arrays(ty, vec![0, 0].into(), none.clone()).is_ok());
    assert!(Attribute::arrays(ty, vec![0].into(), none).is_ok());
    let refuse = |offsets: Vec<u32>| {
        Attribute::arrays(ty, offsets.into(), items())
            .unwrap_err()
            .message()
            .to_owned()
    };
    assert_eq!(
        refuse(vec![1, 3]),
        "an array attribute's offsets start at 0"
    );
    assert_eq!(refuse(vec![]), "an array attribute's offsets start at 0");
    assert_eq!(
        refuse(vec![0, 2, 1, 3]),
        "element 1's array ends before it starts"
    );
    assert_eq!(refuse(vec![0, 2]), "the arrays end at item 2 of 3 items");
    // Arrays and single items are made apart.
    assert!(Attribute::new(ty, items()).is_err());
    let single = AttributeType::new(Storage::I32, 2, Meaning::None);
    assert!(Attribute::arrays(single, vec![0, 3].into(), items()).is_err());
    // Text arrays: a list of names a detail.
    let names = AttributeType {
        array: true,
        ..AttributeType::TEXT
    };
    let rules = Text::from_strs(["F", "+F", "-F"]);
    let list = Attribute::arrays(names, vec![0, 3].into(), Values::Text(rules)).unwrap();
    assert_eq!(list.elements(), 1);
    let detail = Attributes::new()
        .with(Domain::Detail, "rules", list)
        .unwrap();
    assert!(detail.get(Domain::Detail, "rules").is_some());
}

#[test]
fn standard_names_fix_their_type_and_domains() {
    let t = Attributes::new();
    let text = || Attribute::text(Text::from_strs(["a"]));
    let ok = [
        (Domain::Point, names::N, f32s::<3>(1, Meaning::Normal)),
        (Domain::Vertex, names::N, f32s::<3>(1, Meaning::Normal)),
        (Domain::Primitive, names::N, f32s::<3>(1, Meaning::Normal)),
        (
            Domain::Vertex,
            names::UV,
            f32s::<2>(1, Meaning::TextureCoord),
        ),
        (Domain::Primitive, names::CD, f32s::<3>(1, Meaning::Color)),
        (Domain::Detail, names::CD, f32s::<3>(1, Meaning::Color)),
        (Domain::Point, names::ALPHA, f32s::<1>(1, Meaning::None)),
        (Domain::Point, names::V, f32s::<3>(1, Meaning::Vector)),
        (Domain::Instance, names::PSCALE, f32s::<1>(1, Meaning::None)),
        (Domain::Point, names::SCALE, f32s::<3>(1, Meaning::None)),
        (
            Domain::Point,
            names::ORIENT,
            f32s::<4>(1, Meaning::Rotation),
        ),
        (Domain::Point, names::UP, f32s::<3>(1, Meaning::Vector)),
        (Domain::Point, names::WIDTH, f32s::<1>(1, Meaning::None)),
        (Domain::Primitive, names::MATERIAL, text()),
        (Domain::Primitive, names::NAME, text()),
        (Domain::Object, names::PART, text()),
        (
            Domain::Point,
            names::ID,
            Attribute::tuples(vec![1i64], Meaning::None).unwrap(),
        ),
        (
            Domain::Point,
            names::REST,
            Attribute::tuples(vec![[0.0f64; 3]], Meaning::Point).unwrap(),
        ),
    ];
    for (domain, name, attribute) in ok {
        assert!(
            t.with(domain, name, attribute).is_ok(),
            "{name} on {domain:?}"
        );
    }
    let refused = [
        (Domain::Point, names::CD, f32s::<1>(1, Meaning::None)),
        (Domain::Point, names::CD, f32s::<3>(1, Meaning::None)),
        (Domain::Point, names::N, f32s::<3>(1, Meaning::Vector)),
        (Domain::Edge, names::N, f32s::<3>(1, Meaning::Normal)),
        (
            Domain::Point,
            names::UV,
            f32s::<3>(1, Meaning::TextureCoord),
        ),
        (
            Domain::Primitive,
            names::UV,
            f32s::<2>(1, Meaning::TextureCoord),
        ),
        (Domain::Vertex, names::UP, f32s::<3>(1, Meaning::Vector)),
        (
            Domain::Point,
            names::ID,
            Attribute::tuples(vec![1i32], Meaning::None).unwrap(),
        ),
        (
            Domain::Point,
            names::REST,
            Attribute::tuples(vec![[0.0f32; 3]], Meaning::Point).unwrap(),
        ),
        (
            Domain::Primitive,
            names::MATERIAL,
            f32s::<1>(1, Meaning::None),
        ),
        (
            Domain::Point,
            names::P,
            Attribute::tuples(vec![[0.0f64; 3]], Meaning::Point).unwrap(),
        ),
        (
            Domain::Vertex,
            names::P,
            Attribute::tuples(vec![[0.0f64; 3]], Meaning::Point).unwrap(),
        ),
        (Domain::Edge, names::SHARP, f32s::<1>(1, Meaning::None)),
    ];
    for (domain, name, attribute) in refused {
        assert!(
            t.with(domain, name, attribute).is_err(),
            "{name} on {domain:?}"
        );
    }
    let message = |d, name, a| t.with(d, name, a).unwrap_err().message().to_owned();
    assert_eq!(
        message(Domain::Point, names::CD, f32s::<4>(1, Meaning::Color)),
        "Cd is f32 × 3 color, not f32 × 4 color"
    );
    assert_eq!(
        message(Domain::Edge, names::N, f32s::<3>(1, Meaning::Normal)),
        "N is not an attribute of the edge domain (only point, vertex, primitive)"
    );
    assert_eq!(
        message(Domain::Point, names::P, f32s::<3>(1, Meaning::Point)),
        "P is the value's own column, never an attribute"
    );
}

#[test]
fn a_value_refuses_a_domain_it_does_not_have() {
    let quad = Primitives::of_polygons(4, vec![0, 4].into(), vec![0, 1, 2, 3].into()).unwrap();
    let square = Geometry::new(vec![[0.0; 3]; 4].into(), quad, Attributes::new()).unwrap();
    for domain in [Domain::Object, Domain::Instance] {
        let table = Attributes::new().with(domain, "w", f32s::<1>(1, Meaning::None));
        let refused = square.with_attributes(table.unwrap()).unwrap_err();
        assert_eq!(
            refused.message(),
            format!("polygons have no {} domain", domain.name())
        );
    }
}

#[test]
fn replacing_one_column_shares_the_rest_and_checks_it_alone() {
    let quad = Primitives::of_polygons(4, vec![0, 4].into(), vec![0, 1, 2, 3].into()).unwrap();
    let mut table = Attributes::new();
    for name in ["a", "b", "c"] {
        table = table
            .with(Domain::Point, name, f32s::<3>(4, Meaning::None))
            .unwrap();
    }
    let square = Geometry::new(vec![[0.0; 3]; 4].into(), quad, table).unwrap();
    let edited = (square
        .attributes()
        .with(Domain::Point, "b", f32s::<3>(4, Meaning::None)))
    .unwrap();
    let moved = square.with_attributes(edited).unwrap();
    let same = |name| {
        std::ptr::eq(
            square.attributes().get(Domain::Point, name).unwrap(),
            moved.attributes().get(Domain::Point, name).unwrap(),
        )
    };
    assert!(same("a") && !same("b") && same("c"));
    let short = square
        .attributes()
        .with(Domain::Point, "b", f32s::<3>(3, Meaning::None));
    let refused = square.with_attributes(short.unwrap()).unwrap_err();
    assert_eq!(
        refused.message(),
        "point attribute b has 3 values for 4 elements"
    );
}

#[test]
fn standard_groups_are_on_their_domain() {
    let t = Attributes::new();
    assert!(
        t.with_group(Domain::Edge, names::SHARP, Elements::none(3))
            .is_ok()
    );
    assert!(
        t.with_group(Domain::Primitive, names::HIDDEN, Elements::none(3))
            .is_ok()
    );
    let wrong = t.with_group(Domain::Point, names::SHARP, Elements::none(3));
    assert_eq!(
        wrong.unwrap_err().message(),
        "sharp is a group of the edge domain, not of the point domain"
    );
    assert!(
        t.with_group(Domain::Point, names::CD, Elements::none(3))
            .is_err()
    );
    assert!(
        t.with_group(Domain::Point, "set:top", Elements::none(3))
            .is_ok()
    );
}

#[test]
fn text_rules_are_checked() {
    let table = |s: &[&str]| -> Arc<[Arc<str>]> { s.iter().map(|&s| Arc::from(s)).collect() };
    assert!(Text::new(vec![0, 1, Text::NONE].into(), table(&["a", "b"])).is_ok());
    let past = Text::new(vec![0, 2].into(), table(&["a", "b"])).unwrap_err();
    assert_eq!(past.diagnostic.element, Some(1));
    assert_eq!(
        past.message(),
        "element 1's text index 2 is past the table of 2"
    );
    let empty = Text::new(vec![0].into(), table(&["a", ""])).unwrap_err();
    assert_eq!(
        empty.message(),
        "text table entry 1 is empty: an empty string is no value"
    );
    let repeated = Text::new(vec![0].into(), table(&["a", "a"])).unwrap_err();
    assert_eq!(repeated.message(), "text table entry 1 repeats \"a\"");
}

#[test]
fn texts_are_equal_by_their_values_whatever_their_tables() {
    let table: Arc<[Arc<str>]> = ["a", "b", "c"].map(Arc::from).into();
    // A gather keeping its source's table, unused and reordered entries included.
    let gathered = Text::new(vec![2, 0, Text::NONE].into(), table.clone()).unwrap();
    let fresh = Text::from_strs(["c", "a", ""]);
    assert_eq!(gathered, fresh);
    assert_eq!(gathered.compacted(), fresh);
    // Other values differ, over one table or two.
    let other = Text::new(vec![2, 1, Text::NONE].into(), table.clone()).unwrap();
    assert_ne!(gathered, other);
    assert_ne!(other, fresh);
    assert_ne!(Text::from_strs(["c", "a"]), fresh);
    // No value is not a value, and the empty texts are equal.
    assert_ne!(Text::from_strs(["", "a"]), Text::from_strs(["b", "a"]));
    assert_eq!(
        Text::from_strs(["", ""]),
        Text::new(vec![Text::NONE; 2].into(), table).unwrap()
    );
    // A compact text compacts to itself, sharing everything.
    let compact = fresh.compacted();
    assert!(compact.indices().ptr_eq(fresh.indices()));
}

#[test]
fn domains_storages_and_meanings_name_themselves() {
    for domain in Domain::ALL {
        assert_eq!(Domain::from_name(domain.name()), Some(domain));
    }
    for storage in Storage::ALL {
        assert_eq!(Storage::from_name(storage.name()), Some(storage));
    }
    for meaning in Meaning::ALL {
        assert_eq!(Meaning::from_name(meaning.name()), Some(meaning));
    }
    assert_eq!(Domain::from_name("face"), None);
    assert_eq!(Domain::from_name("piece"), None);
}
