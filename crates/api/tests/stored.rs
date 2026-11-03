//! Stored values: registered by type under qualified ids (one package's
//! never clobbers another's), kept in a table at their defaults, shared
//! and copied on write, and written and read through the element traits.
use kinogaki_studio_api::element::MemoryElement;
use kinogaki_studio_api::{
    ElementReader, ElementWriter, Error, Field, Package, PackageId, Registry, RegistryError, Scope,
    Stored, StoredId, StoredValue, StoredValues,
};

/// A toy project value: how dense a forest is.
#[derive(Clone, Debug, PartialEq)]
struct Density(f64);

impl Stored for Density {
    fn id() -> StoredId {
        StoredId::declared("acme.trees.forest")
    }
    fn scope() -> Scope {
        Scope::Project
    }
    fn defaults() -> Self {
        Density(0.5)
    }
    fn write(&self, to: &mut dyn ElementWriter) -> Result<(), Error> {
        to.set("density", Field::Float(self.0))
    }
    fn read(from: &dyn ElementReader) -> Result<Self, Error> {
        Ok(Density(from.float("density")?))
    }
}

/// Another package's value of another type under the same id.
#[derive(Clone, Debug, PartialEq)]
struct Forest(u8);

impl Stored for Forest {
    fn id() -> StoredId {
        StoredId::declared("acme.trees.forest")
    }
    fn scope() -> Scope {
        Scope::User
    }
    fn defaults() -> Self {
        Forest(1)
    }
    fn write(&self, to: &mut dyn ElementWriter) -> Result<(), Error> {
        to.set("count", Field::Int(self.0.into()))
    }
    fn read(_: &dyn ElementReader) -> Result<Self, Error> {
        Ok(Forest(1))
    }
}

#[test]
fn a_package_registers_its_stored_values() {
    let package = Package::new(PackageId::declared("acme.trees"), "Trees").with_stored::<Density>();
    assert_eq!(package.stored.len(), 1);
    assert_eq!(package.stored[0].id().as_str(), "acme.trees.forest");
    assert_eq!(package.stored[0].scope(), Scope::Project);
    assert!(Registry::new().register(package).is_ok());
}

#[test]
fn a_stored_id_is_declared_once_among_the_installed_packages() {
    let mut registry = Registry::new();
    let trees = PackageId::declared("acme.trees");
    registry
        .register(Package::new(trees, "Trees").with_stored::<Density>())
        .unwrap();
    let woods = PackageId::declared("acme.woods");
    assert_eq!(
        registry.register(Package::new(woods, "Woods").with_stored::<Forest>()),
        Err(RegistryError::DuplicateId {
            package: woods,
            what: "stored value",
            id: "acme.trees.forest",
            first: trees,
        })
    );
    let twice = Package::new(woods, "Woods")
        .with_stored::<Forest>()
        .with_stored::<Forest>();
    assert!(matches!(
        Registry::new().register(twice),
        Err(RegistryError::DuplicateId { first, .. }) if first == woods
    ));
    let foreign =
        Package::new(PackageId::declared("other.trees"), "Trees").with_stored::<Density>();
    assert!(matches!(
        Registry::new().register(foreign),
        Err(RegistryError::ForeignId {
            id: "acme.trees.forest",
            ..
        })
    ));
}

#[test]
fn a_table_refuses_two_values_of_one_id() {
    let twice = StoredValues::new([StoredValue::of::<Density>(), StoredValue::of::<Forest>()]);
    assert!(twice.is_err(), "the second would replace the first");
}

#[test]
fn a_table_holds_defaults_and_copies_shared_values_on_write() {
    let mut table = StoredValues::new([StoredValue::of::<Density>()]).unwrap();
    assert_eq!(table.get::<Density>(), Some(&Density(0.5)));
    let snapshot = table.clone();
    table.get_mut::<Density>().unwrap().0 = 0.9;
    assert_eq!(snapshot.get::<Density>(), Some(&Density(0.5)));
    assert_ne!(table, snapshot);
    assert!(table.set(Density(0.5)));
    assert_eq!(table, snapshot);
    assert!(!table.set_any(Density::id(), std::sync::Arc::new(3_u8)));
}

#[test]
fn a_value_writes_and_reads_through_its_registration() {
    let stored = StoredValue::of::<Density>();
    let mut element = MemoryElement::new("forest");
    element.write(|w| stored.write(&Density(0.25), w)).unwrap();
    let read = element.read(|r| stored.read(r)).unwrap();
    assert_eq!(read.downcast_ref::<Density>(), Some(&Density(0.25)));
}
