# Your first render engine

A render engine is a package that renders what a host sends it. It depends on `kinogaki-render-api` alone, and sees no kernel, no document and no network: only prims, in batches, with what changed about each.

The worked example is core's testkit: `core/testkit/src/tracer` is a small CPU ray caster written against this page, and `core/scene/tests/tracer.rs` drives it through core's render sync. Everything below that it does is tested there.

## What a host knows of an engine

An engine is described without a device, so a host can list it, build its rows and read its files with no GPU:

```rust
let description = EngineDescription::builder(EngineId::new("acme.ray.engine"), "Ray")
    .capabilities(Capabilities {
        prims: vec![PrimKind::Triangles, PrimKind::Instancer, PrimKind::Camera,
                    PrimKind::Material, PrimKind::Texture],
        types: standard::lights(),
        aovs: vec![AovSpec::new(aov::standard::beauty(), true)],
        motion: Motion { transforms: true, velocities: true },
        ..Capabilities::default()
    })
    .settings(vec![SettingGroup::new("main", "Main", true)], settings)
    .materials(node_defs, Arc::new(MyRules))
    .build();
```

A host sends nothing the capabilities don't name: no prim of another kind, no light of a type it doesn't declare (a standard one keeps its id across engines, `standard::lights`, `standard::fog`), no motion it doesn't blur. A package carries it with `Package::with_engine` (its `display::Engine` also makes one on the host's device) or `with_engine_description` (headless). Its id is under the package's vendor.

`display::Engine::make` receives the host's `wgpu` device and queue, the wake and `Options`: an engine renders on the host's device and shows its frames as textures there (`Display::acquire`), so nothing is copied. It may share none and put the pixels in `Frame::pixels` instead.

## The scene: batches of prims

`RenderEngine::update(SceneUpdate)` never blocks: an engine queues the batch and applies it at a pass boundary, folding batches it hasn't applied with `SceneUpdate::merge`. A batch holds `added`, `removed`, `changed`, new `settings`, and `settled` when a drag ended and refitted acceleration structures may be rebuilt. What a prim names comes before it: textures, then materials, then shapes, then Instancers.

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

`Shape::primvar(name)` finds it, whichever its domain. Storage (F32, F64, I32, I64, Text), width (1 to 255) and arrays are the attribute's own: nothing narrows on the way, an integer never blends, and colors (`Meaning::Color`) are linear ACEScg. Sent: `N` (shading normals; none: flat), `uv`, `v` when `Motion::velocities`, a curve's `width` (none: your thinnest), and what the shape's materials read (`MaterialNetwork::primvars`).

`Subsets::of(primitive)` is the material a primitive renders with; none is your default material. `Visibility { visible: false }` removes a prim; `camera: false` keeps it lighting, shadowing and reflecting.

## Instances

`Prim::Instancer` places other prims: `prototypes`, `prototype_of(i)`, `xforms` (a column of `Xform`, relative to the Instancer) and `primvars` (`Instance` and `Detail`: `Cd`, `id`, `name`, `v`). **A prim an Instancer names renders only through it**: skip it at the top level. A prototype is a shape or another Instancer (a value holding several shapes arrives as an Instancer of them), so hardware instancing takes each instance's chain of `Xform::compose` and shares what it built of the shape. Never flatten the geometry: an edit of the instances sends you a new `xforms` column and nothing else.

## Time

Sample times are frames from the frame rendered. `Camera::shutter` says when it opens and closes; `Sampled<Xform>` carries placements across it (`is_still` for most): blend the two neighbouring samples, component by component. Deformation is the `v` primvar in scene units a second, on points and on instances; `RenderSettings::rate` says how long a frame is (`Shape::point_at`). Topology never changes across a shutter.

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
- `display`: a simple approximation of your material for the shaded viewport: a color, the primvar it multiplies by, the image it multiplies by (`DisplayTexture`: the file value a node holds, and the primvar of its coordinates, by default `uv`) and whether it fills its shape as a volume. All of it multiplies, linear. The host reads the file through the color service as it reads a render's, once, and shows the color alone until it is read; you never decode or upload anything for it. Name one image, the one that says most about the material's color. Render views show the real look.
- `translate`: a material of yours from a network holding nodes you don't declare (another engine's, a file's). None renders your default.

An engine's starting materials are `NodeCreation`s (`Package::with_creation`): a Material with its inside seeded.

## Previews

`RenderEngine::preview(PreviewRequest)` returns at once; a host shows the `PreviewImage` when it arrives. A request is the SDK's preview shape: `shape`, `lighting`, `side`, the `network` with its colors converted and files resolved, and the `textures` it names. Render it any way you like, on your own thread; `PreviewRequest::scene` gives it as prims for a scene of its own (a sphere on a floor, a dome and a key light, a camera), and a floor naming no material takes your default. Your settings for previews are yours.

## Views, results and being polite

- `view` sets a view's camera, film and `ViewMode`; `drop_view` ends it; `pause` holds its refinement. `Interactive` refines progressively, `Navigating` keeps one short pass in flight, `Final` runs to the settings' samples. A `realtime` engine's every pass is a whole frame, and it never gets a shrunken film.
- Publish each whole pass as a `Frame` (`latest`), and call the `Wake` **once**. Nothing else may wake the host.
- `finish` returns a `Pending<Outputs>`: the requested AOVs once its samples are done. Every `Pending` is yours to resolve with its `Deliver`; drop one you won't answer and the host sees `Cancelled`.
- A host cancels by dropping the `Pending`: `Deliver::wanted` turns false. Ask it between passes and stop.
- With nothing to do, sleep on a condition: an idle engine costs nothing and wakes no one.
- Engines output linear ACEScg.

## Testing

Copy `core/scene/tests/tracer.rs`: it makes a `Target` over a `RenderGeometry` as Studio does, and checks instances, motion, previews, idle, cancel, and that editing a parameter sends one Material with `PARAMS`. `RecordingEngine` stores what it was sent, for tests of a host.
