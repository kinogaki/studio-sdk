//! `kinogaki.core.brep` as a data type: its codec over the B-rep
//! fixtures, refusals of broken saves, counts across bodies, and the
//! registered meshers.
use kinogaki_studio_api::element::{Field, MemoryElement};
use kinogaki_studio_api::geometry::brep::{Body, Brep, Element, FaceId};
use kinogaki_studio_api::geometry::{Detail, Domain, Elements, Geometry, Selection, Tolerances};
use kinogaki_studio_api::standard::{self, BrepCodec};
use kinogaki_studio_api::{
    CancelToken, Data, DataCodec, Diagnostic, DisplayRequest, Error, Meshed, Mesher, MesherId,
    Package, PackageId, Registry,
};
use kinogaki_studio_api_testing::Fixtures;

fn fixture(name: &str) -> Brep {
    let all = Fixtures::default().breps();
    all.into_iter().find(|(n, _)| n == name).unwrap().1
}

fn save(data: &Data) -> MemoryElement {
    let mut saved = MemoryElement::new("result");
    saved.write(|w| BrepCodec.write(data, None, w)).unwrap();
    saved
}

#[test]
fn every_fixture_reads_back_equal_with_its_ids() {
    for (name, value) in Fixtures::default().breps() {
        let data = standard::brep().wrap(value.clone());
        let saved = save(&data);
        let read = saved.read(|r| BrepCodec.read(r, None)).unwrap();
        let back = standard::brep().get(&read).unwrap();
        assert_eq!(back, &value, "{name}");
        // Saved again, the same save: nothing is reordered on the way.
        assert_eq!(save(&read), saved, "{name}");
        for (a, b) in value.bodies().iter().zip(back.bodies().iter()) {
            assert_eq!(a.topology(), b.topology(), "{name}");
            assert_eq!(a.names().faces(), b.names().faces(), "{name}");
            assert_eq!(a.kind(), b.kind(), "{name}");
        }
    }
}

#[test]
fn a_value_with_every_part_keeps_each_part() {
    let value = fixture("every part");
    let data = standard::brep().wrap(value.clone());
    let read = save(&data).read(|r| BrepCodec.read(r, None)).unwrap();
    let back = standard::brep().get(&read).unwrap();
    assert_eq!(back.datums(), value.datums());
    assert_eq!(back.curves(), value.curves());
    assert_eq!(back.detail(), value.detail());
    assert_eq!(back.bodies()[0].instances(), value.bodies()[0].instances());
    assert_eq!(
        back.bodies()[0].attributes(),
        value.bodies()[0].attributes()
    );
}

#[test]
fn a_broken_save_does_not_read() {
    let data = standard::brep().wrap(fixture("every part"));
    let broken = |field: &str, value: Field| {
        let mut bad = MemoryElement::new("result");
        bad.write(|w| {
            BrepCodec.write(&data, None, w)?;
            w.set(field, value)
        })
        .unwrap();
        bad.read(|r| BrepCodec.read(r, None))
            .unwrap_err()
            .to_string()
    };
    assert!(broken("nameTags", Field::U8s(vec![9])).contains("no name kind 9"));
    assert!(broken("datumKinds", Field::U8s(vec![0, 1, 7])).contains("no datum kind 7"));
    // An axis whose direction isn't unit: the value's own rule refuses it.
    let reals = [
        [0.0; 12].as_slice(),
        &[0.0, 0.0, 0.0, 0.0, 0.0, 2.0],
        &[1.0, 2.0, 3.0],
    ];
    let mut plane = reals.concat();
    plane[3..12].copy_from_slice(&[1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]);
    assert!(broken("datumReals", Field::F64s(plane)).contains("datum"));
}

#[test]
fn counts_and_selections_span_the_bodies() {
    let value = fixture("every part");
    // The box's 6 faces, the patch's 1, then the plane; 8 + 4 vertices,
    // then the datum point; 12 + 4 edges, the free curve, then the axis.
    assert_eq!(value.count(Domain::Primitive), Some(8));
    assert_eq!(value.count(Domain::Point), Some(13));
    assert_eq!(value.count(Domain::Edge), Some(18));
    assert_eq!(value.count(Domain::Detail), Some(1));
    assert_eq!(value.count(Domain::Vertex), None);
    assert_eq!(Brep::empty().count(Domain::Primitive), Some(0));
    let body = |body, index| Element::Body { body, index };
    for (domain, id, element) in [
        (Domain::Primitive, 0, body(0, 0)),
        (Domain::Primitive, 6, body(1, 0)),
        (Domain::Primitive, 7, Element::Datum(0)),
        (Domain::Edge, 15, body(1, 3)),
        (Domain::Edge, 16, Element::Curve(0)),
        (Domain::Edge, 17, Element::Datum(1)),
        (Domain::Point, 12, Element::Datum(2)),
    ] {
        assert_eq!(value.element(domain, id), Some(element), "{domain:?} {id}");
        assert_eq!(value.id(domain, element), Some(id), "{element:?}");
    }
    assert_eq!(value.element(Domain::Primitive, 8), None);
    assert_eq!(value.id(Domain::Point, Element::Datum(0)), None);
    assert_eq!(value.id(Domain::Primitive, Element::Curve(0)), None);
    // The numbering's fingerprint is the value's.
    assert_eq!(value.clone().fingerprint(), value.fingerprint());
    let data = standard::brep().wrap(value);
    let members = Elements::from_indices(8, [6, 0]).unwrap();
    let s = Selection::new(Domain::Primitive, members, Some(6)).unwrap();
    let components = standard::components(data.ty(), s.clone());
    let mut saved = MemoryElement::new("components");
    saved
        .write(|w| {
            BrepCodec
                .write_components(&components, &data, w)
                .map(|_| ())
        })
        .unwrap();
    let back = saved.read(|r| BrepCodec.read_components(r, &data)).unwrap();
    assert_eq!(back.selection(), Some(&s));
}

/// A mesher that meshes nothing.
struct Nothing(&'static str);

impl Mesher for Nothing {
    fn id(&self) -> MesherId {
        MesherId::declared(self.0)
    }
    fn label(&self) -> &str {
        "Nothing"
    }
    fn mesh(
        &self,
        _: &Body,
        _: Detail,
        _: &Tolerances,
        cancel: &CancelToken,
    ) -> Result<Meshed, Error> {
        cancel.check()?;
        Ok(Meshed {
            polygons: Geometry::empty(),
            failed: vec![
                Diagnostic::warning("not meshed").element(
                    FaceId::from_index(&fixture("box").bodies()[0], 0)
                        .unwrap()
                        .index(),
                ),
            ],
        })
    }
}

#[test]
fn meshers_register_once_under_their_vendor_in_order() {
    let mut registry = Registry::new();
    let theirs = Package::new(PackageId::declared("acme.mesher"), "Acme")
        .with_mesher(Nothing("acme.mesher.fast"));
    registry.register(theirs).unwrap();
    let ours = Package::new(PackageId::declared("kinogaki.cadmesh"), "CAD")
        .with_mesher(Nothing("kinogaki.cadmesh.mesher"));
    registry.register(ours).unwrap();
    let order: Vec<_> = registry.meshers().map(|m| m.id().as_str()).collect();
    assert_eq!(order, ["acme.mesher.fast", "kinogaki.cadmesh.mesher"]);
    let again = Package::new(PackageId::declared("acme.again"), "Again")
        .with_mesher(Nothing("acme.mesher.fast"));
    assert!(registry.register(again).is_err());
    let foreign = Package::new(PackageId::declared("acme.other"), "Other")
        .with_mesher(Nothing("kinogaki.cadmesh.stolen"));
    assert!(registry.register(foreign).is_err());
    let value = fixture("box");
    let body = &value.bodies()[0];
    let meshed = registry.meshers().next().unwrap().mesh(
        body,
        Detail::default(),
        &Tolerances::default(),
        &CancelToken::default(),
    );
    assert_eq!(meshed.unwrap().failed[0].message, "not meshed");
}

#[test]
fn a_display_request_carries_the_default_tolerances() {
    assert_eq!(DisplayRequest::default().tolerances, Tolerances::default());
}
