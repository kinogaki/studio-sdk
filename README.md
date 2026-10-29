# studio-sdk

What a Kinogaki Studio plugin compiles against. Studio is an environment: with no packages installed it organises a scene (Parts, Cameras, an empty Material Library, a File node that reads nothing). Meshes, CAD bodies, file formats and render engines all come from packages, and this repository holds the contract between Studio and those packages. Its version is the version plugins and Studio agree on.

| Crate | Role |
|---|---|
| `kinogaki-render-api` (`crates/render-api`) | The render interface between a host and its engines: prims (meshes, instancers, cameras, materials, textures) and the prim types an engine declares (lights, fog: parameter schemas, the standard UsdLux ones shared), coalesced scene updates with dirty bits, material networks and an engine's vocabulary, settings and outputs by id, the `RenderEngine` trait and `EngineDescription`, and the triangle source contract (`TriangleSource`) every data type renders through. Engines depend on this crate alone. |
| `kinogaki-studio-api` (`crates/api`) | The headless API: ids, `Data` and `Typed`, `DataType` with its optional facets (`Displayable`, `Pickable`, `Tabular`, `Renderable`), `Components`, `DataCodec`, `Conversion`, the node cook contract (`NodeType`, `CookCx`, `Incremental`, `Outputs`) and `HeldEdit`, `ObjectBody` and `ObjectKindSpec`, `FileFormat`, `Imported` and `Reading`, `SessionSlot`, `CancelToken`, `Time`, `Package` and the `Registry`; and the plain types they exchange: `math`, `params`, `element`, `viewport`, `render` and the `parallel` workers service |
| `kinogaki-studio-ui-api` (`crates/ui-api`, library `studio_ui_api`) | The UI layer: how a plugin's panels, tool cards, overlays and settings pages ask the host to draw. Geometry (`Pos`, `Vec2`, `Rect`, `Align2`), `Color`, the host-filled `Theme`, fonts and icons by name, `Id`, plain-data `Input`, the `Painter` and `Shape`s, the `Ui` trait (immediate widgets, layout, and the host's composites: property sheets, cards, popovers, value fields) with `UiExt`; property sheets as data (`sheet::Schema`); the pure viewport maths of the transform gizmo and point labels; and `testing`'s headless `RecordingUi` and `RecordingPainter`. **Plugins never see Studio's UI library or egui: the host draws.** |

[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) describes the API and the UI layer: what each part is for, the rules every package follows, and how the environment uses it.

## Coming next

These parts are designed but not written yet:

- **The host traits** in `kinogaki-studio-ui-api`: what a context, tool or panel can ask of the host beyond drawing. This covers selection, transactions and undo, cooking results, viewport drawing layers, gizmo painting and picking, menus and shelves, status and messages, and settings sections. They are written against `Host` traits, never against the editor's internals, and Studio implements them.
- **`testkit`**: a toy kernel and context, plus the conformance suite (`conformance::check`) that every kernel's data types pass.

## Who depends on it

- **studio** (the environment) implements the host side and registers packages.
- **studio-mesh** and **studio-cad** (and later studio-cam, studio-usd and the render plugins) implement it over their kernels.

Plugins never link Studio's editor crate.

## Self-contained

**The SDK never depends on a private Kinogaki crate; Studio adapts at the boundary.**

The SDK depends on the Rust standard library and, behind `kinogaki-render-api`'s `wgpu` feature only, on wgpu from crates.io. Every type the API exposes is its own:

| Module | What it defines | Studio converts it to and from |
|---|---|---|
| `math` | `Vec3`, `Affine3`, `Mat4`, `Bounds`: plain `Copy` data, no algorithms | its maths library |
| `params` | `Value`, `Parameters`, `Schema` (sections and typed parameters: number with unit and range, integer, toggle, choice, text, code, file, color, vector, ramp), `Diagnostic` | its inspector's property schemas |
| `element` | `ElementWriter` / `ElementReader` (typed fields, metadata, children, shared values) and `MemoryElement` | its project documents |
| `viewport` | `Surface` (the display packet), `Level`, `Elements`, `Hit` | its viewport's packets and picks |
| `render` | `RenderItem`; the source contract (`TriangleSource`, `Ranges`, `Look`, `MeshAttribute`) is `kinogaki-render-api`'s, re-exported | its render scenes |
| `parallel` | `Workers`, the host service behind `CookCx::parallel_for` / `parallel_map`, and `Serial` for tests | its shared worker threads |
| the `ui-api` crate | Geometry, colors, the theme, input, the painter, widgets and property sheets | its UI library (each call maps to the call it replaces) |

So a package written against the SDK compiles against nothing private, and the SDK builds and tests from a plain clone. A plugin's kernel may use any library it likes inside; it converts at its own boundary, as Studio does at its.

Studio's `tools/check_architecture.py` enforces the rule: the SDK's crates depend on no Kinogaki crate but their own. Its one third-party dependency is wgpu 30.0.1, pinned exactly and resolved from crates.io as a normal dependency, only with the `wgpu` feature: a wgpu host and its engines share the device and display textures, so they must agree on the version. The default build has no dependencies at all.

## Building

```sh
git clone https://github.com/kinogaki/studio-sdk.git
cd studio-sdk
cargo test --workspace --locked
cargo test -p kinogaki-render-api --features wgpu --locked
```

The `wgpu` feature is portable code with no platform-specific parts; it is verified on macOS so far.

## Checks

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo clippy --workspace --all-targets --locked --features kinogaki-render-api/wgpu -- -D warnings
cargo test --workspace --locked
cargo test -p kinogaki-render-api --features wgpu --locked
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked --features kinogaki-render-api/wgpu
```

Studio's `tools/check_all.py` runs the same checks on the SDK.

## Licenses

Some of `kinogaki-studio-ui-api`'s geometry and color arithmetic follows egui's, formula for formula, so Studio's egui-based UI library converts its values bit for bit. [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md) credits it, and egui's licenses are in [licenses/](licenses/). The SDK copies no third-party code otherwise; its crates.io dependency (wgpu, with the `wgpu` feature) carries its own licenses.
