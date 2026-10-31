//! A Kinogaki Studio plugin that adds a Box node.
//!
//! The package defines:
//!
//! - a data type, `example.boxes.box` (a box's size and center), which
//!   tells Studio how a box shows in the viewport, how it renders and how
//!   it saves;
//! - a network context, `example.boxes.network`, and an object kind,
//!   Boxes (`example.boxes.boxes`), whose objects hold a network of that
//!   context and show the box it yields;
//! - a node type, `example.boxes.box_node`, offered in that context and
//!   listed in its Tab menu, that makes a box from its size and center. It
//!   binds a box handle to its parameters, so Studio draws the box in the
//!   viewport and drags its faces and center with no code here;
//! - the kind's creations: an empty Boxes object, and a Box (a Boxes
//!   object already holding a Box node), in Create, Tab and the A menu;
//! - one icon added to Studio's set, `example.boxes/box`, which the node
//!   and the object kind show by.
use kinogaki_studio_api::cook::{Handle, HandleKind, Vector};
use kinogaki_studio_api::element::Field;
use kinogaki_studio_api::network::FlagSet;
use kinogaki_studio_api::params::{Number, Parameter, ParameterKind};
use kinogaki_studio_api::*;
use std::sync::{Arc, LazyLock};

/// A box: its size along x, y and z and its middle.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoxShape {
    /// Its sides along x, y and z.
    pub size: [f64; 3],
    /// The middle.
    pub center: [f64; 3],
}

impl BoxShape {
    /// Its smallest and largest corners.
    pub fn corners(&self) -> ([f64; 3], [f64; 3]) {
        let half = self.size.map(|s| s / 2.0);
        (
            [0, 1, 2].map(|k| self.center[k] - half[k]),
            [0, 1, 2].map(|k| self.center[k] + half[k]),
        )
    }

    /// Its eight corners: the bottom four, then the top four.
    pub fn points(&self) -> [[f32; 3]; 8] {
        let (low, high) = self.corners();
        let [a, b, c] = low.map(|v| v as f32);
        let [x, y, z] = high.map(|v| v as f32);
        [
            [a, b, c],
            [x, b, c],
            [x, y, c],
            [a, y, c],
            [a, b, z],
            [x, b, z],
            [x, y, z],
            [a, y, z],
        ]
    }
}

/// The six faces, as corner numbers counterclockwise seen from outside:
/// bottom, top, front, back, right, left.
const FACES: [[u32; 4]; 6] = [
    [0, 3, 2, 1],
    [4, 5, 6, 7],
    [0, 1, 5, 4],
    [2, 3, 7, 6],
    [1, 2, 6, 5],
    [3, 0, 4, 7],
];

/// The connectivity key: every box has the same faces.
const TOPOLOGY: u64 = 1;

/// Wraps boxes as `Data` and reads them back.
pub static BOX: LazyLock<Typed<BoxShape>> =
    LazyLock::new(|| Typed::new(DataTypeId::declared("example.boxes.box")));

fn box_of(data: &Data) -> Result<&BoxShape, Error> {
    BOX.get(data).ok_or_else(|| Error::invalid("not a box"))
}

/// The box data type, with its display, render and save facets.
pub struct BoxType;

impl DataType for BoxType {
    fn id(&self) -> DataTypeId {
        BOX.id()
    }
    fn label(&self) -> &str {
        "Box"
    }
    fn wire_color(&self) -> Color {
        Color::rgb(232, 164, 58)
    }
    fn stamp(&self, data: &Data) -> Stamp {
        // The sides, then a hash of the middle (FNV-1a).
        let mut stamp = [0; 32];
        if let Some(shape) = BOX.get(data) {
            for (bytes, side) in stamp.chunks_mut(8).zip(shape.size) {
                bytes.copy_from_slice(&side.to_le_bytes());
            }
            let middle = shape.center.iter().flat_map(|v| v.to_le_bytes());
            let hash = middle.fold(0xcbf2_9ce4_8422_2325_u64, |h, byte| {
                (h ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
            });
            stamp[24..].copy_from_slice(&hash.to_le_bytes());
        }
        stamp
    }
    fn memory(&self, _data: &Data) -> usize {
        size_of::<BoxShape>()
    }
    fn bounds(&self, data: &Data) -> Option<Bounds> {
        let ([a, b, c], [x, y, z]) = BOX.get(data)?.corners();
        Some(Bounds {
            min: Vec3::new(a, b, c),
            max: Vec3::new(x, y, z),
        })
    }
    fn info(&self, data: &Data) -> Vec<InfoRow> {
        let Some(BoxShape {
            size: [w, d, h], ..
        }) = BOX.get(data)
        else {
            return Vec::new();
        };
        vec![InfoRow::new("Size", format!("{w} × {d} × {h}"))]
    }
    fn codec(&self) -> &dyn DataCodec {
        self
    }
    fn displayable(&self) -> Option<&dyn Displayable> {
        Some(self)
    }
    fn renderable(&self) -> Option<&dyn Renderable> {
        Some(self)
    }
}

/// In the viewport: six flat faces and their twelve edges.
impl Displayable for BoxType {
    fn levels(&self) -> &[ElementKind] {
        &[] // selected as a whole object only
    }
    fn prepare(
        &self,
        data: &Data,
        _: Option<&Drawn>,
        _: &DisplayRequest,
        _: &CancelToken,
    ) -> Result<Drawn, Error> {
        let surface = Surface::polygons(box_of(data)?.points(), FACES, TOPOLOGY);
        Ok(Drawn::one(Arc::new(surface)))
    }
    fn highlight(&self, _: &Data, _: &Drawn, _: &Components) -> ElementSet {
        ElementSet::new(ElementKind::Face, false, [])
    }
    fn convert(&self, _: &Data, _: &Components, _: ElementKind) -> Option<Components> {
        None
    }
}

/// In renders: twelve triangles, two a face.
impl Renderable for BoxType {
    fn items(&self, data: &Data, _: Time, _: &CancelToken) -> Result<Vec<RenderItem>, Error> {
        let triangles: Vec<[u32; 3]> = FACES
            .iter()
            .flat_map(|&[a, b, c, d]| [[a, b, c], [a, c, d]])
            .collect();
        let source = PlainTriangles::new(box_of(data)?.points(), triangles, TOPOLOGY);
        Ok(vec![RenderItem::triangles(0, Arc::new(source))])
    }
}

/// In project files: the size and the middle.
impl DataCodec for BoxType {
    fn write(
        &self,
        data: &Data,
        _: Option<&Data>,
        out: &mut dyn ElementWriter,
    ) -> Result<(), Error> {
        let shape = box_of(data)?;
        out.set("size", Field::F64s(shape.size.to_vec()))?;
        out.set("center", Field::F64s(shape.center.to_vec()))
    }
    fn read(&self, element: &dyn ElementReader, _: Option<&Data>) -> Result<Data, Error> {
        let three = |name: &str| -> Result<[f64; 3], Error> {
            element
                .f64s(name)?
                .try_into()
                .map_err(|_| Error::invalid(format!("a box's {name} has three numbers")))
        };
        Ok(BOX.wrap(BoxShape {
            size: three("size")?,
            center: three("center")?,
        }))
    }
}

/// The package's icon, embedded: 11 × 11 icon pixels, white, as Studio's
/// own icons are drawn (Studio tints it with the widget showing it).
const BOX_ICON: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 11 11" width="11" height="11" shape-rendering="crispEdges" fill="#fff"><rect x="2" y="3" width="7" height="2"/><rect x="2" y="5" width="2" height="2"/><rect x="7" y="5" width="2" height="2"/><rect x="2" y="7" width="7" height="1"/></svg>"##;

/// The context of the networks inside Boxes objects.
pub fn network() -> ContextId {
    ContextId::declared("example.boxes.network")
}

/// The Boxes object kind.
pub fn boxes() -> ObjectKindId {
    ObjectKindId::declared("example.boxes.boxes")
}

/// The Box node: makes a box from its parameters.
pub struct BoxNode;

impl NodeType for BoxNode {
    fn id(&self) -> NodeTypeId {
        NodeTypeId::declared(BOX_NODE)
    }
    /// No inputs, one box out, its size and center in scene units, and a
    /// box handle on them: Studio draws the box with a dot on each face
    /// and one in the middle, and dragging them writes the size and the
    /// center. It is offered in Boxes networks, listed in Tab under Create.
    fn description(&self) -> Description {
        let vector = |number: Number| ParameterKind::Vector {
            components: 3,
            number,
        };
        let sides = vector(Number::length().range(0.001, f64::INFINITY));
        Description::new("Box")
            .icon("example.boxes/box")
            .context(network())
            .listed("Create", Category::Create, "Primitives", &["cube"])
            .output(Port::new("box", "Box", BOX.id()))
            .parameters(
                Schema::default()
                    .parameter(Parameter::new("size", "Size", sides, vec![1.0; 3]))
                    .parameter(Parameter::new(
                        "center",
                        "Center",
                        vector(Number::length()),
                        vec![0.0; 3],
                    )),
            )
            .handle(Handle::new(
                "box",
                HandleKind::Box {
                    center: Vector::parameter("center"),
                    size: Vector::parameter("size"),
                },
            ))
    }
    fn cook(&self, cx: &mut CookCx<'_>, _inputs: &[Data]) -> Result<Outputs, CookError> {
        let three = |name: &str| {
            Vector::parameter(name)
                .read(cx.parameters())
                .ok_or_else(|| CookError::message(format!("the {name} is three numbers")))
        };
        let (size, center) = (three("size")?, three("center")?);
        if size.iter().any(|&side| side <= 0.0) {
            return Err(CookError::message("every side must be longer than zero"));
        }
        Ok(Outputs::one(BOX.wrap(BoxShape { size, center })))
    }
}

/// A Boxes object's body: it holds nothing of its own (its network, which
/// Studio keeps, makes its box).
#[derive(Debug)]
pub struct BoxesBody;

impl ObjectBody for BoxesBody {
    fn kind(&self) -> ObjectKindId {
        boxes()
    }
}

/// The Box node's id.
const BOX_NODE: &str = "example.boxes.box_node";

/// Boxes objects: placed in the scene, each holding a Boxes network whose
/// box it shows.
struct BoxesKind;

impl ObjectKindSpec for BoxesKind {
    fn id(&self) -> ObjectKindId {
        boxes()
    }
    fn label(&self) -> &str {
        "Boxes"
    }
    fn icon(&self) -> &str {
        "example.boxes/box"
    }
    fn placed(&self) -> bool {
        true
    }
    fn context(&self) -> Option<ContextId> {
        Some(network())
    }
    fn result(&self) -> Option<DataTypeId> {
        Some(BOX.id())
    }
    fn new_body(&self) -> Arc<dyn ObjectBody> {
        Arc::new(BoxesBody)
    }
    fn codec(&self) -> &dyn ObjectCodec {
        self
    }
    fn adopts(&self, _data: &Data) -> Option<Arc<dyn ObjectBody>> {
        None
    }
    /// In Create, Tab and the A menu: an empty Boxes object, which opens
    /// for its first node, and a Box: a Boxes object already holding a Box
    /// node.
    fn creations(&self, _cx: &CreateCx<'_>) -> Vec<KindCreation> {
        let mut empty = KindCreation::new(boxes().as_str(), "Boxes");
        empty.opens = true;
        let mut one = KindCreation::new(BOX_NODE, "Box");
        one.family = "Boxes".into();
        one.aliases = vec!["cube".into()];
        one.start = Some(NodeTypeId::declared(BOX_NODE));
        vec![empty, one]
    }
    fn create(&self, id: &str, _cx: &CreateCx<'_>) -> Result<Arc<dyn ObjectBody>, String> {
        match id {
            _ if id == boxes().as_str() || id == BOX_NODE => Ok(self.new_body()),
            _ => Err(format!("Boxes don't make {id}")),
        }
    }
}

/// A Boxes body saves nothing but its kind.
impl ObjectCodec for BoxesKind {
    fn write(&self, _body: &dyn ObjectBody, _out: &mut dyn ElementWriter) -> Result<(), Error> {
        Ok(())
    }
    fn read(
        &self,
        _element: &dyn ElementReader,
        _cx: &ReadCx<'_>,
    ) -> Result<Arc<dyn ObjectBody>, Error> {
        Ok(Arc::new(BoxesBody))
    }
}

/// The package: what Studio registers when it loads the plugin.
pub fn package() -> Package {
    let context = NetworkContext::new(network(), "Boxes", "boxes").flags(FlagSet::all());
    Package::new(PackageId::declared("example.boxes"), "Boxes")
        .with_data_type(BoxType)
        .with_context(context)
        .with_object_kind(BoxesKind)
        .with_node_type(BoxNode)
        .with_icon(Icon::new("box", BOX_ICON))
}

// The entry point Studio looks up in the plugin's library.
export_package!(package);

#[cfg(test)]
mod tests;
