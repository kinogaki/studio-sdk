//! The attribute model: names, standard types, indexed text, groups, and
//! sharing between tables.
use kinogaki_geometry::{
    Attribute, AttributeKind, Attributes, Class, ErrorKind, Group, Text, TextBuilder, TypeInfo,
    check_name, limits::MAX_NAME, names,
};
use std::sync::Arc;

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
    let bad = Attributes::new().with(Class::Point, "a b", Attribute::float(vec![0.0]));
    assert_eq!(bad.unwrap_err().kind(), ErrorKind::Attribute);
}

#[test]
fn standard_names_keep_their_types() {
    let t = Attributes::new();
    let vector = |info| Attribute::vector(vec![[0.0; 3]], info);
    let ok = [
        (Class::Point, names::N, vector(TypeInfo::Normal)),
        (Class::Vertex, names::UV, vector(TypeInfo::TextureCoord)),
        (Class::Primitive, names::CD, vector(TypeInfo::Color)),
        (Class::Point, names::V, vector(TypeInfo::Vector)),
        (Class::Point, names::ALPHA, Attribute::float(vec![1.0])),
        (Class::Point, names::WIDTH, Attribute::float(vec![1.0])),
        (
            Class::Primitive,
            names::MATERIAL,
            Attribute::text(Text::from_strs(["/M"])),
        ),
        (
            Class::Primitive,
            names::NAME,
            Attribute::text(Text::from_strs(["a"])),
        ),
        (
            Class::Primitive,
            names::PART,
            Attribute::text(Text::from_strs(["a"])),
        ),
        (
            Class::Instance,
            names::INSTANCE,
            Attribute::integer(vec![0]),
        ),
        (Class::Vertex, names::P, vector(TypeInfo::Point)),
    ];
    for (class, name, attribute) in ok {
        assert!(t.with(class, name, attribute).is_ok(), "{name}");
    }
    let refused = [
        (Class::Point, names::CD, Attribute::float(vec![0.0])),
        (Class::Point, names::CD, vector(TypeInfo::None)),
        (Class::Point, names::N, vector(TypeInfo::Vector)),
        (
            Class::Primitive,
            names::MATERIAL,
            Attribute::integer(vec![0]),
        ),
        (Class::Point, names::P, vector(TypeInfo::Point)),
        (Class::Edge, names::SHARP, Attribute::integer(vec![0])),
    ];
    for (class, name, attribute) in refused {
        assert!(t.with(class, name, attribute).is_err(), "{name}");
    }
    let err = t.with(Class::Point, names::CD, Attribute::float(vec![0.0]));
    assert_eq!(
        err.unwrap_err().message(),
        "Cd is a vector attribute (color), not float (none)"
    );
}

#[test]
fn standard_groups_are_on_their_class() {
    let t = Attributes::new();
    assert!(
        t.with_group(Class::Edge, names::SHARP, Group::none(3))
            .is_ok()
    );
    assert!(
        t.with_group(Class::Primitive, names::HIDDEN, Group::none(3))
            .is_ok()
    );
    let wrong = t.with_group(Class::Point, names::SHARP, Group::none(3));
    assert_eq!(
        wrong.unwrap_err().message(),
        "sharp is a group of the edge class, not of the point class"
    );
    assert!(
        t.with_group(Class::Point, names::CD, Group::none(3))
            .is_err()
    );
    assert!(
        t.with_group(Class::Point, "set:top", Group::none(3))
            .is_ok()
    );
}

#[test]
fn with_shares_every_other_entry() {
    let n = Attribute::vector(vec![[0.0, 0.0, 1.0]; 2], TypeInfo::Normal);
    let a = Attributes::new().with(Class::Point, names::N, n).unwrap();
    let b = a
        .with(Class::Point, "w", Attribute::float(vec![1.0, 2.0]))
        .unwrap();
    assert!(Arc::ptr_eq(
        a.get_shared(Class::Point, names::N).unwrap(),
        b.get_shared(Class::Point, names::N).unwrap()
    ));
    assert!(
        a.get(Class::Point, "w").is_none(),
        "the original is unchanged"
    );
    let c = b.without(Class::Point, "w");
    assert_eq!(c, a);
    assert_eq!(b.without(Class::Point, "missing"), b);
    assert_eq!(
        b.float(Class::Point, "w").map(|c| c.to_vec()),
        Some(vec![1.0, 2.0])
    );
    assert!(
        b.vector(Class::Point, "w").is_none(),
        "typed reads check the kind"
    );
    let listed: Vec<_> = b
        .iter()
        .map(|(c, n, a)| (c, n.to_owned(), a.kind()))
        .collect();
    assert_eq!(
        listed,
        [
            (Class::Point, "N".to_owned(), AttributeKind::Vector),
            (Class::Point, "w".to_owned(), AttributeKind::Float)
        ]
    );
    let grouped = b.with_group(Class::Point, "w", Group::none(2)).unwrap();
    assert!(grouped.get(Class::Point, "w").is_some() && grouped.group(Class::Point, "w").is_some());
    assert!(
        grouped
            .without_group(Class::Point, "w")
            .group(Class::Point, "w")
            .is_none()
    );
}

#[test]
fn text_is_indexed_and_empty_is_no_value() {
    let mut b = TextBuilder::new();
    assert_eq!(b.push("a"), 0);
    assert_eq!(b.push(""), Text::NONE);
    assert_eq!(b.push("b"), 1);
    assert_eq!(b.push("a"), 0);
    b.push_none();
    let red = b.intern("red");
    b.push_index(red);
    let text = b.finish();
    assert_eq!(&**text.indices(), &[0, Text::NONE, 1, 0, Text::NONE, 2]);
    assert_eq!(text.table().len(), 3);
    assert_eq!(
        (0..text.len()).map(|i| text.get(i)).collect::<Vec<_>>(),
        [Some("a"), None, Some("b"), Some("a"), None, Some("red")]
    );
}

#[test]
fn text_rules_are_checked() {
    let table = |s: &[&str]| -> Arc<[Arc<str>]> { s.iter().map(|&s| Arc::from(s)).collect() };
    assert!(Text::new(vec![0, 1, Text::NONE].into(), table(&["a", "b"])).is_ok());
    let past = Text::new(vec![0, 2].into(), table(&["a", "b"])).unwrap_err();
    assert_eq!(past.element(), Some(1));
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
#[should_panic(expected = "text index 5 was not interned here")]
fn a_foreign_text_index_panics() {
    TextBuilder::new().push_index(5);
}

#[test]
fn group_rules_are_checked() {
    let g = Group::from_indices(130, [0, 64, 129]).unwrap();
    assert_eq!((g.elements(), g.members(), g.words().len()), (130, 3, 3));
    assert_eq!(g.iter().collect::<Vec<_>>(), [0, 64, 129]);
    assert!(g.contains(129) && !g.contains(130) && !g.contains(1000));
    assert!(Group::new(130, g.words().clone()).is_ok());
    let few = Group::new(130, vec![0, 0].into()).unwrap_err();
    assert_eq!(few.message(), "a group of 130 elements has 2 words, not 3");
    let past = Group::new(130, vec![0, 0, 1 << 2].into()).unwrap_err();
    assert_eq!(
        past.message(),
        "a group of 130 elements holds bits past its last element"
    );
    assert!(Group::new(128, vec![u64::MAX, u64::MAX].into()).is_ok());
    let out = Group::from_indices(4, [4]).unwrap_err();
    assert_eq!(
        (out.element(), out.message()),
        (Some(4), "group member 4 is past its 4 elements")
    );
    assert_eq!(Group::none(0).words().len(), 0);
}

#[test]
fn classes_and_kinds_name_themselves() {
    for class in Class::ALL {
        assert_eq!(Class::from_name(class.name()), Some(class));
    }
    for info in TypeInfo::ALL {
        assert_eq!(TypeInfo::from_name(info.name()), Some(info));
    }
    assert_eq!(Class::from_name("face"), None);
}
