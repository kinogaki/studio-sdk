//! Cooks the Box node the way Studio does, then asks the box data type
//! what Studio asks it: bounds, viewport surface, render triangles, save.
use super::*;
use kinogaki_studio_api::element::MemoryElement;
use kinogaki_studio_api::parallel::Serial;

/// Cooks the Box node with `width`, `depth` and `height`.
fn cook(width: f64, depth: f64, height: f64) -> Result<Outputs, CookError> {
    let mut parameters = BoxNode::parameters().values();
    parameters.insert("width".into(), width.into());
    parameters.insert("depth".into(), depth.into());
    parameters.insert("height".into(), height.into());
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
            size: [2.0, 3.0, 4.0]
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
        .prepare(&data, DisplayQuality::Normal, &cancel)
        .unwrap();
    assert_eq!(drawn.surface.faces(), 6);
    assert_eq!(drawn.surface.triangles().len(), 12);
    assert_eq!(drawn.surface.lines().len(), 2 * 12); // twelve edges
}

#[test]
fn a_box_saves_and_reads_back() {
    let data = cook(1.0, 2.0, 0.5).unwrap().data.remove(0);
    let codec = BoxType;
    let codec = codec.codec();
    let mut element = MemoryElement::new("box");
    element.write(|out| codec.write(&data, None, out)).unwrap();
    let read = element.read(|reader| codec.read(reader, None)).unwrap();
    assert_eq!(BOX.get(&read), BOX.get(&data));
    assert_eq!(codec_stamp(&read), codec_stamp(&data));
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
