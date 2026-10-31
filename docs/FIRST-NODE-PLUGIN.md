# Write your first node plugin

This walkthrough builds a plugin that adds a Box node to Kinogaki Studio: a box data type that shows in the viewport, renders and saves; a Box node that makes one from its size and center, with a handle Studio drags; and the Boxes context and object kind that make the node reachable from Tab. It ends with the plugin built against Studio's plugin kit and installed.

[`examples/box-node`](../examples/box-node) is the finished plugin, under the `example` vendor and with an icon of its own. Here it is `acme`'s. [ARCHITECTURE.md](ARCHITECTURE.md) explains each part in more depth.

You need the Rust toolchain this repository pins (`rust-toolchain.toml`) and, for the last step, the plugin kit of the Studio release you install into.

## 1. The crate

A plugin is a Rust dylib. Create a crate:

```sh
cargo new --lib acme-boxes
cd acme-boxes
```

Copy this repository's `rust-toolchain.toml` into it: a plugin shares Studio's standard library, so it must be built by the same compiler. Then write `Cargo.toml`:

```toml
[package]
name = "acme-boxes"
version = "0.1.0"
edition = "2024"

[lib]
# A Rust dylib: Studio and its plugins share one standard library and one
# copy of the SDK.
crate-type = ["dylib"]

[dependencies]
kinogaki-studio-api = { git = "https://github.com/kinogaki/studio-sdk" }

[dev-dependencies]
# The fake host and its in-memory networks, for the tests.
kinogaki-studio-ui-api = { git = "https://github.com/kinogaki/studio-sdk" }
```

Pin the dependency (`rev = "…"`) to the SDK your Studio release's kit was built from once you ship. During development, cargo and these crates are all you need; step 7 builds the shipping library against the kit.

## 2. The data type

Everything on a wire is a `Data`: a shared, immutable value with the id of its data type. A plugin that makes a new kind of value registers a `DataType` for it, which tells Studio what it can't see inside the value: its label and wire color, a stamp (equal stamps mean equal values), its memory and bounds, its info rows, how it saves, and optionally how it shows and renders.

Replace `src/lib.rs` with the box and its typed handle. `BOX` wraps a `BoxShape` as `Data` and reads it back; it returns `None` for any other type.

```rust
//! Acme's Boxes plugin: a box data type, a Box node, and the Boxes
//! context and object kind the node lives in.
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
    /// Its eight corners: the bottom four, then the top four.
    pub fn points(&self) -> [[f32; 3]; 8] {
        let low = [0, 1, 2].map(|k| (self.center[k] - self.size[k] / 2.0) as f32);
        let high = [0, 1, 2].map(|k| (self.center[k] + self.size[k] / 2.0) as f32);
        let [a, b, c] = low;
        let [x, y, z] = high;
        [
            [a, b, c], [x, b, c], [x, y, c], [a, y, c],
            [a, b, z], [x, b, z], [x, y, z], [a, y, z],
        ]
    }
}

/// The six faces, as corner numbers counterclockwise seen from outside.
const FACES: [[u32; 4]; 6] = [
    [0, 3, 2, 1], [4, 5, 6, 7], [0, 1, 5, 4],
    [2, 3, 7, 6], [1, 2, 6, 5], [3, 0, 4, 7],
];

/// Every box has the same faces: one connectivity key.
const TOPOLOGY: u64 = 1;

/// Wraps boxes as `Data` and reads them back.
pub static BOX: LazyLock<Typed<BoxShape>> =
    LazyLock::new(|| Typed::new(DataTypeId::declared("acme.boxes.box")));

fn box_of(data: &Data) -> Result<&BoxShape, Error> {
    BOX.get(data).ok_or_else(|| Error::invalid("not a box"))
}
```

Ids are `<vendor>.<package>.<name>`, declared once and never derived from Rust names: projects store them. A package declares ids under its own vendor only; the registry refuses others.

Now the type itself. Its facets are optional: a type without `Displayable` isn't drawn, one without `Renderable` isn't rendered, and nothing fails.

```rust
/// The box data type.
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
        // The six numbers' bytes, hashed (FNV-1a).
        let mut stamp = [0; 32];
        if let Some(shape) = BOX.get(data) {
            let bytes = shape.size.iter().chain(&shape.center).flat_map(|v| v.to_le_bytes());
            let hash = bytes.fold(0xcbf2_9ce4_8422_2325_u64, |h, byte| {
                (h ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
            });
            stamp[..8].copy_from_slice(&hash.to_le_bytes());
        }
        stamp
    }
    fn memory(&self, _data: &Data) -> usize {
        size_of::<BoxShape>()
    }
    fn bounds(&self, data: &Data) -> Option<Bounds> {
        let shape = BOX.get(data)?;
        let corner = |sign: f64| {
            let [x, y, z] = [0, 1, 2].map(|k| shape.center[k] + sign * shape.size[k] / 2.0);
            Vec3::new(x, y, z)
        };
        Some(Bounds { min: corner(-1.0), max: corner(1.0) })
    }
    fn info(&self, data: &Data) -> Vec<InfoRow> {
        BOX.get(data).map_or_else(Vec::new, |shape| {
            let [w, d, h] = shape.size;
            vec![InfoRow::new("Size", format!("{w} × {d} × {h}"))]
        })
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
```

**In the viewport**, a type hands Studio packets to draw. `Surface` is the simple packet: points and polygons, whose triangles, edges and point markers it derives. A box has no component levels (`levels` is empty): it is selected as a whole object.

```rust
impl Displayable for BoxType {
    fn levels(&self) -> &[ElementKind] {
        &[]
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
```

**In renders**, engines never see a plugin's types: a type answers render items over a `TriangleSource`.

```rust
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
```

**In project files**, a type writes its own element through its codec and reads it back strictly: a missing or malformed field is an error, never a default.

```rust
impl DataCodec for BoxType {
    fn write(&self, data: &Data, _: Option<&Data>, out: &mut dyn ElementWriter) -> Result<(), Error> {
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
        Ok(BOX.wrap(BoxShape { size: three("size")?, center: three("center")? }))
    }
}
```

## 3. The node type

A `NodeType` is a description, read once when the package registers, and a cook. The description gives the node's label, its ports, its parameters at their defaults, and the viewport handles bound to them. Studio's core owns everything else: evaluation order, caching, cancellation, workers, undo and the inspector, which it draws from the parameters.

The box handle binds Studio's box handle to `size` and `center`: while a Box node is selected, Studio draws the box with a dot on each face and one in the middle, and dragging them writes the parameters. The plugin draws nothing.

```rust
/// The context of the networks inside Boxes objects.
pub fn network() -> ContextId {
    ContextId::declared("acme.boxes.network")
}

/// The Box node: makes a box from its size and center.
pub struct BoxNode;

impl NodeType for BoxNode {
    fn id(&self) -> NodeTypeId {
        NodeTypeId::declared("acme.boxes.box_node")
    }
    fn description(&self) -> Description {
        let vector = |number: Number| ParameterKind::Vector { components: 3, number };
        let sides = vector(Number::length().range(0.001, f64::INFINITY));
        let center = vector(Number::length());
        Description::new("Box")
            .context(network())
            .listed("Create", Category::Create, "Primitives", &["cube"])
            .output(Port::new("box", "Box", BOX.id()))
            .parameters(
                Schema::default()
                    .parameter(Parameter::new("size", "Size", sides, vec![1.0; 3]))
                    .parameter(Parameter::new("center", "Center", center, vec![0.0; 3])),
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
        Ok(Outputs::one(BOX.wrap(BoxShape { size, center })))
    }
}
```

A cook is a pure function of its inputs and parameters. It reads only what its type declares (`NodeType::reads`: time, input names, the selection, objects, the project), polls `cx.check()` in long work, and splits work with `cx.parallel_for`; it never starts threads or keeps caches of its own (`cx.scratch` is the core-owned slot for that).

Studio checks each parameter value against its declaration before a cook (a size under the range's 0.001 is refused), so the cook needn't.

## 4. Where the node is offered

A node type is reachable only where a context offers it. `Description::context` above names the context whose networks offer the Box node, and `Description::listed` puts it in that context's Tab menu, under Create, in the Primitives family, found by "cube" too. A type with no listing can still be added by code and files, never by Tab.

The context is the plugin's own: Studio has no general-purpose network a third party lists into. A context is one id with two halves. Its networks are declared as data (`NetworkContext`): how they evaluate (cooked node by node, by default), the core flags their nodes offer, and the element they are saved under. Its UI half (menus, keys, levels, an inspector) is optional; without one, Studio's own network pane, Tab and inspector serve it. [Write your first context plugin](FIRST-CONTEXT-PLUGIN.md) adds a UI half.

A context's networks live inside objects: an object kind names the context its objects open into, and Studio keeps one network beside each object. The Boxes kind is placed in the scene and shows the box its network yields.

```rust
/// The Boxes object kind.
pub fn boxes() -> ObjectKindId {
    ObjectKindId::declared("acme.boxes.boxes")
}

/// A Boxes object's body: the kind's own state, here none (its network,
/// which Studio keeps, makes its box).
#[derive(Debug)]
pub struct BoxesBody;

impl ObjectBody for BoxesBody {
    fn kind(&self) -> ObjectKindId {
        boxes()
    }
}

/// Boxes objects: placed in the scene, each holding a Boxes network.
struct BoxesKind;

impl ObjectKindSpec for BoxesKind {
    fn id(&self) -> ObjectKindId {
        boxes()
    }
    fn label(&self) -> &str {
        "Boxes"
    }
    fn icon(&self) -> &str {
        "cube"
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
}

/// A Boxes body saves nothing but its kind.
impl ObjectCodec for BoxesKind {
    fn write(&self, _body: &dyn ObjectBody, _out: &mut dyn ElementWriter) -> Result<(), Error> {
        Ok(())
    }
    fn read(&self, _element: &dyn ElementReader, _cx: &ReadCx<'_>) -> Result<Arc<dyn ObjectBody>, Error> {
        Ok(Arc::new(BoxesBody))
    }
}
```

`"cube"` is one of Studio's icons. A package can add its own (`Package::with_icon`), named under its id (`"acme.boxes/box"`); see [Icons](ARCHITECTURE.md#icons).

## 5. The package and its entry point

A `Package` is everything the plugin registers. `export_package!` writes the entry Studio looks up in the library; it catches a panic in `package` and keeps `unsafe` out of the plugin's code.

```rust
/// The package: what Studio registers when it loads the plugin.
pub fn package() -> Package {
    let context = NetworkContext::new(network(), "Boxes", "boxes").flags(FlagSet::all());
    Package::new(PackageId::declared("acme.boxes"), "Boxes")
        .with_data_type(BoxType)
        .with_context(context)
        .with_object_kind(BoxesKind)
        .with_node_type(BoxNode)
}

export_package!(package);

#[cfg(test)]
mod tests;
```

Studio admits the package through the SDK's `Registry` before anything of it is used: every id new and under the package's vendor, every data type it names registered. A package that fails is refused whole, listed as unavailable in Settings ▸ Plugins with the reason, and Studio keeps running.

## 6. Tests

A plugin's tests run with plain `cargo test`, no Studio needed. Create `src/tests.rs`. The first test registers the package as Studio does; the second cooks the node the way core does, with `Serial` workers; the third puts the node in a Boxes network on the SDK's fake host, adds and edits it as Tab and the inspector do (one undo step each), and cooks it.

```rust
//! The package registers, the node cooks, and it works in a network.
use super::*;
use kinogaki_studio_api::network::{NetworkRead, NewNode};
use kinogaki_studio_api::parallel::Serial;
use studio_ui_api::host::{HostNetwork, HostUndo};
use studio_ui_api::testing::{FakeHost, FakeNetwork};

#[test]
fn the_package_registers() {
    let mut registry = Registry::new();
    registry.register(package()).unwrap();
    assert!(registry.node_type(BoxNode.id()).is_some());
    assert_eq!(registry.object_kind(boxes()).unwrap().context(), Some(network()));
}

#[test]
fn the_node_makes_a_box() {
    let mut parameters = BoxNode.description().parameters.values();
    parameters.insert("size".into(), vec![2.0, 3.0, 4.0].into());
    let cancel = CancelToken::default();
    let mut scratch = None;
    let mut cx = CookCx::new(&parameters, &cancel, &Serial, &mut scratch);
    let data = BoxNode.cook(&mut cx, &[]).unwrap().data.remove(0);
    let bounds = BoxType.bounds(&data).unwrap();
    assert_eq!(bounds.max, Vec3::new(1.0, 1.5, 2.0));
}

#[test]
fn the_node_cooks_in_a_network() {
    let package = package();
    let object = ObjectRef(1);
    let mut host = FakeHost::new(network(), []);
    host.networks.insert(object, FakeNetwork::new(network(), &[&package]).unwrap());
    let mut added = None;
    host.edit_network(object, "Add Box", &mut |net| {
        added = Some(net.add_node(NewNode::new(BoxNode.id()).displayed(), [0.0, 0.0])?);
        Ok(())
    })
    .unwrap();
    let node = added.unwrap();
    host.edit_network(object, "Size", &mut |net| {
        net.set_parameter(node, "size", vec![2.0, 1.0, 1.0].into())
    })
    .unwrap();
    // A flat box is refused, and changes nothing.
    let flat = host.edit_network(object, "Flat", &mut |net| {
        net.set_parameter(node, "size", vec![2.0, 0.0, 1.0].into())
    });
    assert!(flat.is_err());
    assert_eq!(host.history(), ["Add Box", "Size"]);
    let data = host.networks[&object].cook(node).unwrap().data.remove(0);
    assert_eq!(BOX.get(&data).unwrap().size, [2.0, 1.0, 1.0]);
    host.undo();
    assert_eq!(host.networks[&object].display(), Some(node));
}
```

```sh
cargo test
```

`element::MemoryElement` tests a codec the same way (write a value, read it back, compare stamps); the finished example's tests do, and also check the handle's bindings.

## 7. Build against the kit

Studio loads plugins built against its **plugin kit**: the build of the SDK the release ships (`deps/`, with `libkinogaki_shared` holding every SDK crate), its standard library (`sysroot/`) and the kit's id (`kinogaki-kit`). The kit's README names the exact compiler. A plugin built against another kit is refused unopened.

Build the library with `rustc`, from a small root file that links the kit's shared library once and includes the plugin's own `src/lib.rs`. Here `$KIT` is the unpacked kit, and the commands are macOS's (Linux: `.so`; Windows: `.dll`):

```sh
cat > target/root.rs <<'EOF'
extern crate kinogaki_shared;
#[path = "../src/lib.rs"]
mod plugin;
pub use plugin::*;
EOF
rustc --edition 2024 --crate-type dylib --crate-name boxes -C prefer-dynamic \
    --sysroot "$KIT/sysroot" \
    --extern kinogaki_shared="$KIT/deps/libkinogaki_shared.dylib" \
    --extern kinogaki_studio_api="$(ls "$KIT"/deps/libkinogaki_studio_api-*.rmeta)" \
    -L dependency="$KIT/deps" \
    target/root.rs -o target/boxes.dylib
```

A plugin with UI adds `--extern studio_ui_api="$(ls "$KIT"/deps/libstudio_ui_api-*.rmeta)"` the same way.

## 8. Install it

Studio looks for plugins in the user's plugin folder (macOS: `~/Library/Application Support/Kinogaki Studio/plugins`; Windows: `%APPDATA%\Kinogaki Studio\plugins`; Linux: `$XDG_CONFIG_HOME/kinogaki-studio/plugins`, else `~/.config/kinogaki-studio/plugins`), in one folder per kind: `kernels/`, `nodes/`, `formats/` and `renders/`. A plugin is its library and a manifest beside it, named alike. The Boxes plugin is a node plugin:

```sh
PLUGINS="$HOME/Library/Application Support/Kinogaki Studio/plugins"
mkdir -p "$PLUGINS/nodes"
cp target/boxes.dylib "$PLUGINS/nodes/"
cat > "$PLUGINS/nodes/boxes.prisma" <<EOF
#prisma 4.0
def Plugin "plugin" {
    str id = "acme.boxes"
    str kind = "nodes"
    str kit = "$(cat "$KIT/kinogaki-kit")"
    str version = "0.1.0"
}
EOF
```

A plugin that builds on another package's types names it in its manifest, one `def Depends "depends0" { str id = "acme.trees" }` block each; Studio loads it after them, and not at all without them.

Start Studio: Settings ▸ Plugins lists Boxes, loaded or with the reason it isn't, and the Console says why a plugin was refused. Inside a Boxes object, Tab lists Box under Create; the node draws its box, the box handle drags its size and center, and the inspector shows Size and Center. Studio's Create menu offers the kinds a context's UI offers (`Creations`, in the next walkthrough), and projects that hold Boxes objects open with them.

## Where to go next

- [Write your first context plugin](FIRST-CONTEXT-PLUGIN.md): a context with its own UI, levels, commands and an inspector row.
- A node that reads inputs declares input `Port`s; its cook receives the wired values in port order (`CookCx::port` says which are whose). The inputs must be of data types some installed package registers. Studio's own Mesh and CAD data are its plugins' types, whose kernels are not public, so a third-party node works on its own data types (or another public package's).
- A node that updates quickly during a drag implements `NodeType::incremental`; one that shapes its inspector rows from what is wired implements `NodeType::schema`. See [Cooking](ARCHITECTURE.md#cooking) and [What a plugin uses](ARCHITECTURE.md#what-a-plugin-uses).
