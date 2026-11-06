# studio-sdk

What a Kinogaki Studio plugin compiles against. Studio is an environment: with no packages installed it organises a scene (Parts, Cameras, an empty Material Library, a File node that reads nothing). Meshes, CAD bodies, file formats and render engines all come from packages, and this repository holds the contract between Studio and those packages. Its version is the version plugins and Studio agree on.

| Crate | Role |
|---|---|
| `kinogaki-value` (`crates/value`) | The value model: declared ids (one type per kind of thing), `Data` (a type-erased value of a registered data type), `Value` and `ValueKind` (what every node parameter, format option, render setting, declared prim value and material port holds, its limits and options), `Condition`, and the structured values `ramp::Ramp` (every ramp's encoding and evaluation: nodes, the ramp widget and engines alike) and `ToneCurve`, `RefFilter` and `ObjectRef`. It depends on `kinogaki-geometry` alone. |
| `kinogaki-geometry` (`crates/geometry`) | The standard geometry values every package shares: `Geometry` (polygons, polylines and curves over one shared `Primitives` table: rows of point ids with a `Kind` each, its `Basis` and whether it closes, in any order, with its canonical edge index; each primitive read as a `Curve` of linear, Bézier, B-spline, Catmull-Rom or NURBS basis), `brep::Brep` (boundary-representation bodies with exact analytic and NURBS geometry, persistent names and history, built by `BrepBuilder`), the typed attribute model (`Attributes` of `Attribute`s: storage × width × meaning, arrays, per `Domain`; indexed `Text`, groups as `Elements`, the standard names table, unique stable `id`s), `Selection` (`Elements` of a domain, with its members' ids) and the `Incidence` selection rules read, `Detail` and `Tolerances`. Each is a validated, immutable value over shared `Column`s, with typed cache slots and the cache rule in its docs. Kernels, formats and engines read and build these; Studio's core registers, displays and saves them. |
| `kinogaki-mtlx` (`crates/mtlx`) | MaterialX 1.39, our own implementation: `Document` (read from and written to MaterialX's XML as MaterialX writes it, 1.36 to 1.38 upgraded on reading), the `library` of MaterialX 1.39.5's definitions (stdlib, pbrlib, the bxdf shading models and their translation graphs, cmlib, nprlib, lights; vendored verbatim in `libraries/` with MaterialX's Apache-2.0 licence and their checksums, `libraries.sha256`) and Kinogaki's (`kinogaki/`), each definition's typed inputs (`NodeDef`, `Port`), the graphs implementing them resolved (`Graph`), translation chains between shading models, and evaluation of a node of values. It depends on nothing. |
| `kinogaki-render-api` (`crates/render-api`) | The render interface between a host and its engines: prims (shapes: triangles or curves with typed primvars; instancers, cameras, materials, textures) and the prim types an engine declares (lights, fog: parameter schemas, the standard UsdLux ones shared), coalesced scene updates with dirty bits, material networks and an engine's vocabulary, settings and outputs by id, the `RenderEngine` trait and `EngineDescription`, the shape source contract (`ShapeSource`) every data type renders through, and the SDK's preview shape (`PreviewRequest`). An engine's renderer depends on this crate alone; its package also on `kinogaki-studio-api`. |
| `kinogaki-viewport-api` (`crates/viewport-api`) | The viewport packet contract: what a data type hands Studio's viewport to draw. Surface packets (`SurfaceSource`: point positions, vertex shading records, triangles, line classes with their looks, point markers, sparse draw updates, derived shading plans, see-through face runs) and `packet_as` to read one back as its own type; element sets and selections; live preview patches and GPU motions; overlay, guide and scene line layers. Its maths is the SDK's geometry vocabulary (`Vec3`, `Xform`, `Bounds`). The `bytemuck` feature derives `Pod` on the records a renderer uploads. Kernels depend on this crate alone for drawing. |
| `kinogaki-edit` (`crates/edit`) | The edit contracts core and the kernels share, over `kinogaki-geometry` alone: the `editor` (`Editor`, the one way a stored value changes: points and primitives added, removed, welded, split and moved, attributes, groups and ids following, unchanged columns shared, and a `Change` saying what changed; `gather` joining placed values), selection rules (`select::convert` by `Coverage`, over any value's `Incidence`; `select::reresolve` by stable ids), the stable-id rule (`ids::fresh`, `ids::merged`) and modes (`Mode`, `Expand::Connected` pieces), a move as data (`deform::Deform`: a `Move`, per-point weights and a `Symmetry`; `place` and `apply`, the one function a drag's preview and its commit share) and a drag's moving points (`preview::Moving`). Re-exported as `kinogaki_studio_api::edit`. |
| `kinogaki-studio-api` (`crates/api`) | The headless API: ids, `Data` and `Typed`, `DataType` with its optional facets (`Displayable`, `Pickable`, `Tabular`, `Renderable`), `Components`, `DataCodec`, the node cook contract (`NodeType`, `CookCx`, `Outputs`) and `HeldEdit`, `ObjectBody` and `ObjectKindSpec`, `FileFormat`, `Imported` and `Reading`, `SessionSlot`, `CancelToken`, `Time` and `Rate` (a moment of the scene on the tick grid, and frames a second: the geometry crate's), `Key` and `Interp` (a parameter's keys over time), `Icon`, `kernel` (a kernel's `Verb`s, which any package runs by id while the kernel is loaded), `script` (the host's script engine, which code nodes run through), `Package` (with its render engines' descriptions, and their makers behind the `wgpu` feature), `export_package!` (a plugin library's entry point) and the `Registry`; the standard data types (`standard`: the ids, handles and codecs of `geometry`'s values) and their conformance suite (`conformance`); and the plain types they exchange: the geometry vocabulary, `params`, `element`, `viewport` and `render` |
| `kinogaki-studio-ui-api` (`crates/ui-api`, library `studio_ui_api`) | The UI layer: how a plugin's panels, tool cards, overlays and settings pages ask the host to draw. Geometry (`Pos`, `Vec2`, `Rect`, `Align2`), `Color`, the host-filled `Theme`, fonts and icons by name, `Id`, plain-data `Input`, the `Painter` and `Shape`s, the `Ui` trait (immediate widgets, layout, and the host's composites: property sheets, cards, popovers, value fields) with `UiExt`; property sheets as data (`sheet::Schema`); the pure viewport maths of the transform gizmo and point labels; the host interface (`host`: the capabilities a package's UI asks of Studio through one call context) and a package's UI (`package`: `UiPackage`, its contexts' parts, `WithUi::with_ui`, which adds it to the package); and `testing`'s headless `RecordingUi`, `RecordingPainter`, `FakeHost` (with in-memory networks, `FakeNetwork`) and `Hosting`. **Plugins never see Studio's UI library or egui: the host draws.** |
| `kinogaki-studio-api-testing`, `kinogaki-studio-ui-api-testing` (`crates/api-testing`, `crates/ui-api-testing`) | Test support, dev-dependencies only: the conformance suite's fixtures and invariants, and the UI layer's headless fakes (`FakeHost`, `FakeNetwork`, `RecordingUi`, `RecordingPainter`) |

[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) describes the API and the UI layer: what each part is for, the rules every package follows, and how the environment uses it.

To write a plugin, follow the two walkthroughs:

- [Write your first node plugin](docs/FIRST-NODE-PLUGIN.md): a data type, a node type that makes it, the context and object kind that make the node reachable from Tab, tests, and building against the plugin kit.
- [Write your first context plugin](docs/FIRST-CONTEXT-PLUGIN.md): a context with its own UI (levels, commands, an inspector row) and its tests on the fake host.

## Who depends on it

- **Studio** (the environment) implements the host side, loads plugin libraries and admits their packages.
- **Plugins** implement packages against it: Studio's own (Mesh, CAD, rendering, Comp, formats) and third parties' alike.

Plugins never link Studio's editor; they see only the SDK.

## Not written yet

- The conformance suite's checks of a package's own data types and node types (stamps, codec round trips, empty values, pure parameter hooks). The conformance suite checks the standard data types today.

## Self-contained

**The SDK never depends on a private Kinogaki crate; Studio adapts at the boundary.**

The SDK depends on the Rust standard library, on two crates.io crates at exact versions for its standard geometry values (blake3 1.8.7 for the topology fingerprint and bytemuck 1.25.2 for plain records and casts into columns), and, behind optional features only, on wgpu (`kinogaki-render-api`'s `wgpu` feature) and bytemuck (`kinogaki-viewport-api`'s `bytemuck` feature). Every type the API exposes is its own:

| Module | What it defines | Studio converts it to and from |
|---|---|---|
| `geometry` (re-exported at the root) | the one vocabulary: `Error` (an `ErrorKind` and the `Diagnostic` the user reads: what every fallible call, a kernel's operation and a node's cook alike, returns), `Vec3`, `Xform` (f64 affine, column vectors), `Bounds`, `Ray`, `Projective`, `Units`, `Time` (ticks of 1/705,600,000 s), `Rate`, `Sampled` | its maths library |
| `params` | `Value`, `Parameters`, `Schema` (sections and typed parameters: number with unit and range, integer, toggle, choice, text, code, file, path, reference with its `RefFilter`, color, vector, float or color ramp with `params::ramp`'s encoding, `kinogaki-value`'s `ramp` re-exported; whether each can be keyed, `Parameter::animatable`) | its inspector's property schemas |
| `element` | `ElementWriter` / `ElementReader` (typed fields, metadata, children, shared values) and `MemoryElement` | its project documents |
| `viewport` | `Surface` (the simple display packet, a `SurfaceSource` itself) and `Hit`; packets, `ElementKind` and `ElementSet` are `kinogaki-viewport-api`'s, re-exported | its viewport's picks |
| `render` | `RenderItem`; the source contract (`ShapeSource`, `Topology`, `Ranges`) is `kinogaki-render-api`'s, re-exported | its render scenes |
| `geometry::parallel` | `Workers`, the host's shared workers, installed once per process; `chunks_mut` / `map_blocks` run on them (on the calling thread until a host installs its own) | its shared worker threads |
| the `ui-api` crate | Geometry, colors, the theme, input, the painter, widgets and property sheets | its UI library (each call maps to the call it replaces) |

So a package written against the SDK compiles against nothing private, and the SDK builds and tests from a plain clone. A plugin's kernel may use any library it likes inside; it converts at its own boundary, as Studio does at its.

Studio's architecture check enforces the rule: the SDK's crates depend on no Kinogaki crate but their own. Its third-party dependencies are pinned exactly and resolved from crates.io as normal dependencies: blake3 1.8.7 and bytemuck 1.25.2 with `kinogaki-geometry` (the standard values' fingerprint, plain records and casts), and two with one feature each: wgpu 30.0.1 with `kinogaki-render-api`'s `wgpu` (a wgpu host and its engines share the device and display textures, so they must agree on the version), and bytemuck 1.25.2 with `kinogaki-viewport-api`'s `bytemuck` (a renderer casts the packet records it uploads). The default build depends on blake3 and bytemuck alone.

## Example plugins

[`examples/box-node`](examples/box-node) is a whole node plugin in one file: a box data type that shows in the viewport, renders as twelve triangles and saves its size and center; a Boxes object kind opening into a Boxes network context; and a Box node, offered and listed in Tab there, that makes a box from its size and center, with a box handle Studio drags. Its tests cook the node the way Studio does, and add and edit it in a network on the fake host. [Write your first node plugin](docs/FIRST-NODE-PLUGIN.md) builds it step by step; https://studiosdk.kinogaki.com walks through it too.

[`examples/flat-engine`](examples/flat-engine) is a render engine: a CPU ray caster on the SDK's render thread, its frames shown through a texture on the host's device; its test runs the SDK's engine conformance. [Your first render engine](docs/FIRST-RENDER-ENGINE.md) is its contract.

[`examples/points-format`](examples/points-format) is a file format: XYZ point lists read into the standard geometry and written from it, with one option; its tests run the conformance suite's fixtures through it as a `Consumer`.

[`examples/pin-board`](examples/pin-board) is a plugin with UI: a Pin Board object opening into a context of its own (its networks and its UI), with two selection levels, a Pin command and an inspector row, all in the one package it exports. Its tests drive the UI on the fake host, as Studio drives it. [Write your first context plugin](docs/FIRST-CONTEXT-PLUGIN.md) builds it step by step.

Studio's own tests build both against its plugin kit, load them as plugin libraries, run the pin board's commands and cook the Box node in a network.

## Building

```sh
git clone https://github.com/kinogaki/studio-sdk.git
cd studio-sdk
cargo test --workspace --locked
cargo test -p kinogaki-render-api --features wgpu --locked
cargo test -p kinogaki-studio-api --features wgpu --locked
cargo test -p kinogaki-viewport-api --features bytemuck --locked
```

`rust-toolchain.toml` pins the exact compiler Studio and its plugins are built with: a plugin is a Rust dylib sharing Studio's standard library, so it must come from the same compiler build.

The `wgpu` feature is portable code with no platform-specific parts; it is verified on macOS so far.

## Checks

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo clippy --workspace --all-targets --locked --features kinogaki-render-api/wgpu,kinogaki-viewport-api/bytemuck -- -D warnings
cargo test --workspace --locked
cargo test -p kinogaki-render-api --features wgpu --locked
cargo test -p kinogaki-viewport-api --features bytemuck --locked
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked --features kinogaki-render-api/wgpu,kinogaki-viewport-api/bytemuck
```

Studio runs the same checks on the SDK.

## License

The SDK is licensed under the Kinogaki Studio SDK license, [LICENSE.md](LICENSE.md).

## Third-party notices

Some of `kinogaki-studio-ui-api`'s geometry and color arithmetic follows egui's, formula for formula, so Studio's egui-based UI library converts its values bit for bit. [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md) credits it, and egui's licenses are in [licenses/](licenses/). The SDK copies no third-party code otherwise; its crates.io dependencies (blake3 and bytemuck; wgpu behind its feature; and the crates they use) carry their own licenses, which THIRD_PARTY_NOTICES.md lists.
