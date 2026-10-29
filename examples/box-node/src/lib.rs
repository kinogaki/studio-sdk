//! A Kinogaki Studio plugin that adds a Box node.
//!
//! The package defines a data type, `example.boxes.box` (a box's size),
//! and a node type, `example.boxes.box_node`, that makes a box from its
//! width, depth and height. The data type tells Studio how a box shows in
//! the viewport, how it renders and how it saves.
use kinogaki_studio_api::element::Field;
use kinogaki_studio_api::params::{Number, Parameter, ParameterKind};
use kinogaki_studio_api::*;
use std::sync::{Arc, LazyLock};

/// A box centered on the origin: its size along x, y and z (z is up).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoxShape {
    /// Width, depth and height.
    pub size: [f64; 3],
}

impl BoxShape {
    /// Its eight corners: the bottom four, then the top four.
    pub fn points(&self) -> [[f32; 3]; 8] {
        let [x, y, z] = self.size.map(|s| (s / 2.0) as f32);
        [
            [-x, -y, -z],
            [x, -y, -z],
            [x, y, -z],
            [-x, y, -z],
            [-x, -y, z],
            [x, -y, z],
            [x, y, z],
            [-x, y, z],
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
        let mut stamp = [0; 32];
        if let Some(shape) = BOX.get(data) {
            for (bytes, side) in stamp.chunks_mut(8).zip(shape.size) {
                bytes.copy_from_slice(&side.to_le_bytes());
            }
        }
        stamp
    }
    fn memory(&self, _data: &Data) -> usize {
        size_of::<BoxShape>()
    }
    fn bounds(&self, data: &Data) -> Option<Bounds> {
        let [x, y, z] = BOX.get(data)?.size.map(|s| s / 2.0);
        Some(Bounds {
            min: Vec3::new(-x, -y, -z),
            max: Vec3::new(x, y, z),
        })
    }
    fn info(&self, data: &Data) -> Vec<InfoRow> {
        let Some(BoxShape { size: [w, d, h] }) = BOX.get(data) else {
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
    fn levels(&self) -> &[Level] {
        &[] // selected as a whole object only
    }
    fn prepare(&self, data: &Data, _: DisplayQuality, _: &CancelToken) -> Result<Drawn, Error> {
        let surface = Surface::polygons(box_of(data)?.points(), FACES, TOPOLOGY);
        Ok(Drawn::new(surface))
    }
    fn highlight(&self, _: &Data, _: &Drawn, _: &Components) -> Elements {
        Elements::new(Level::Face, [])
    }
    fn convert(&self, _: &Data, _: &Components, _: Level) -> Option<Components> {
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

/// In project files: the size.
impl DataCodec for BoxType {
    fn write(
        &self,
        data: &Data,
        _: Option<&Data>,
        out: &mut dyn ElementWriter,
    ) -> Result<(), Error> {
        out.set("size", Field::F64s(box_of(data)?.size.to_vec()))
    }
    fn read(&self, element: &dyn ElementReader, _: Option<&Data>) -> Result<Data, Error> {
        let size = element.f64s("size")?;
        let size = size
            .try_into()
            .map_err(|_| Error::invalid("a box has three sides"))?;
        Ok(BOX.wrap(BoxShape { size }))
    }
}

/// The Box node: makes a box from its parameters.
pub struct BoxNode;

impl NodeType for BoxNode {
    fn id(&self) -> NodeTypeId {
        NodeTypeId::declared("example.boxes.box_node")
    }
    /// No inputs, one box out, and its width, depth and height in scene
    /// units.
    fn description(&self) -> Description {
        let side = |name: &str, label: &str| {
            let length = ParameterKind::Number(Number::length().range(0.001, f64::INFINITY));
            Parameter::new(name, label, length, 1.0)
        };
        Description::new("Box")
            .output(Port::new("box", "Box", BOX.id()))
            .parameters(
                Schema::default()
                    .parameter(side("width", "Width"))
                    .parameter(side("depth", "Depth"))
                    .parameter(side("height", "Height")),
            )
    }
    fn cook(&self, cx: &mut CookCx<'_>, _inputs: &[Data]) -> Result<Outputs, CookError> {
        let size = [
            cx.number("width")?,
            cx.number("depth")?,
            cx.number("height")?,
        ];
        if size.iter().any(|&side| side <= 0.0) {
            return Err(CookError::message("every side must be longer than zero"));
        }
        Ok(Outputs::one(BOX.wrap(BoxShape { size })))
    }
}

/// The package: what Studio registers when it loads the plugin.
pub fn package() -> Package {
    Package::new(PackageId::declared("example.boxes"), "Boxes")
        .with_data_type(BoxType)
        .with_node_type(BoxNode)
}

/// The entry point Studio looks up in the plugin's library.
#[allow(unsafe_code)] // `no_mangle` keeps the name Studio looks for.
#[unsafe(no_mangle)]
pub fn kinogaki_package() -> Package {
    package()
}

#[cfg(test)]
mod tests;
