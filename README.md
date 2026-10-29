# studio-sdk

What a Kinogaki Studio plugin compiles against. Studio is an environment: with no packages installed it organises a scene (Parts, Cameras, an empty Material Library, a File node that reads nothing). Meshes, CAD bodies, file formats and render engines all come from packages, and this repository holds the contract between Studio and those packages. Its version is the version plugins and Studio agree on.

| Crate | Role |
|---|---|
| `kinogaki-render-api` (`crates/render-api`) | The render interface between a host and its engines: prims (meshes, instancers, cameras, materials, textures) and the prim types an engine declares (lights, fog: parameter schemas, the standard UsdLux ones shared), coalesced scene updates with dirty bits, material networks and an engine's vocabulary, settings and outputs by id, the `RenderEngine` trait and `EngineDescription`, and the triangle source contract (`TriangleSource`) every data type renders through. Engines depend on this crate alone. |
| `kinogaki-viewport-api` (`crates/viewport-api`) | The viewport packet contract: what a data type hands Studio's viewport to draw. Surface packets (`SurfaceSource`: point positions, vertex shading records, triangles, line and point classes with their looks, sparse draw updates, derived shading plans, see-through face runs) and `packet_as` to read one back as its own type; element sets and selections; live preview patches and GPU motions; overlay, guide and scene line layers. Its maths is plain arrays. The `bytemuck` feature derives `Pod` on the records a renderer uploads. Kernels depend on this crate alone for drawing. |
| `kinogaki-studio-api` (`crates/api`) | The headless API: ids, `Data` and `Typed`, `DataType` with its optional facets (`Displayable`, `Pickable`, `Tabular`, `Renderable`), `Components`, `DataCodec`, `Conversion`, the node cook contract (`NodeType`, `CookCx`, `Incremental`, `Outputs`) and `HeldEdit`, `ObjectBody` and `ObjectKindSpec`, `FileFormat`, `Imported` and `Reading`, `SessionSlot`, `CancelToken`, `Time`, `Package` (with its render engines behind the `wgpu` feature), `export_package!` (a plugin library's entry point) and the `Registry`; and the plain types they exchange: `math`, `params`, `element`, `viewport`, `render` and the `parallel` workers service |
| `kinogaki-studio-ui-api` (`crates/ui-api`, library `studio_ui_api`) | The UI layer: how a plugin's panels, tool cards, overlays and settings pages ask the host to draw. Geometry (`Pos`, `Vec2`, `Rect`, `Align2`), `Color`, the host-filled `Theme`, fonts and icons by name, `Id`, plain-data `Input`, the `Painter` and `Shape`s, the `Ui` trait (immediate widgets, layout, and the host's composites: property sheets, cards, popovers, value fields) with `UiExt`; property sheets as data (`sheet::Schema`); the pure viewport maths of the transform gizmo and point labels; the host interface (`host`: the capabilities a package's UI asks of Studio through one call context) and a package's UI (`package`: `UiPackage`, its contexts' parts, `register`); and `testing`'s headless `RecordingUi`, `RecordingPainter`, `FakeHost` and `Hosting`. **Plugins never see Studio's UI library or egui: the host draws.** |

[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) describes the API and the UI layer: what each part is for, the rules every package follows, and how the environment uses it.

## Coming next

These parts are designed but not written yet:

- **`testkit`**: a toy kernel and context, plus the conformance suite (`conformance::check`) that every kernel's data types pass.

## Who depends on it

- **studio** (the environment) implements the host side and registers packages.
- **studio-mesh** and **studio-cad** (and later studio-cam, studio-usd and the render plugins) implement it over their kernels.

Plugins never link Studio's editor crate.

## Self-contained

**The SDK never depends on a private Kinogaki crate; Studio adapts at the boundary.**

The SDK depends on the Rust standard library and, behind optional features only, on two crates.io crates: wgpu (`kinogaki-render-api`'s `wgpu` feature) and bytemuck (`kinogaki-viewport-api`'s `bytemuck` feature). Every type the API exposes is its own:

| Module | What it defines | Studio converts it to and from |
|---|---|---|
| `math` | `Vec3`, `Affine3`, `Mat4`, `Bounds`: plain `Copy` data, no algorithms | its maths library |
| `params` | `Value`, `Parameters`, `Schema` (sections and typed parameters: number with unit and range, integer, toggle, choice, text, code, file, color, vector, ramp), `Diagnostic` | its inspector's property schemas |
| `element` | `ElementWriter` / `ElementReader` (typed fields, metadata, children, shared values) and `MemoryElement` | its project documents |
| `viewport` | `Surface` (the simple display packet), `Level`, `Elements`, `Hit`; the full packet contract is `kinogaki-viewport-api`'s | its viewport's picks |
| `render` | `RenderItem`; the source contract (`TriangleSource`, `Ranges`, `Look`, `MeshAttribute`) is `kinogaki-render-api`'s, re-exported | its render scenes |
| `parallel` | `Workers`, the host service behind `CookCx::parallel_for` / `parallel_map`, and `Serial` for tests | its shared worker threads |
| the `ui-api` crate | Geometry, colors, the theme, input, the painter, widgets and property sheets | its UI library (each call maps to the call it replaces) |

So a package written against the SDK compiles against nothing private, and the SDK builds and tests from a plain clone. A plugin's kernel may use any library it likes inside; it converts at its own boundary, as Studio does at its.

Studio's `tools/check_architecture.py` enforces the rule: the SDK's crates depend on no Kinogaki crate but their own. Its third-party dependencies are pinned exactly, resolved from crates.io as normal dependencies, and each comes only with one feature: wgpu 30.0.1 with `kinogaki-render-api`'s `wgpu` (a wgpu host and its engines share the device and display textures, so they must agree on the version), and bytemuck 1.25.2 with `kinogaki-viewport-api`'s `bytemuck` (a renderer casts the packet records it uploads). The default build has no dependencies at all.

## Example plugin

[`examples/box-node`](examples/box-node) is a whole plugin in one file: a box data type that shows in the viewport, renders as twelve triangles and saves its size, a Box node that makes one from its width, depth and height, and the package that registers both. Its tests cook the node the way Studio does. https://studiosdk.kinogaki.com walks through it.

Studio does not load plugin libraries yet; the example builds the library a plugin will ship as.

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

Studio's `tools/check_all.py` runs the same checks on the SDK.

## Licenses

Some of `kinogaki-studio-ui-api`'s geometry and color arithmetic follows egui's, formula for formula, so Studio's egui-based UI library converts its values bit for bit. [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md) credits it, and egui's licenses are in [licenses/](licenses/). The SDK copies no third-party code otherwise; its crates.io dependencies (wgpu and bytemuck, each behind its feature) carry their own licenses.
