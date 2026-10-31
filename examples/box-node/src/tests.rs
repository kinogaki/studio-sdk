//! Cooks the Box node the way Studio does, then asks the box data type
//! what Studio asks it: bounds, viewport surface, render triangles, save;
//! reads the handles the node type binds; and adds the node to a Boxes
//! network, as Tab does, on the SDK's fake host.
use super::*;
use kinogaki_studio_api::cook::{Constraint, Space};
use kinogaki_studio_api::element::MemoryElement;
use kinogaki_studio_api::network::{NetworkRead, NewNode};
use kinogaki_studio_api::parallel::Serial;
use studio_ui_api::host::{HostNetwork, HostUndo};
use studio_ui_api::testing::{FakeHost, FakeNetwork};

/// Cooks the Box node with `width`, `depth` and `height`, centered on the
/// origin.
fn cook(width: f64, depth: f64, height: f64) -> Result<Outputs, CookError> {
    cook_at(width, depth, height, [0.0; 3])
}

/// Cooks the Box node with `width`, `depth`, `height` and `center`.
fn cook_at(width: f64, depth: f64, height: f64, center: [f64; 3]) -> Result<Outputs, CookError> {
    let mut parameters = BoxNode.description().parameters.values();
    parameters.insert("size".into(), vec![width, depth, height].into());
    parameters.insert("center".into(), center.to_vec().into());
    let cancel = CancelToken::default();
    let mut scratch = None;
    let mut cx = CookCx::new(&parameters, &cancel, &Serial, &mut scratch);
    BoxNode.cook(&mut cx, &[])
}

#[test]
fn the_package_registers() {
    let mut registry = Registry::new();
    registry.register(package()).unwrap();
    let node = NodeTypeId::declared("example.boxes.box_node");
    assert!(registry.node_type(node).is_some());
    assert_eq!(
        registry.data_type_owner(BOX.id()).unwrap().as_str(),
        "example.boxes"
    );
    // Boxes objects hold Boxes networks and show the box they yield.
    let kind = registry.object_kind(boxes()).unwrap();
    assert_eq!(
        (kind.context(), kind.result()),
        (Some(network()), Some(BOX.id()))
    );
}

/// Where Studio offers the node: in Boxes networks, Tab's Create section.
#[test]
fn the_node_is_listed_in_its_context() {
    let description = BoxNode.description();
    assert_eq!(description.contexts, [network()]);
    let listing = description.listing.expect("listed in Tab");
    assert_eq!(
        (
            listing.section.as_str(),
            listing.category,
            listing.family.as_str()
        ),
        ("Create", Category::Create, "Primitives")
    );
}

/// The node in a Boxes object's network, added and edited the way Tab and
/// the inspector do (one undo step each), then cooked.
#[test]
fn the_node_cooks_in_a_network() {
    let package = package();
    let object = ObjectRef(1);
    let mut host = FakeHost::new(network(), []);
    host.networks
        .insert(object, FakeNetwork::new(network(), &[&package]).unwrap());
    let node_type = BoxNode.id();
    let mut added = None;
    host.edit_network(object, "Add Box", &mut |net| {
        let node = NewNode::new(node_type).displayed();
        added = Some(net.add_node(node, [0.0, 0.0])?);
        Ok(())
    })
    .unwrap();
    let node = added.unwrap();
    host.edit_network(object, "Size", &mut |net| {
        net.set_parameter(node, "size", vec![2.0, 1.0, 1.0].into())
    })
    .unwrap();
    // A flat box's size is refused, and changes nothing.
    let flat = host.edit_network(object, "Flat", &mut |net| {
        net.set_parameter(node, "size", vec![2.0, 0.0, 1.0].into())
    });
    assert!(flat.is_err());
    assert_eq!(host.history(), ["Add Box", "Size"]);
    let net = &host.networks[&object];
    assert_eq!(net.display(), Some(node));
    assert_eq!(net.node(node).unwrap().name, "Box 1");
    let data = net.cook(node).unwrap().data.remove(0);
    assert_eq!(BOX.get(&data).unwrap().size, [2.0, 1.0, 1.0]);
    host.undo();
    let net = &host.networks[&object];
    let data = net.cook(node).unwrap().data.remove(0);
    assert_eq!(BOX.get(&data).unwrap().size, [1.0; 3]);
}

#[test]
fn the_node_shows_by_the_package_icon() {
    let package = package();
    let [icon] = &package.icons[..] else {
        panic!("one icon")
    };
    assert!(icon.check().is_ok());
    let name = kinogaki_studio_api::icon::qualified(package.id, &icon.name);
    assert_eq!(BoxNode.description().icon, name);
}

#[test]
fn the_node_makes_a_box_that_renders_as_twelve_triangles() {
    let registry = {
        let mut registry = Registry::new();
        registry.register(package()).unwrap();
        registry
    };
    let data = cook(2.0, 3.0, 4.0).unwrap().data.remove(0);
    assert_eq!(
        BOX.get(&data),
        Some(&BoxShape {
            size: [2.0, 3.0, 4.0],
            center: [0.0; 3],
        })
    );
    let ty = registry.data_type(data.ty()).unwrap();

    let bounds = ty.bounds(&data).unwrap();
    assert_eq!(bounds.min, Vec3::new(-1.0, -1.5, -2.0));
    assert_eq!(bounds.max, Vec3::new(1.0, 1.5, 2.0));
    assert_eq!(ty.info(&data)[0].value, "2 × 3 × 4");

    let cancel = CancelToken::default();
    let items = ty
        .renderable()
        .unwrap()
        .items(&data, Time::default(), &cancel)
        .unwrap();
    let RenderItem::Triangles { source, .. } = &items[0];
    assert_eq!((items.len(), source.triangle_count()), (1, 12));
    // Every triangle faces away from the center.
    let points = source.points();
    for [a, b, c] in source
        .triangles()
        .iter()
        .map(|t| t.map(|i| points[i as usize]))
    {
        let (u, v) = (sub(b, a), sub(c, a));
        let normal = [
            u[1] * v[2] - u[2] * v[1],
            u[2] * v[0] - u[0] * v[2],
            u[0] * v[1] - u[1] * v[0],
        ];
        let center = [0, 1, 2].map(|k| (a[k] + b[k] + c[k]) / 3.0);
        assert!(dot(normal, center) > 0.0);
    }

    let display = ty.displayable().unwrap();
    let drawn = display
        .prepare(&data, None, &DisplayRequest::default(), &cancel)
        .unwrap();
    let surface = viewport::packet_as::<Surface>(&drawn.pieces[0].packet).unwrap();
    assert_eq!(surface.faces(), 6);
    assert_eq!(surface.triangles().len(), 12);
    assert_eq!(surface.lines().len(), 2 * 12); // twelve edges
}

#[test]
fn a_box_saves_and_reads_back() {
    let data = cook_at(1.0, 2.0, 0.5, [3.0, -1.0, 2.0])
        .unwrap()
        .data
        .remove(0);
    let codec = BoxType;
    let codec = codec.codec();
    let mut element = MemoryElement::new("box");
    element.write(|out| codec.write(&data, None, out)).unwrap();
    let read = element.read(|reader| codec.read(reader, None)).unwrap();
    assert_eq!(BOX.get(&read), BOX.get(&data));
    assert_eq!(codec_stamp(&read), codec_stamp(&data));
}

#[test]
fn a_moved_box_has_its_own_bounds_and_stamp() {
    let moved = cook_at(2.0, 2.0, 2.0, [1.0, 0.0, 0.0])
        .unwrap()
        .data
        .remove(0);
    let bounds = BoxType.bounds(&moved).unwrap();
    assert_eq!(bounds.min, Vec3::new(0.0, -1.0, -1.0));
    assert_eq!(bounds.max, Vec3::new(2.0, 1.0, 1.0));
    let still = cook(2.0, 2.0, 2.0).unwrap().data.remove(0);
    assert_ne!(codec_stamp(&moved), codec_stamp(&still));
}

/// What Studio reads to run the node's handle: a box handle whose center
/// is the `center` parameter and whose size is the `size` one, in the
/// object's space, free and always shown; and the values it reads from a
/// new node.
#[test]
fn the_node_binds_a_box_handle_to_its_center_and_sides() {
    let description = BoxNode.description();
    let [handle] = &description.handles[..] else {
        panic!("one handle: {:?}", description.handles);
    };
    let HandleKind::Box { center, size } = &handle.kind else {
        panic!("a box handle: {:?}", handle.kind);
    };
    assert_eq!(*center, Vector::parameter("center"));
    assert_eq!(*size, Vector::parameter("size"));
    assert_eq!(handle.label, "box");
    assert_eq!(
        (handle.space, handle.constraint, handle.range),
        (Space::Object, Constraint::Free, None)
    );
    // It binds only parameters the node declares, and shows for any.
    let values = description.parameters.values();
    for name in handle.parameters() {
        assert!(values.contains_key(name), "{name} is declared");
    }
    assert!(handle.shown(&values));
    assert_eq!(center.read(&values), Some([0.0; 3]));
    assert_eq!(size.read(&values), Some([1.0; 3]));
    // A drag's write is what the next cook reads.
    let mut moved = values;
    size.write(&mut moved, [2.0, 1.0, 1.0]);
    center.write(&mut moved, [0.5, 0.0, 0.0]);
    let cancel = CancelToken::default();
    let mut scratch = None;
    let mut cx = CookCx::new(&moved, &cancel, &Serial, &mut scratch);
    let data = BoxNode.cook(&mut cx, &[]).unwrap().data.remove(0);
    assert_eq!(
        BOX.get(&data),
        Some(&BoxShape {
            size: [2.0, 1.0, 1.0],
            center: [0.5, 0.0, 0.0],
        })
    );
}

#[test]
fn a_flat_box_is_an_error() {
    assert!(matches!(cook(1.0, 0.0, 1.0), Err(CookError::Failed(_))));
}

fn codec_stamp(data: &Data) -> Stamp {
    BoxType.stamp(data)
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

#[test]
fn the_entry_point_makes_the_package() {
    // What Studio's loader calls, through the entry's own type.
    let entry: kinogaki_studio_api::entry::Entry = kinogaki_package;
    let package = entry().expect("no panic");
    assert_eq!(package.id, PackageId::declared("example.boxes"));
    assert_eq!(package.node_types.len(), 1);
}
