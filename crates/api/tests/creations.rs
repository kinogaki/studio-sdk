//! Creations are named like every other id: well formed, under their
//! package's vendor and declared once, a kind's (those it makes with the
//! engines registered and the package's own), a package's creations of
//! another's kind and its node creations alike.
use kinogaki_studio_api::element::{ElementReader, ElementWriter};
use kinogaki_studio_api::menu::{Home, topics};
use kinogaki_studio_api::network::{NewNode, NodeCreation};
use kinogaki_studio_api::*;
use kinogaki_studio_api_testing::assert_refused;
use std::sync::Arc;

#[derive(Debug)]
struct Body;
impl ObjectBody for Body {
    fn kind(&self) -> ObjectKindId {
        ObjectKindId::declared("acme.toys.top")
    }
}

struct Unsaved;
impl ObjectCodec for Unsaved {
    fn write(&self, _: &dyn ObjectBody, _: &mut dyn ElementWriter) -> Result<(), Error> {
        Err(Error::invalid("not saved"))
    }
    fn read(&self, _: &dyn ElementReader, _: &ReadCx<'_>) -> Result<Arc<dyn ObjectBody>, Error> {
        Err(Error::invalid("not saved"))
    }
}

/// Kind `id`, made as `made` says, and as `engine_made` too while an
/// engine is installed.
struct Kind {
    id: &'static str,
    made: &'static [&'static str],
    engine_made: &'static str,
}

impl ObjectKindSpec for Kind {
    fn id(&self) -> ObjectKindId {
        ObjectKindId::declared(self.id)
    }
    fn label(&self) -> &str {
        self.id
    }
    fn icon(&self) -> &str {
        "cube"
    }
    fn placed(&self) -> bool {
        true
    }
    fn context(&self) -> Option<ContextId> {
        None
    }
    fn result(&self) -> Option<DataTypeId> {
        None
    }
    fn new_body(&self) -> Arc<dyn ObjectBody> {
        Arc::new(Body)
    }
    fn codec(&self) -> &dyn ObjectCodec {
        &Unsaved
    }
    fn creations(&self, cx: &CreateCx<'_>) -> Vec<KindCreation> {
        let engine = cx.engines().iter().next().map(|_| self.engine_made);
        (self.made.iter().copied().chain(engine))
            .map(|id| KindCreation::new(CreationId::declared(id), id, utility()))
            .collect()
    }
}

fn kind(id: &'static str, made: &'static [&'static str]) -> Kind {
    Kind {
        id,
        made,
        engine_made: "other.lights.lamp",
    }
}

fn utility() -> Home {
    Home::topic(topics::objects::UTILITY)
}

fn package(id: &'static str) -> Package {
    Package::new(PackageId::declared(id), id)
}

fn node(id: &'static str) -> NodeCreation {
    NodeCreation::new(
        CreationId::declared(id),
        id,
        Home::family(),
        NewNode::new(NodeTypeId::declared("other.trees.grow")),
    )
}

#[test]
fn a_kinds_creations_are_its_vendors_and_new() {
    let mut registry = Registry::new();
    let toys = package("acme.toys").with_object_kind(kind("acme.toys.top", &["acme.toys.top"]));
    registry.register(toys).unwrap();
    let again = package("acme.more").with_object_kind(kind("acme.more.top", &["acme.toys.top"]));
    assert_refused(
        registry.register(again),
        &[", which ", "object creation", "acme.toys.top"],
    );
    let foreign = package("acme.odd").with_object_kind(kind("acme.odd.top", &["other.toys.top"]));
    assert_refused(registry.register(foreign), &["under another vendor's name"]);
    // A creation of another package's kind: of a declared kind, under the
    // declaring package's vendor, new.
    let made = |id| KindCreation::new(CreationId::declared(id), "Top", utility());
    let top = ObjectKindId::declared("acme.toys.top");
    let own = package("acme.spin").with_object_creation(top, made("acme.spin.top"));
    registry.register(own).unwrap();
    let nowhere = ObjectKindId::declared("acme.none.top");
    let lost = package("acme.lost").with_object_creation(nowhere, made("acme.lost.top"));
    assert_refused(
        registry.register(lost),
        &["of kind acme.none.top, which no package declares"],
    );
    let again = package("acme.spun").with_object_creation(top, made("acme.spin.top"));
    assert_refused(
        registry.register(again),
        &["acme.spin.top", "acme.spin declared"],
    );
}

#[test]
fn node_creations_are_their_vendors_and_new() {
    let mut registry = Registry::new();
    registry
        .register(package("acme.forest").with_creation(node("acme.forest.oak")))
        .unwrap();
    let again = package("acme.more").with_creation(node("acme.forest.oak"));
    assert_refused(registry.register(again), &[", which ", "node creation"]);
    let twice = package("acme.twice")
        .with_creation(node("acme.twice.oak"))
        .with_creation(node("acme.twice.oak"));
    assert_refused(registry.register(twice), &[", which "]);
    let foreign = package("acme.odd").with_creation(node("other.forest.oak"));
    assert_refused(registry.register(foreign), &["under another vendor's name"]);
    // A malformed creation id can't be made: `CreationId` checks it.
    assert!(CreationId::parse("acme.bad").is_none());
    // A node creation and an object creation may share a name: each kind
    // of id is its own.
    let toys = package("acme.toys").with_object_kind(kind("acme.toys.top", &["acme.toys.oak"]));
    registry
        .register(toys.with_creation(node("acme.toys.oak")))
        .unwrap();
    assert_eq!(registry.creations().count(), 2);
}
