# Your first file format

A file format is a package that reads files into the SDK's standard values and writes them back: Studio's File ▸ Import and Export, and the File node, use it. It knows no context and no kernel: what it makes is standard Geometry, materials, images, lights, cameras and instances, which any package's object kinds adopt.

[`examples/points-format`](../examples/points-format) is a whole format: XYZ, a text file of points, faces over them and the materials they show, read into the standard values and written from them, with its tests. This page walks through it. The reference is [ARCHITECTURE.md](ARCHITECTURE.md#formats) and the rustdoc of `FileFormat`, `FormatWriter`, `Reading`, `Writing` and `Imported`.

## The crate

A format is a Rust dylib, as every plugin is (Studio and its plugins share one standard library and one copy of the SDK):

```toml
[lib]
crate-type = ["dylib"]

[dependencies]
kinogaki-studio-api = "…"
kinogaki-render-api = "…"   # material networks, for a format with materials

[dev-dependencies]
kinogaki-studio-api-testing = "…"
```

Its library exports its package with `export_package!(package)`; Studio loads it from its `formats` folder.

A format of any size keeps the file's own model, its parser and its writer in a module of its crate that uses nothing of the SDK but what bounds a read (`format::ReadBudget`, and `format::files::Named` for the files it names; [Reading untrusted files](#reading-untrusted-files)), what a write shares (`format::Unique` for names given once each, `format::files::replace` for every file written) and the SDK's workers (`parallel::map`, `map_blocks`; a package never starts a thread of its own). That module is tested and measured on its own, and nothing in it changes when the SDK does. The format converts between that model and the standard values at its boundary, and nowhere else. The example's file is small enough that its reader is one type, `File`, beside the format.

## The package and the format

```rust
const TYPES: &[DataTypeId] = &[standard::materials().id(), standard::geometry().id()];

pub struct Points;

impl FileFormat for Points {
    fn id(&self) -> FormatId {
        FormatId::declared("example.points.xyz")
    }
    fn label(&self) -> &str {
        "XYZ Points"
    }
    fn extensions(&self) -> &[&str] {
        &["xyz"]
    }
    fn produces(&self) -> &[DataTypeId] {
        TYPES
    }
    // options, schema, read and writer below
}

pub fn package() -> Package {
    Package::new(PackageId::declared("example.points"), "XYZ Points").with_format(Points)
}

export_package!(package);
```

- `id`, `label` and `extensions` say what Studio's file dialogs offer; `detects` (no by default) recognizes a file by its first bytes where the extension isn't enough.
- `produces` lists the data types a read makes: the standard ones' ids are `const`, so the list is a constant. A read gives what `Reading::wants` names when it names one (a File node's type).
- `options` gives every option with its default, by name, and `schema(chosen, purpose)` its rows: File ▸ Import's (`Purpose::Objects`), which Studio shows on Settings ▸ Import (File ▸ Import opens no dialog of its own and reads with what is set there), or one value's (`Purpose::Value`: a File node's, which leaves out options that shape objects). `format::options` holds the rows most formats show (`scale`, `embed`: Embed Textures, `toggle`) and reads a typed option (`option`). The example has two: Scale, and Embed Textures.
- `read` and `writer`, below. The host saves the options, one field per option (`format::options::write`, read back strictly by `format::options::read`), so a format holds no codec of its own for them.

## Reading

```rust
fn read(&self, input: &Input<'_>, chosen: &FormatOptions, reading: &Reading<'_>) -> Result<Imported, Error> {
    let bytes = input.bytes()?;
    let text = std::str::from_utf8(&bytes)
        .map_err(|_| Error::invalid(format_args!("{input}: not text")))?;
    let scale = options::option(chosen, SCALE, Value::number)?;
    let mut file = File::read(text, scale * per_metre(reading.units), reading)?;
    let named = Named::new(input, chosen, reading);
    let materials = file.materials(&named, reading.colors)?;
    // …the items: the materials first, then the geometry
}
```

- **The input.** An `Input` is a file on disk (`Input::file`), a file kept in the project or packed in another (`Input::embedded`, an `EmbeddedFile` whose bytes are shared), or bytes lent from memory (`Input::memory`). `input.bytes()` lends its bytes; a message about the file names it as `input` displays (its path, else its name). A format that keeps files packed in its file (a usdz's textures) takes `input.held()`: all of its bytes as one `EmbeddedFile`, read once and shared, whose ranges `held.slice(name, range)` keep without a copy.
- **What the read is for.** `Reading` carries the purpose, the project's `Units`, the host's `ColorService`, the read's `ReadBudget` (and its cancellation) and, for a value, the type wanted.
- **Into the project.** A format converts at its boundary: lengths from the file's unit into the project's (`Reading::units`; the example's metres times `per_metre`), and colors from the file's space into the working space (`Reading::colors`). A format names the spaces files mean by their interop names: `color::LIN_REC709` (the example's material colors), `color::SRGB_TEXTURE`, `color::SRGB_DISPLAY`. A format of a hierarchy changes its file's space into the project's with `format::Conversion` (a scale, a turn, a mirroring): points by `point`, normals by `direction`, lengths by `length`, a node's placement by `placement` (`C·M·C⁻¹`, so no placement carries the change of units) and a camera's or a light's by `frame` (`Q·M`: its value keeps the frame it looks down); a writer converts back through its `inverse`. A format of one piece of polygons may place it with the editor's `gather` (`kinogaki_studio_api::edit::editor`) instead.
- **Long reads stop when they are no longer wanted.** The example calls `reading.check()` on every line: a read Studio cancels ends with `Error::cancelled()`.
- **What comes back.** `Imported` holds `FileItem`s, parents before children (each named, placed relative to its parent, shown or hidden, holding a value or nothing for a group), and warnings (`Diagnostic::warning`) for what was read with loss or left out. A damaged or truncated file is an `Error` naming the line or the input, never a panic.

## Reading untrusted files

A file may come from anywhere, and a hostile one costs nothing to make. A read runs on a thread with a 2 MiB stack, and Studio can't catch a stack overflow or a failed allocation: either ends the process with the user's unsaved work. So every reader keeps one policy, the SDK's:

- **What a file claims is checked before it is used.** `Reading::budget`, a `format::ReadBudget`, holds what one read may spend (`ReadLimits`: bytes of the files it names, one allocation, all allocations together, nesting depth, a decoder's output per byte of input, a picture's pixels) and the read's cancellation. Before allocating for a count or a size the file gives, a reader charges it (`budget.vec::<T>(count)`, `budget.allocate(bytes)`); before following a level of nesting, `budget.nest(depth)`; before decoding a stream, `budget.expand(encoded, decoded)`; before sizing a picture, `budget.pixels(width, height)`. Arithmetic on the file's numbers is checked (release builds keep overflow checks, so a wrapped multiply is a panic): `checked_mul`, or `saturating_mul` into the budget, which refuses the overflow. Clones of a budget share what was spent, so workers draw on one budget.
- **Recursion is bounded.** A recursive reader stops at the budget's depth (256 levels hold on 2 MiB with room to spare); a structure that may be deeper is walked with loops, compared and dropped without recursion.
- **Everything is read once.** References, includes, layers and shared values are followed once each, keyed by the identity of what they name (the canonical path `Named::file` gives, a value's offset in the file): a cycle is refused, and a repeat reuses what was read. Work stays in proportion to the file: names made unique are counted per base name (`format::Unique`), never probed one by one; lookups go through maps; a table being made takes its entries in place (`Attributes::set`, `set_group`).
- **Cancellation reaches the decoders.** Every budget check also polls the cancellation, and a loop over the file's records calls `budget.check()` often enough that a cancelled read stops within milliseconds.
- **Only regular files are read.** `Input` reads the file itself as `Named` reads the files it names (`format::files::regular`): looked at before it is opened, a pipe, a device or a folder refused, and read to the size it had when opened.
- **The files a file names are reached through `format::files::Named` alone.** It resolves a name relative to the read file's folder, or wherever an absolute name says: a file may name files anywhere. It reads regular files only, never a device, a pipe or a folder; charges what it reads to the budget; names a found file by its canonical path, which is its identity; and returns its warnings, one a reason (`Named::warnings`), which the read adds to its own. Pictures are `Named::image`'s answer: a path, or the picture's bytes when Embed Textures is on. A file held in memory names nothing on disk: its pictures keep the names it gives, for the project's folder to find. Names in the file's text are decoded first (a URI's `%2F` is a `/`) and then resolved; another URI scheme is left out and said.
- **A failure is an error, never an abort.** What the budget refuses is an `Error` naming the file; what a reader left out or approximated is a warning.

Each hostile file a format met becomes a test of its own: the file made by code where it can be, read through `kinogaki_studio_api_testing::read_hostile` (as Import reads, on a 2 MiB stack) within small `ReadLimits`, asserting a clean error or warning within a time bound.

## Materials and lights

A 3D format brings its materials, its lights (a dome with its image) and its cameras as standard values beside its geometry. The contract, both ways:

| | A read returns | Export hands a writer |
|---|---|---|
| Materials | an item of `standard::materials()` for each library the file holds, `Materials { materials }`: under the group holding it (a USD Scope of Materials where it is), or one at the top for a file of one list | an item for each Material Library, at its place among the objects, all its Materials; a library outside the objects written that they name at the top |
| A material | `ImportedMaterial { key, name, inside, standard, double_sided }`: `key` what the polygons name it by, `inside` its whole Shading network as the file records it, `standard` its OpenPBR Surface form, which a host takes when it lacks a node type `inside` holds (none when `inside` is MaterialX's nodes alone), `double_sided` the Material's Double Sided | every Material of a library, named or not, `key` its path, `inside` its whole network as authored, `standard` its OpenPBR form, as its engine approximates it, `double_sided` its Double Sided |
| A texture | an Image node in `inside` (`image_files::image_node::reading(&source)`) wired from its `image` output into the texture node's `file` input, one per distinct file and color space in a material | the same; a path absolute |
| Which material a face shows | the primitive `material` text of polygons (the file's key) | the Material's path |
| A material bound per placement | a mesh used once: its primitives' `material`; a mesh several placements share: one prototype of a `standard::instances()` item, each instance's `material` (Instance domain) naming its binding | the same |
| A light | `standard::light().wrap(light)?`: render-api's standard `Light`, checked as it is wrapped (an `Error` for one out of range); a dome's image is an `ImageRef::File(source)`: a file on disk or the file's bytes, and how its colors read | the same; a dome naming an Image node reads, as `ImageRef::File`, what that node reads at the frame exported, a path absolute |
| A camera | `standard::camera().wrap(camera)?`: render-api's `Camera`, checked as it is wrapped | the same |
| Shared meshes | `standard::instances()` items, each prototype one `Data` however many instances and items place it | the same: a value several items hold, written once |

The example reads each `material` statement as an OpenPBR Surface whose base color is the statement's color, converted into the working space, or an image texture reading its picture through an Image node:

```rust
let mut rgb = [*color];
colors.to_working(color::LIN_REC709, &mut rgb)?;
let surface = NewNode::materialx("ND_open_pbr_surface_surfaceshader").expect("MaterialX's OpenPBR Surface");
let mut inside = NetworkSeed::default();
let surface = inside.node(surface.with("base_color", Value::Color(rgb[0])).displayed());
if let Some(file) = picture.as_deref().and_then(|p| named.image(p, None)) {
    let texture = inside.node(NewNode::materialx("ND_image_color3").expect("MaterialX's image"));
    let image = inside.node(image_node::reading(&ImageSource::new(file, ImageColor::Auto)));
    inside.wire(image, image_node::IMAGE, texture, "file");
    inside.wire(texture, "out", surface, "base_color");
}
```

and its faces name their material in the primitive `material` text.

A file that packs its pictures and lights its scene with a dome: say a glTF PBR material reading a picture packed in the file, and a dome lit by an HDR beside it:

```rust
let held = input.held()?; // the file's bytes, shared
let mut inside = NetworkSeed::default();
let surface = inside.node(NewNode::materialx("ND_gltf_pbr_surfaceshader").unwrap().displayed());
let base = inside.node(NewNode::materialx("ND_gltf_colorimage").unwrap());
inside.wire(base, "outcolor", surface, "base_color");
// A picture packed in the file: a range of its bytes, kept without a copy.
let packed = ImageFile::Embedded(held.slice("wood.png", 1200..413_200)?);
let wood = inside.node(image_node::reading(&ImageSource::new(packed, ImageColor::Auto)).named("wood"));
inside.wire(wood, image_node::IMAGE, base, "file");
let red = ImportedMaterial { key: "0".into(), name: "Red".into(), inside, standard: None, double_sided: false };
let hdr = named.image("sky.exr", None).unwrap(); // found beside the file
let sky = ImageRef::File(ImageSource::new(hdr, ImageColor::Auto));
let dome = Light { shape: LightShape::Dome { image: Some(sky), ground: None }, ..Light::default() };
let items = vec![
    FileItem::top("Materials", standard::materials().wrap(Materials { materials: vec![red] })),
    FileItem::top("Chair", standard::geometry().wrap(chair)), // its primitives' `material` is "0"
    FileItem::top("Sky", standard::light().wrap(dome)?), // checked: an out-of-range light is an error
];
```

Imported, it is a Material Library holding Red (its glTF PBR, texture and Image node as made); Chair's faces name `/Materials/Red`, and the dome reads the HDR itself (embedded with Embed Textures, as a texture is).

**Sources.** An `ImageSource` is a file and how its colors read:
- **The file.** `ImageFile::Path`, absolute, or `ImageFile::Embedded`, an `EmbeddedFile` with its name: what the file packs is embedded; a file on disk it names is `Named::image`'s answer.
- **Its colors.** `ImageColor::Auto` (the file's tag, then the project's file rules), `Data` (a normal map, a roughness: never converted), or a space, made by `ImageColor::space(name)`, which refuses a name a save would read back as Auto or Data; it is the Image node's Color. A MaterialX `colorspace` (and USD's on an asset input) maps through `format::materialx::image_color` and back through `colorspace` (`raw`, `none` and `data` are data); USD's `sourceColorSpace` `sRGB` is `color::SRGB_TEXTURE`.
- **Undecoded encodings.** A format passes bytes and decodes nothing: a picture no installed format reads stays embedded as it is, and its Image node says so.

**MaterialX nodes.** A node of MaterialX's library is `NewNode::materialx(signature)` (`ND_gltf_pbr_surfaceshader`): its type `kinogaki.mtlx.<node>` holding its signature. Inputs keep MaterialX's names. An engine's own node is its type's id with its values by input name. Kinogaki's own definitions (`kinogaki-mtlx`'s `kinogaki/`: glTF's textures with their whole sampler, Surface Unlit Cutout, Phong Surface) are known to Studio alone: a file other applications read names one that inherits a MaterialX definition by that definition, its own inputs beside (`Library::in_materialx`), and a reader takes a node holding inputs its definition lacks for the Kinogaki definition that adds them (`Library::holding`).

**Lengths.** A material input that is a length in the scene's units (its kind a `Length`: glTF PBR's thickness and attenuation distance, OpenPBR's and Standard Surface's transmission depth and scattering distance, a MaterialX `distance` of no fixed unit) is in the project's units, as geometry is: a format reading a file of other units converts it as it converts the file's points (`NetworkSeed::scale_lengths`), and back when it writes. A length a file says no unit of stays as read.

**What Import does.** It adopts each materials item as a Material Library (named as the item, in the Part its parent item becomes), each Material double sided as `double_sided` says and holding `inside` when every node type in it is installed, else `standard`, else nothing, with one warning a package missing. The libraries are adopted first, so the keys become paths in every other item wherever it comes: the polygons' and instances' `material` and instances' `material_map` pairs; each object still goes among its siblings in the file's order. Lights become core's lights of their standard types, which every engine renders, cameras Cameras, and polygons and instances objects of the kind adopting them (Mesh's), each network showing its value from a Stash.

**What Export does, and writing a material's model.** A writer resolves nothing and calls no engine: a network is data. A format of one shading model reads a material as a network (`MaterialNetwork::try_from(&inside)`: its MaterialX nodes and their images; refused for an engine's own, whose `standard` it reads instead) and takes it to its model with `MaterialNetwork::translated` (`open_pbr_surface`, `standard_surface`, `gltf_pbr`, `UsdPreviewSurface`): the chain of translation graphs, folded, the textures as they are. The example writes each material's OpenPBR base color, out of the working space into linear Rec.709 (`color::settled` keeps a color read and written back the number it was), or the picture its base color reads. A format of MaterialX documents, or of a file holding them, converts between a document's materials and Studio's with `format::materialx::{read, write}` (values, colors in MaterialX's working space `format::materialx::SPACE`, files through `Named` and `Beside`, lengths and their units, the shown node, Double Sided, an engine's material by its standard form), `kinogaki-mtlx` reading and writing the document itself.

## Writing

```rust
impl FormatWriter for Points {
    fn accepts(&self) -> &[DataTypeId] {
        TYPES
    }
    fn write(&self, path: &Path, items: &[FileItem], writing: &Writing<'_>, _options: &FormatOptions) -> Result<Vec<Diagnostic>, Error> {
        // …each material's line, then each item's points and faces, placed
        replace(path, |file| file.write_all(text.as_bytes()))
            .map_err(|e: std::io::Error| Error::invalid(format_args!("{}: {e}", path.display())))?;
        Ok(said.into_iter().map(Diagnostic::warning).collect())
    }
}
```

- `accepts` lists the data types it writes; other items' values are skipped, their groups kept. A writer of polygons alone asks the host for any other value as polygons (`Writing::meshed`, the installed mesher).
- `write(path, items, writing, options)` writes `path` and nothing else it didn't make, whole or not at all: `format::files::replace` writes a new file beside and renames it over `path` once it is whole. The pictures a document names go through `format::files::Beside`: a file on disk by its path relative to the document, an embedded one written into `<stem>_textures/` beside it, once for equal bytes, never over a file of other bytes. Names a write makes unique (keys, files) are a `format::Unique`'s.
- It places each item by its placement after its parents' (`FileItem::placement`, `parent`) into the file's space, converts lengths and colors out of the project's, calls `writing.check()`, and returns its warnings for what it wrote with loss or left out ("only polygons are written"), which Export shows; an `Err` is a write that failed.
- Its own options are `FormatWriter::options`, and `schema` their rows, which Studio shows on Settings ▸ Export; every file Studio writes with it (File ▸ Export, a final render's, a picture of the Canvas) is written with what is set there, else the defaults. A writer never sees the format's reading options.

## Image formats

A format of pictures (PNG, JPEG, OpenEXR) produces and accepts the standard Image (`standard::image()`). A picture's size is checked against the read's budget (`ReadBudget::pixels`) before its samples are sized. A read gives one item holding one Image as the file holds it: its samples, every channel group a layer, its colors tagged as the file says (`ColorTag`) and every attribute as metadata; it converts nothing, since the host resolves the tag. A writer says whether its files are `display_referred` (PNG, JPEG): the host then gives it a display's code values tagged with the display (the sRGB display as `color::SRGB_DISPLAY`), else linear light tagged with the working space; the writer copies the tag (a named space's primaries from `Writing::colors`' `primaries`, where the file holds them) and warns of what it can't hold. A format converting a column of colors (a mesh's vertex colors) converts it in place in single floats (`ColorService::convert_f32`), with no copy of the column.

## Tests

The example's tests (`src/tests.rs`) are the pattern, through the format alone, with `kinogaki_studio_api_testing::Unconverted` (or a color service of your own) as the colors:

- a file lands in the standard values; units and the scale option place it; a bad line is an error naming it;
- materials read first, their colors in the working space and their pictures as Image nodes, embedded with Embed Textures;
- what is written reads back the same (`kinogaki_studio_api_testing::format::round_trip`, in a folder of the test's own, `format::Scratch`), an embedded picture written beside the file once;
- the conformance suite's fixtures as a `Consumer`: every standard geometry value written and read back keeps its points.

Add the hostile files of [Reading untrusted files](#reading-untrusted-files), cancellation of long reads and writes, parallel work compared on one thread and several (`kinogaki_studio_api_testing::workers::install`), and the reference files of the format's own test suite where one exists.

## Checklist

1. A dylib crate exporting a package that carries the format.
2. The parser and writer apart from the SDK, but for the read's budget, `Named`, `replace`, `Unique` and the workers.
3. The `FileFormat` and, if it writes, its `FormatWriter`: standard values only; units, axes and colors converted at the boundary; what the file claims checked against the read's budget, and the files it names reached through `Named`.
4. The tests above.
