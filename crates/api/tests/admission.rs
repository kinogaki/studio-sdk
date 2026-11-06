//! Registry admission refuses a package naming what isn't there: a port's
//! data type, a context a node type is offered in or an object kind holds,
//! a node type adopted twice, a malformed or
//! doubled icon, and a package icon no package carries.
use kinogaki_studio_api::cook::{Description, Port};
use kinogaki_studio_api::element::{ElementReader, ElementWriter};
use kinogaki_studio_api::icon::Icon;
use kinogaki_studio_api::network::NetworkContext;
use kinogaki_studio_api::*;
use std::sync::Arc;

const TREES: &str = "acme.trees.network";
const WOOD: &str = "acme.trees.wood";
const SQUARE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 11 11" fill="#fff"><rect x="3" y="3" width="5" height="5"/></svg>"##;

fn trees() -> PackageId {
    PackageId::declared("acme.trees")
}

fn context(id: &'static str) -> NetworkContext {
    NetworkContext::new(ContextId::declared(id), "Trees", "trees")
}

/// A node type offered in `offered`, its one output of `data`, shown by
/// icon `icon`.
struct Grow {
    offered: &'static str,
    data: &'static str,
    icon: &'static str,
}

impl NodeType for Grow {
    fn id(&self) -> NodeTypeId {
        NodeTypeId::declared("acme.trees.grow")
    }
    fn description(&self) -> Description {
        Description::new("Grow")
            .icon(self.icon)
            .context(ContextId::declared(self.offered))
            .output(Port::new("out", "Out", DataTypeId::declared(self.data)))
    }
    fn cook(&self, _: &mut CookCx<'_>, _: &[Data]) -> Result<Outputs, Error> {
        Ok(Outputs::default())
    }
}

fn grow(offered: &'static str, data: &'static str) -> Grow {
    Grow {
        offered,
        data,
        icon: "",
    }
}

/// A data type the toy packages register: no facets, never saved.
struct Wood;
struct Unsaved;
impl DataCodec for Unsaved {
    fn write(&self, _: &Data, _: Option<&Data>, _: &mut dyn ElementWriter) -> Result<(), Error> {
        Err(Error::invalid("not saved"))
    }
    fn read(&self, _: &dyn ElementReader, _: Option<&Data>) -> Result<Data, Error> {
        Err(Error::invalid("not saved"))
    }
}
impl DataType for Wood {
    fn id(&self) -> DataTypeId {
        DataTypeId::declared(WOOD)
    }
    fn label(&self) -> &str {
        "Wood"
    }
    fn wire_color(&self) -> WireColor {
        WireColor::rgb(0, 0, 0)
    }
    fn stamp(&self, _: &Data) -> Stamp {
        [0; 32]
    }
    fn memory(&self, _: &Data) -> usize {
        0
    }
    fn bounds(&self, _: &Data) -> Option<Bounds> {
        None
    }
    fn info(&self, _: &Data) -> Vec<InfoRow> {
        Vec::new()
    }
    fn codec(&self) -> &dyn DataCodec {
        &Unsaved
    }
}

/// An object kind whose objects hold networks of `holds`.
struct Grove {
    holds: &'static str,
    icon: &'static str,
}

#[derive(Debug)]
struct Body;
impl ObjectBody for Body {
    fn kind(&self) -> ObjectKindId {
        ObjectKindId::declared("acme.trees.grove")
    }
}
struct Bodies;
impl ObjectCodec for Bodies {
    fn write(&self, _: &dyn ObjectBody, _: &mut dyn ElementWriter) -> Result<(), Error> {
        Err(Error::invalid("not saved"))
    }
    fn read(&self, _: &dyn ElementReader, _: &ReadCx<'_>) -> Result<Arc<dyn ObjectBody>, Error> {
        Err(Error::invalid("not saved"))
    }
}
impl ObjectKindSpec for Grove {
    fn id(&self) -> ObjectKindId {
        ObjectKindId::declared("acme.trees.grove")
    }
    fn label(&self) -> &str {
        "Grove"
    }
    fn icon(&self) -> &str {
        self.icon
    }
    fn placed(&self) -> bool {
        true
    }
    fn context(&self) -> Option<ContextId> {
        Some(ContextId::declared(self.holds))
    }
    fn result(&self) -> Option<DataTypeId> {
        None
    }
    fn new_body(&self) -> Arc<dyn ObjectBody> {
        Arc::new(Body)
    }
    fn codec(&self) -> &dyn ObjectCodec {
        &Bodies
    }
    fn creations(&self, _: &CreateCx<'_>) -> Vec<KindCreation> {
        Vec::new()
    }
}

/// The trees package: its context and data type, plus `more`.
fn package(more: impl FnOnce(Package) -> Package) -> Package {
    more(
        Package::new(trees(), "Trees")
            .with_context(context(TREES))
            .with_data_type(Wood),
    )
}

#[test]
fn a_well_formed_package_registers() {
    let whole = package(|p| {
        p.with_node_type(Grow {
            icon: "acme.trees/leaf",
            ..grow(TREES, WOOD)
        })
        .with_object_kind(Grove {
            holds: TREES,
            icon: "cube",
        })
        .with_icon(Icon::new("leaf", SQUARE))
    });
    let mut registry = Registry::new();
    registry.register(whole).unwrap();
    // A later package shows the trees' icon, holds their context and
    // makes their data type.
    let moss =
        Package::new(PackageId::declared("acme.moss"), "Moss").with_object_kind(MossKind(Grove {
            holds: TREES,
            icon: "acme.trees/leaf",
        }));
    registry.register(moss).unwrap();
}

/// The grove kind under the moss package's id.
struct MossKind(Grove);
impl ObjectKindSpec for MossKind {
    fn id(&self) -> ObjectKindId {
        ObjectKindId::declared("acme.moss.patch")
    }
    fn label(&self) -> &str {
        "Patch"
    }
    fn icon(&self) -> &str {
        self.0.icon()
    }
    fn placed(&self) -> bool {
        true
    }
    fn context(&self) -> Option<ContextId> {
        self.0.context()
    }
    fn result(&self) -> Option<DataTypeId> {
        Some(DataTypeId::declared(WOOD))
    }
    fn new_body(&self) -> Arc<dyn ObjectBody> {
        Arc::new(Body)
    }
    fn codec(&self) -> &dyn ObjectCodec {
        &Bodies
    }
    fn creations(&self, _: &CreateCx<'_>) -> Vec<KindCreation> {
        Vec::new()
    }
}

#[test]
fn a_ports_data_type_is_registered() {
    let stray = package(|p| p.with_node_type(grow(TREES, "acme.trees.bark")));
    assert_eq!(
        Registry::new().register(stray),
        Err(Error::refused(format_args!(
            "{package}'s {by} names data type {data}, which isn't registered",
            package = trees(),
            by = "acme.trees.grow",
            data = DataTypeId::declared("acme.trees.bark")
        )))
    );
}

#[test]
fn a_node_type_is_offered_in_declared_contexts() {
    let stray = package(|p| p.with_node_type(grow("acme.trees.orchard", WOOD)));
    assert_eq!(
        Registry::new().register(stray),
        Err(Error::refused(format_args!(
            "{package}'s {by} names context {context}, which no package declares",
            package = trees(),
            by = "acme.trees.grow",
            context = ContextId::declared("acme.trees.orchard")
        )))
    );
}

#[test]
fn an_object_kind_holds_a_declared_context() {
    let stray = package(|p| {
        p.with_object_kind(Grove {
            holds: "acme.trees.orchard",
            icon: "cube",
        })
    });
    assert_eq!(
        Registry::new().register(stray),
        Err(Error::refused(format_args!(
            "{package}'s {by} names context {context}, which no package declares",
            package = trees(),
            by = "acme.trees.grove",
            context = ContextId::declared("acme.trees.orchard")
        )))
    );
}

#[test]
fn a_context_adopts_a_node_type_once() {
    let import = NodeTypeId::declared("acme.mesh.import");
    let twice = Package::new(trees(), "Trees").with_context(
        context(TREES)
            .adopts(import, "Model")
            .adopts(import, "Model"),
    );
    let refused = Err(Error::refused(format_args!(
        "{package}'s {context} adopts {node} twice",
        package = trees(),
        context = ContextId::declared(TREES),
        node = import
    )));
    assert_eq!(Registry::new().register(twice), refused);
    let once = Package::new(trees(), "Trees").with_context(context(TREES).adopts(import, "Model"));
    assert!(Registry::new().register(once).is_ok());
}

#[test]
fn icons_are_well_formed_named_once_and_found() {
    let bad = package(|p| p.with_icon(Icon::new("Leaf", SQUARE)));
    assert_eq!(
        Registry::new().register(bad),
        Err(Error::refused(format_args!(
            "{}'s icon: \"Leaf\" is not an icon name: a lowercase letter, then lowercase \
             letters, digits and `-`, at most 64 bytes",
            trees()
        )))
    );
    let twice = package(|p| {
        p.with_icon(Icon::new("leaf", SQUARE))
            .with_icon(Icon::new("leaf", SQUARE))
    });
    assert_eq!(
        Registry::new().register(twice),
        Err(Error::refused(format_args!(
            "{}'s icon: a second icon is named \"leaf\"",
            trees()
        )))
    );
    for missing in ["acme.trees/bark", "acme.moss/leaf"] {
        let shown = package(|p| {
            p.with_icon(Icon::new("leaf", SQUARE)).with_node_type(Grow {
                icon: missing,
                ..grow(TREES, WOOD)
            })
        });
        assert_eq!(
            Registry::new().register(shown),
            Err(Error::refused(format_args!(
                "{package}'s {by} shows icon {icon}, which no package carries",
                package = trees(),
                by = "acme.trees.grow",
                icon = missing
            )))
        );
    }
}
