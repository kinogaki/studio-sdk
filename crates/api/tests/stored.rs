//! Stored values: registered by type, kept in a table at their defaults,
//! shared and copied on write, and written and read through the element
//! traits.
use kinogaki_studio_api::element::MemoryElement;
use kinogaki_studio_api::{
    ElementReader, ElementWriter, Error, Field, Package, PackageId, Scope, Stored, StoredValue,
    StoredValues,
};

/// A toy project value: how dense a forest is.
#[derive(Clone, Debug, PartialEq)]
struct Density(f64);

impl Stored for Density {
    fn id() -> &'static str {
        "forest"
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

#[test]
fn a_package_registers_its_stored_values() {
    let package = Package::new(PackageId::declared("acme.trees"), "Trees").with_stored::<Density>();
    assert_eq!(package.stored.len(), 1);
    assert_eq!(package.stored[0].id(), "forest");
    assert_eq!(package.stored[0].scope(), Scope::Project);
}

#[test]
fn a_table_holds_defaults_and_copies_shared_values_on_write() {
    let mut table = StoredValues::new([StoredValue::of::<Density>()]);
    assert_eq!(table.get::<Density>(), Some(&Density(0.5)));
    let snapshot = table.clone();
    table.get_mut::<Density>().unwrap().0 = 0.9;
    assert_eq!(snapshot.get::<Density>(), Some(&Density(0.5)));
    assert_ne!(table, snapshot);
    assert!(table.set(Density(0.5)));
    assert_eq!(table, snapshot);
    assert!(!table.set_any("forest", std::sync::Arc::new(3_u8)));
}

#[test]
fn a_value_writes_and_reads_through_its_registration() {
    let stored = StoredValue::of::<Density>();
    let mut element = MemoryElement::new("forest");
    element.write(|w| stored.write(&Density(0.25), w)).unwrap();
    let read = element.read(|r| stored.read(r)).unwrap();
    assert_eq!(read.downcast_ref::<Density>(), Some(&Density(0.25)));
}
