# Your first render engine

A render engine is a package that renders what a host sends it. Its renderer depends on `kinogaki-render-api` alone, and sees no kernel, no document and no network: only prims, in batches, with what changed about each. Its package (the lights it brings as object kinds, `export_package!`) depends on `kinogaki-studio-api` too, with its `wgpu` feature.

Studio's engines, Spectre (a spectral path tracer) and Pixel (a realtime pixel-art engine, open source, building beside this repository alone: [plugin-pixel](https://github.com/kinogaki/plugin-pixel)), are written against this page and nothing else. [`examples/flat-engine`](../examples/flat-engine) is a whole engine in three files: a CPU ray caster on the SDK's render thread, shown through a texture on the host's device. The snippets below are an engine called Ray under the `acme` vendor.

## What a host knows of an engine

An engine is described without a device, so a host can list it, build its rows and read its files with no GPU:

```rust
let description = EngineDescription::builder(EngineId::declared("acme.ray.engine"), "Ray")
    .capabilities(Capabilities {
        prims: vec![PrimKind::Triangles, PrimKind::Instancer, PrimKind::Camera,
                    PrimKind::Material, PrimKind::Texture],
        types: standard::lights(),
        aovs: vec![AovSpec::new(aov::standard::beauty(), true)],
        motion: Motion { transforms: true, velocities: true },
        ..Capabilities::default()
    })
    .settings(vec![Section::new("main", "Main", true)], settings)
    .materials(node_defs, Arc::new(MyRules))
    .build();
```

`film_bytes` is what you hold for a view a pixel of its film, every buffer and kept image counted: a host budgets the films rendering at a time by it. A host sends nothing the capabilities don't name: no prim of another kind, no light of a type it doesn't declare (a standard one keeps its id across engines, `standard::lights`, `standard::fog`), no motion it doesn't blur. A standard output is named by its `aov::standard` constant and declared as the SDK describes it (`aov::standard::get`): the registry refuses one declared otherwise. A package carries it with `Package::with_engine` (its `display::Engine` also makes one on the host's device) or `with_engine_description` (headless). Its id is under the package's vendor.

Your lights, and your fog if you render one, are object kinds of your package: `render::lights::DeclaredKind::lights(kind, family, types)` (placed, one kind whatever the light is) and `DeclaredKind::atmosphere(kind, fog)` (at the root). Their `renders()` facet answers prims of the types you declare, so the host syncs them to whichever engine renders, draws their rows from your descriptors and decodes the standard ones for the viewport; a standard light is a light value formats carry. A type of your own is `PrimType::new` with its `PrimTypeId`, under your package's vendor (the registry refuses another vendor's, and an own type two packages declare), and its sections (`group`; `standard::emission_params` gives a light's emission rows), and a section of yours on a standard type is `group` on it (Pixel's gobo on its spot and distant lights): another engine renders such a light by its own schema of the type. A standard light decodes with `standard::Light::decode` and the fog with `standard::Fog::decode`; an RGB engine takes its `radiance()` (the emission's color × intensity × 2^exposure, linear ACEScg), and `standard::Emission::color` gives any emission's color at luminance 1: a blackbody's or a CIE illuminant's as the spectral library integrates it, so a "D65" shows one color in every engine. Render Outputs are the host's: a person picks your engine there, and its settings rows are your descriptors.

`display::Engine::make` receives the host's `wgpu` device and queue, the wake and `Options`: an engine renders on the host's device and shows its frames as textures there (`Display::acquire`), so nothing is copied. When a view goes the host calls `Display::drop_view` after `RenderEngine::drop_view`: whatever the display keeps for that view goes, since a later view may get the same id. It may share none and put the pixels in `Frame::pixels` instead. A film whose pixels are meant to be seen as pixels (pixel art) is `ShownFilm::pixelated`: a host magnifying it samples the nearest texel.

## The scene: batches of prims

`RenderEngine::update(SceneUpdate)` never blocks: an engine queues the batch and applies it at a pass boundary, folding batches it hasn't applied with `SceneUpdate::merge`. A batch holds `added`, `removed`, `changed`, new `settings` (what every view shares: units, frame rate, the Camera whose shutter placements blur across), and `settled` when a drag ended and refitted acceleration structures may be rebuilt. What a prim names comes before it: textures, then materials, then shapes, then Instancers.

A `Change` carries the prim whole; `dirty` says what differs from what you hold:

| Bit | What changed | What to do |
|---|---|---|
| `POINTS` | a shape's positions, at `Change::points` | refit, updating those points |
| `TOPOLOGY` | a shape's connectivity (or an Instancer's prototypes) | rebuild; everything sized by it came with it |
| `PRIMVARS` | a shape's or Instancer's primvars | compare by name and column (`Column::ptr_eq`): an unchanged primvar is the same column |
| `MATERIALS` | which materials a shape's primitives render with | re-read `Shape::subsets` |
| `TRANSFORM` | a placement, its samples, or an Instancer's instances (`Instancer::xforms` by `ptr_eq`) | update the top level; a million instances moving is this bit on one prim |
| `VISIBILITY` | `Visibility` | |
| `PARAMS` | a Material's, Light's or Camera's values, or a prim's name | update constants; the wiring stands |
| `NETWORK` | a Material's nodes or links | translate it again |

Any batch restarts a view's film unless it changes nothing the view sees.

## Shapes

`Prim::Shape` is triangles or curves in their own space: `Shape::topology` (`Topology::Triangles` or `Topology::Curves`, whose `Primitives` table holds each curve's kind, degree and knots over the shape's points), `points` (`Column<[f32; 3]>`, read with `[i]`, `iter`, `runs`), `xform`, `visibility`, `path` (what Cryptomatte names it by) and `subsets`. A prim is built by its builder, which refuses anything half-formed: you never check a size.

`Shape::primvars` is the SDK's one attribute table. A primvar's domain is its interpolation class:

| Domain | One value a | Read at a hit |
|---|---|---|
| `Vertex` | triangle corner, three a triangle in corner order (`3 * t + k`); curve control vertex | blended |
| `Point` | point | blended by the corners' points |
| `Primitive` | triangle or curve | constant |
| `Detail` | the whole shape | constant |

`Shape::primvar(name)` finds it, whichever its domain; `Shape::corners::<N>(triangles, name)` reads it at each triangle corner as every engine does (one number spreads to all `N`, a narrower value fills its first components and the rest are 0). `Units::up().direction()` is the world's up. Storage (F32, F64, I32, I64, Text), width (1 to 255) and arrays are the attribute's own: nothing narrows on the way, an integer never blends, and colors (`Meaning::Color`) are linear ACEScg. Sent: `N` (shading normals; none: flat), `uv` (`v` 0 is an image's bottom row, as USD lays them out), `v` when `Motion::velocities`, a curve's `width` (none: your thinnest), and what the shape's materials read (`MaterialNetwork::primvars`).

`Subsets::of(primitive)` is the material a primitive renders with; none is your default material. `Visibility { visible: false }` removes a prim; `camera: false` keeps it lighting, shadowing and reflecting.

## Instances

`Prim::Instancer` places other prims: `prototypes`, `prototype_of(i)`, `xforms` (a column of `Xform`, relative to the Instancer) and `primvars` (`Instance` and `Detail`: `Cd`, `id`, `name`, `v`). **A prim an Instancer names renders only through it**: skip it at the top level. A prototype is a shape or another Instancer (a value holding several shapes arrives as an Instancer of them), so hardware instancing takes each instance's chain of `Xform::compose` and shares what it built of the shape. Never flatten the geometry: an edit of the instances sends you a new `xforms` column and nothing else.

## Time

Sample times are frames from the frame rendered. `Camera::shutter` says when it opens and closes; `Sampled<Xform>` carries placements across it (`is_still` for most): blend the two neighbouring samples, component by component. Deformation is the `v` primvar in scene units a second, on points and on instances; `SceneSettings::rate` says how long a frame is (`Shape::point_at`). Topology never changes across a shutter.

## Materials

The SDK has no shading model. An engine declares its nodes as data; a host makes one node type of each, listed under the engine's label in a Material's network, and compiles what the user wires:

```rust
let surface = NodeDef::new(NodeTypeId::declared("acme.ray.surface"), "Surface", "Surface")
    .input(Input::Driven(color, Socket::Color))      // a value a wire may drive
    .input(Input::Setting(reads))                    // ValueKind::Attribute: a primvar it reads
    .output(Output::new("out", "Out", Socket::Closure("surface".into())));
```

A value's kind is the SDK's `ValueKind`, its descriptor the one lights use, so a row needs no UI code. `Socket::Closure` is yours to name: closures meet only their own name. Numbers drive numbers. An image is `ValueKind::File`, a color unless `NodeDef::data_file` says otherwise.

`Prim::Material` holds a `MaterialNetwork`: `nodes()` (each `def`, every value by name, `textures` by input name), `links()`, `output()` (the node shown: your own output node, if you want several terminals). Translate it to your shaders. A network with no nodes is your default material: what a look naming nothing renders. A node whose file isn't read yet isn't there (its links gone, what it drove holding its value), and the host sends the Material again once it is; colors are linear ACEScg.

What needs your code is `MaterialRules`:
- `display`: a simple approximation of your material for the shaded viewport: a color, the primvar it multiplies by, the image it multiplies by (`DisplayTexture`: the file value a node holds, the primvar of its coordinates, by default `uv`, the transform that moves them before the image is read, and its `Sampling`: each axis's wrap past the image's edges, and smooth or nearest texels) and whether it fills its shape as a volume. All of it multiplies, linear. The host reads the file through the color service as it reads a render's, once, and shows the color alone until it is read; you never decode or upload anything for it. Name one image, the one that says most about the material's color. Render views show the real look.
- `translate`: a material of yours from a network holding nodes you don't declare (another engine's, a file's). None renders your default.

An engine's starting materials are `NodeCreation`s (`Package::with_creation`): a Material with its inside seeded.

## Previews

`RenderEngine::preview(PreviewRequest)` returns at once; a host shows the `PreviewImage` when it arrives. A request is the SDK's preview shape: `shape`, `lighting`, `side`, the `network` with its colors converted and files resolved, and the `textures` it names. A host asks previews of an engine it made for them and drops at rest, never of one showing its views, so render them as you render a view: `PreviewRequest::scene` gives the request as prims (a sphere on a floor, a dome and a key light, a camera) that replace the last request's, and a floor naming no material takes your default. No second engine is needed. Your settings for previews are yours.

## The render thread

An engine never blocks its caller: its renderer and scene live on a render thread of its own, and every `RenderEngine` call is a message to it. `render_api::thread` is that plumbing, written once: `EngineThread::start` runs your thread's loop on a `Mailbox` (the host's `Message`s in order, a sender back to itself for your completion callbacks, and the `Published` frames and problems), and the `EngineThread` it returns is the `RenderEngine` a host holds. Your own messages travel as `Message::Engine`, and what your handle offers besides (statistics, a helper for your display) is its `extra`. `finish` and `preview` arrive as a `Message::Read` of a `Reading` (a final render's outputs, or a swatch's beauty as its picture); a preview is a view of its own, `thread::PREVIEW`, whose scene `PreviewScene::replacing` swaps for the last request's, seen as your `preview_view` says. Publish through `Published::publish`, which drops a frame of a view the host dropped since it gave it, so a view id given again never shows the old view's frame.

## Views, results and being polite

- `view` sets a view's camera, film, `ViewMode` and settings (`values`, by your descriptors' names); `drop_view` ends it, stopping its passes; `pause` holds its refinement. `Interactive` refines progressively, `Navigating` keeps one short pass in flight, `Final` runs to the settings' samples. A `realtime` engine's every pass is a whole frame, and it never gets a shrunken film.
- One engine holds one scene and any number of views over it: two views of different settings and cameras share every prim. Views take turns: render one for a few samples, then the next that still needs them, a `Navigating` one ahead of any turn. After a restart the view last named by `focus` goes first. `focus` itself restarts nothing.
- A view that can't render says why through `problem` and wakes the host once. The other views render on.
- Publish each whole pass as a `Frame` (`latest`), and call the `Wake` **once**. Nothing else may wake the host.
- `finish` returns a `Pending<Image>`: the requested AOVs once its samples are done, as one `Image` of the view's size. `aov::image` makes it from each output's values (interleaved, `Aov::channels` a pixel, ids as their bits) and the paths your Cryptomatte IDs name: the beauty is the main layer (`""`, `R G B A`), every other output a layer of its id (`X Y Z` for vectors, `Z` for depth, `Y` for other numbers, `id` as U32 for ids), and a Cryptomatte output of *n* ranks is `n.div_ceil(2)` layers `<id>00`, `<id>01`… of two (ID, coverage) ranks each, its manifest the image's metadata under Cryptomatte 1.2's keys (`aov::cryptomatte`: the names whose IDs the layers hold, each ID `cryptomatte::id` of its name). Every `Pending` is yours to resolve with its `Deliver`; drop one you won't answer and the host sees `Cancelled`.
- A host cancels by dropping the `Pending`: `Deliver::wanted` turns false. Ask it between passes and stop.
- With nothing to do, sleep on a condition: an idle engine costs nothing and wakes no one.
- Engines output linear ACEScg.

## Testing

An engine is tested with plain cargo against this crate alone, as a host drives it: make prims with their builders, send them as `SceneUpdate`s, open a view and read what comes back. Test at least:

- each prim kind you declare, added, changed by each dirty bit you handle, and removed; an Instancer's prototypes rendering only through it, and an instance edit sending a new `xforms` column alone;
- motion across the shutter: sampled placements and the `v` primvar;
- a Material sent with `PARAMS` alone updating constants, and with `NETWORK` translating again; `translate` and `display` for a network of nodes you don't declare;
- previews, two views sharing one scene, `focus`, `pause`, `drop_view`;
- one wake per published pass, none while idle; a dropped `Pending` stopping the work (`Deliver::wanted`), and `finish` delivering the AOVs it was asked for;
- closed-form scenes where your engine allows them (a white furnace returns the light it is lit by).

GPU tests need a device: mark them `#[ignore]` and run them on a machine with one. The testing crate's `wgpu` feature gives them one (`engine::device`) and counts your wakes (`engine::Waiter`: wait until a condition holds, take a `Pending`'s value). `RecordingEngine` is the opposite tool: it stores what it was sent, for tests of a host.

`engine::conformance` is the check Studio's engines pass: a scene of prims for each prim kind and standard light type your description declares, each rendered as a final frame beside the same scene without its subject. A scene passes when your engine renders it without failing, its final frame is an `Image` of the view's size whose main layer is the beauty, its beauty is finite, and its subject shows:

```rust
let (device, queue) = engine::device();
let make = |wake| Ray.make(&device, &queue, wake, &Options::default());
let report = engine::conformance("Ray", &make);
assert!(report.failures().is_empty(), "{report}");
```

## Build against the kit

An engine builds against the plugin kit as a node plugin does ([FIRST-NODE-PLUGIN.md §7](FIRST-NODE-PLUGIN.md#7-build-against-the-kit), which also builds a plugin's own dependencies), with the render interface added to the externs: `--extern kinogaki_render_api="$(ls "$KIT"/deps/libkinogaki_render_api-*.rmeta)"`. The kit's render interface carries its `wgpu` feature, and `kinogaki_render_api::display::wgpu` is the wgpu your engine draws with. Install the library in the `renders/` folder.
