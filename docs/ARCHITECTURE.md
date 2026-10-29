# Architecture

The SDK has two crates:
- `kinogaki-studio-api` (`crates/api`) is the headless API: everything a package registers and exchanges that has no UI.
- `kinogaki-studio-ui-api` (`crates/ui-api`, library `studio_ui_api`) is the UI layer: how a plugin's panels, cards, overlays and settings pages ask the host to draw. It is described in [The UI layer](#the-ui-layer) below.

This page explains how their parts fit together and the rules packages follow. The rustdoc on each item is the reference.

# The headless API

## Environment and packages

Studio is the **environment**. With no packages installed, it is the editor, networks, undo, files, Part, Camera, an empty Material Library and a File node that can read nothing. Everything else is a **package**:

| Kind | Registers here | Example |
|---|---|---|
| Kernel | data types (`DataType`), how held edits apply to them (`HeldEdit`) and their session state (`SessionSlot`) | the mesh kernel adapter: `kinogaki.mesh.geometry` over lib-mesh |
| Node package | node types' cooks (`NodeType`) and object kinds (`ObjectKindSpec`); node descriptions and contexts elsewhere | Mesh objects, the Mesh nodes and the Mesh context |
| Bridge | conversions (`Conversion`) between two kernels' types | Tessellate: `kinogaki.cad.body` → `kinogaki.mesh.geometry` |
| Format | readers and writers (`FileFormat`) | OBJ, STEP, USD |
| Render | engines; they consume `RenderPrim`s, never kernel types | Spectre, Pixel |

The environment registers packages into a `Registry` in dependency order. The librarian resolves that order from the packages' manifests, so a kernel always comes before the packages that build on it. Node types, contexts and UI register through the librarian and the host UI interface (the host traits of `kinogaki-studio-ui-api`, to come), not through this crate. The two share the ids defined here.

## Ids

An id is written `<vendor>.<package>.<name>`, for example `kinogaki.mesh.extrude`. Each id is declared once in code and never derived from a Rust name. Project files, shelves and settings store ids, so renaming code changes nothing a user has saved.

Each kind of thing has its own id type:
- `DataTypeId`
- `ObjectKindId`
- `ContextId`
- `FormatId`
- `NodeTypeId`
- `SlotId`
- `ConversionId`
- `PackageId` (two segments only)

Every id is interned in one table and is a `Copy` pointer to its text. Packages are never unloaded, so the table never needs to free anything.

A package declares only ids under its own vendor. The registry refuses a package that does otherwise.

## Data

A `Data` holds one value, which is either on a wire or an object's result. It has two parts:
- the value's `DataTypeId`;
- the value itself, as an `Arc<dyn Any + Send + Sync>`.

Values are immutable and shared:
- Cloning a `Data` shares the same value.
- A change produces a new `Data`.
- Caches key on identity (`ptr_eq`, `identity`).

When identity is not enough, a cache keys on content instead, using `DataType::stamp`. This covers values that outlive an allocation, such as a value read back from a file.

A kernel reads its own values through a `Typed<T>` handle. The handle pairs the id with the Rust type, so `get` returns `None` for any other type. This works across packages because Studio and its plugins are built by one compiler and share one copy of each kernel crate, so a type's `TypeId` is the same everywhere.

The environment never looks inside a `Data`. It only calls the `DataType` registered for the value's id. A value whose type is not registered is kept and saved back unchanged, and nothing else happens to it.

## Data types and facets

Every data type answers a fixed set of questions:
- its label and wire color;
- the value's stamp, memory, bounds and info rows;
- its codec.

Facets are optional:

| Facet | Used by | Notes |
|---|---|---|
| `Displayable` | the viewport | levels, `prepare` → `Drawn` (a `viewport::Surface` plus type-owned detail), `highlight` → `Elements`, and `convert` |
| `Pickable` | viewport picking | the host finds surface elements (`Hit`s, box `Elements`); the facet turns them into `Components` and combines selections |
| `Tabular` | the spreadsheet | tables, columns, and cells read a range of rows at a time |
| `Renderable` | render sync | `RenderItem::Triangles` over a `TriangleSource`, placed once or many times |

If a type does not provide a facet, the surface that needs it simply leaves the type's values out.

`Components` is a component selection described in the data type's own terms. A mesh uses element indices. A CAD body uses persistent topology names. The environment stores, undoes and counts a selection, but only the data type that made it reads what is inside.

A `Conversion` is a declared function that turns one data type into another. The package that provides it depends on both kernels, so neither kernel needs to know about the other. The registry holds at most one conversion for each (from, to) pair.

## Cooking

Studio's core owns the cooking DAG for every context:
- evaluation order, dirty propagation and invalidation;
- the per-node result cache, its memory budget and reuse;
- latest-wins scheduling, cancellation, worker threads and wakes;
- time dependency and held edits.

A package owns only what a node computes. It implements `NodeType`:
- `cook(cx, inputs)`: the outputs (`Outputs`: one `Data` per output port, the selection output 0 carries downstream, warnings) or a `CookError` (cancelled, or failed with diagnostics);
- `reads()`: what besides inputs and parameters invalidates the node, as data (time, input names, selection). What a node doesn't declare, core withholds from its `CookCx`, so it can't depend on it by accident;
- `incremental()`, optionally: `prepare` returns the outputs and a `CookPlan`, and the plan's `update` produces outputs for new light parameters (a drag) until it returns `None`. This is lib-mesh's `cook_prepared` / `ToolPlan` as a capability. Plans live in core's cache, never in the project or undo.

`CookCx` gives a cook:
- parameters by name (`Parameters` of `params::Value`; typed reads fail with the parameter's name);
- which inputs are on which port (`port`), and input names when declared;
- the component selection the node acts on and the one input 0 carries;
- cancellation (`check`), time, and `parallel_for` / `parallel_map` on the host's shared workers (the `parallel::Workers` service, which receives each caller's location so the host can size runs per call site), so packages never start threads;
- a core-owned scratch slot per node (`scratch::<T>()`), for state such as a tessellation cache. Core may empty it at any time, so it may only make cooks faster, never change their results.

Packages keep no evaluator, worker or private cache.

## Held edits

Edit Mesh and Edit CAD hold an authored delta over their input. Core owns the mechanism: capturing the delta, applying it again when the input recooks, and undo. A package supplies `HeldEdit` for its data type: the delta's data type (so a delta saves, stamps and counts memory like any value) and `apply(base, delta)`.

This part is a sketch. How deltas are authored interactively belongs to `studio-ui-api`, which is still to be designed.

## Saving

A project is a tree of elements. Each element has a kind, typed fields (scalars and flat arrays), text metadata and named children. Each data type and each object kind writes and reads its own element through its codec, `DataCodec` or `ObjectCodec`: the environment creates the element and hands the codec an `ElementWriter` to fill, or an `ElementReader` to read. Packages never see the file format; Studio implements both traits over its project documents, and `element::MemoryElement` implements them in memory for package tests.

`DataCodec` receives the base value an edit was derived from. This lets a codec write only what changed: for example, a mesh whose points moved stores no topology.

A value held in several places (two frozen nodes keeping the same mesh) is written once with `ElementWriter::share` and read once with `read_shared`, which hands every place the same `Arc`.

Codecs follow the project rules:
- reading is strict: the typed reads fail naming the field and the element;
- there are no defaults for missing values;
- there are no version fields.

If a project uses a package that is not installed, it still opens. The elements that package wrote are kept and saved back exactly as they were.

## Objects

An `ObjectKindSpec` describes one kind of object. It gives:
- the label and icon;
- whether its objects are placed in the scene;
- the context of the network inside them;
- the data type of their result;
- a new body;
- a codec;
- `adopts`, which turns an imported value into a new body.

An `ObjectBody` is the part of an object that the package owns. The environment keeps the object's name, its place in the hierarchy, its placement and its flags. Bodies are immutable and are replaced whole, so undo keeps the previous body.

## Formats

A `FileFormat` declares:
- its label and extensions, and optionally `detects`, for a file recognized by its first bytes;
- the data types it produces;
- its options: defaults by name, a `params::Schema` for a `Purpose` (File ▸ Import's `Objects`, or a File node's `Node`, which leaves out options that shape objects), and how they are saved (`write_options`/`read_options`; by default one field per option).

It reads a file (`read(path, options, &Reading)`) into `Imported`, a list of `FileItem`s with parents listed before their children; an item with no data is a group, and each is placed relative to its parent and shown or hidden. A `Reading` carries the purpose, the project's `Units` (lengths convert into them), an origin a format may seed names with (a File node's id), project values the host lends by type, and a cancellation poll. For a File node a format reads one item. A format may also provide a `FormatWriter`, which writes items in given units.

To import, the environment passes each value to the first object kind that `adopts` it. With no formats installed, nothing can be imported.

## Session slots

Some state belongs to a package and lasts only for the session: a selection, picks, an open sketch. A package declares each such piece of state as a value type implementing `Slot` and registers it (`SessionSlot::of::<T>()`, `Package::with_slot`, `UiPackage::slots`):
- `id`, `initial`, and whether undo keeps it (`UNDOABLE`);
- `PLACE_BOUND`: another level starts it over (a selection belongs to the level it was made at);
- `restored`: after an undo, a redo or an edit, the value brought in line with where the user is (`SlotDoc`);
- `estimated_bytes` for the undo history's budget; values compare with `PartialEq`, so an unchanged value records no step.

`SessionSlots` holds every slot's value, shared and copy-on-write: a clone is a snapshot (one pointer a value), and changing a value copies it only while a snapshot still shares it. Studio keeps its undo history with these tables.

Session state is never saved in the project.

## Cancellation and time

Long work polls a `CancelToken`: preparing a display, converting, reading a file.

`Time` is the scene moment that cooks and render prims read: frame, seconds and fps.

## Rendering

Engines never see kernel types. A data type's `Renderable` facet returns `RenderItem::Triangles { part, source, placements, label }`:
- `part` is a key stable across recooks (a CAD body's index), so the host matches the items of successive values;
- `source` is a `TriangleSource`, an immutable snapshot with shared arrays;
- `placements` are relative to the object: the first is the item, the rest are copies sharing its acceleration structure;
- `label` is the name Cryptomatte gives the object.

The source contract (`TriangleSource`, `Ranges`, `Look`, `MeshAttribute`, `AttributeDomain`) is `kinogaki-render-api`'s, the render interface engines build on, and `render` re-exports it; so a source a data type makes is what a host hands its engines, with no conversion. `RenderItem` stays here.

The host keeps engine scenes in step cheaply: a source with the last identity is skipped; one with the same topology is a points update narrowed by `moved_since`; looks, coordinates and attributes are resent only when their identities change. Normals are asked for lazily, after a drag ends.

## The viewport

Studio's core draws and picks. A data type describes what to draw as a `viewport::Surface`: points, polygon corners (point, face, normal), triangles over the corners, line segments with edge ids and point markers with point ids. During a drag it returns a surface `derived` from the last one, with the points that moved, and the host patches only those. Picks come back as `Hit`s and box selections as `Elements`, in the ids the surface gave.

## Self-contained

The SDK depends on the standard library alone, but for `kinogaki-render-api`'s `wgpu` feature (the display contract), which brings wgpu 30.0.1 from crates.io. Every type above is the SDK's own, so a package compiles against nothing private and the SDK builds from a plain clone. Studio converts between these types and its own libraries' at the boundary (its maths, its property schemas, its project documents, its viewport and render libraries, its worker threads).


# The UI layer

`kinogaki-studio-ui-api` is how a plugin shows things: an inspector, a tool's card, viewport overlays, a settings page, heads-up value fields.

## The host draws

**Plugins never see Studio's UI library or egui.** Every type a plugin draws with is this crate's own: geometry, colors, the theme, fonts and icons by name, input, a painter and the widgets. Studio implements the `Ui` and `Painter` traits over its private UI library, and each call maps to exactly the call it replaces there. So the SDK stays self-contained, a plugin compiles against nothing private, and Studio can change how it draws without breaking a plugin.

The crate depends on `kinogaki-studio-api` (for parameter values, units and diagnostics) and the standard library, nothing else.

## What a plugin uses

From the most declarative to the most direct:

| Part | What it is | Used for |
|---|---|---|
| `sheet::Schema<K>` | Sections and typed fields with their values, as data; the host draws it with `Ui::sheet` and reports `Edits` | inspectors, a tool's card body, settings pages (`page() -> Schema`, then apply the `Edits`) |
| `CardSpec`, `PopoverSpec` | Floating panels the host frames around a body | a tool's parameters; a picker beside a handle |
| `ValueFields` | The heads-up boxes a drawing tool types into; `type_events` is the whole keyboard behaviour, `Ui::value_fields` draws them | a line's length and angle while drawing |
| `Ui` | Immediate widgets (labels, buttons, toggles, number fields, sliders, text fields, choices, swatches, menus) and layout | small panels the schema can't express |
| `Painter` | Segments, circles, rectangles, polylines, text and icons, single or in batches; text measuring; reserved slots | viewport overlays: handles, guides, badges, labels |
| `Input` | This frame's pointer, keys and typing, as plain data | tools reading the pointer and shortcuts |

A node's parameter schema (`params::Schema`) becomes a sheet with `From`, keyed by parameter name, so the host shows every node's parameters the same way.

Meshes, CAD surfaces and wireframes never go through the painter. They are the data types' display surfaces (the `Displayable` facet), drawn by the host's viewport; the painter is for the few hundred marks a tool puts over them.

## Rules

- **Immediate mode, on the UI thread.** A plugin calls its `Ui` every frame, inside the host's frame, and what it called is what shows. Nothing it draws persists. No call blocks; long work belongs on the host's workers.
- **Bodies run once.** Layout methods take their body as `&mut dyn FnMut(&mut dyn Ui)` so the traits stay object-safe. The host calls it exactly once before returning; menu bodies run only while the menu is open. `UiExt` wraps the layouts for closures that return a value (`horizontal_with`, `card_with` and so on).
- **Ids name what a widget is**, never where it is. Make them from stable keys with `Id::new` and `Id::with`. `UiExt::sheet_keyed` turns any hashable field key into a `FieldKey` by hashing it, so a row keeps its focus or open dropdown when rows above it come and go.
- **Colors come from the theme.** `Theme` is filled in by the host. Use the token that names the role (`surface`, `muted`, `accent`), and ink a fill with `Theme::ink_on`, so a plugin follows the host's theme. `Color` is premultiplied sRGBA bytes; its arithmetic matches the host's to the bit.
- **Icons are named**, from the host's icon set. An unknown name draws nothing.
- **Values are edited in place.** A widget writes what it edits into the `&mut` the plugin passed and reports `changed`. A sheet writes new values into its schema and reports `Edits`: the fields changed, the actions taken, and the gesture's start and finish, which one undo step spans.

## Viewport maths

Some pure maths the host's own tools use lives here, so a plugin's tools hit, drag and label exactly as Studio's do:
- `projection`: a view's `Projection` (clip space from the edited object's space and back, and the direction towards the viewer), filled in by the host from its camera for one view rectangle's aspect;
- `gizmo`: the transform gizmo's frame, handles, hit testing, and what a drag of a handle means (move along an axis or plane, scale factors, angle changes). The host paints the gizmo;
- `labels::point_labels`: where to put point numbers so a dense mesh shows an evenly spread, readable subset at a bounded cost.

## The host interface

A package's UI never sees the editor. Each call into it gets a context, `&mut dyn host::Cx`, reaching the host's capabilities, one trait each: `HostUndo` (gestures, undo, redo), `HostSlots` (typed through `HostSlotsExt`), `HostSelection`, `HostDoc` (where the user is, objects and their bodies), `HostView`, `HostStatus`, `HostShell`, `HostTransform` and `HostSettings` (typed through `HostSettingsExt`); plus the plain `HostPrefs`, the wake for the package's workers, and two services over the host's drawing: `GizmoService` (a view's transform gizmo) and `UnitsService` (numbers in the user's units). A view's parts get a `ViewCx` (the view as data: rectangle, response, pointer, modifiers, camera), the inspector an `InspectorCx`.

**A package** (`package::UiPackage`) is a state type, its contexts (`ContextPart<S>`: optional `Levels`, `ViewportLayer`, `Inspector`, `Vocabulary`, `NetworkView`, `ObjectRows`, `OutlineRows`, `Creations`), its session slots and its settings sections (`Section`, `SettingsSection`). The host keeps the state: `register` erases its type, and for each call the host takes the state out, lends it to the part with the call's context, and puts it back. Host methods never call back into a package; what would (an undo ending a live tool, an action) is queued until the call returns. Undo never holds a package's state: it restores the document and the slots.

Rules: every call runs on the UI thread and returns at once; a query gets `&S` and `&dyn Cx` and changes nothing; workers wake the host once per result and the host calls `UiPackage::poll`; no timers.

## Testing

`testing::FakeHost` stands in for Studio's host interface (slots with a real undo history, selections, a place, views and settings sections as plain fields, every request recorded as a `HostCall`), and `testing::Hosting` calls a registered package's parts on it as the host does.

`testing::RecordingUi` and `testing::RecordingPainter` are headless fakes: they lay widgets out simply, record every call and every shape, and play back scripted user actions (click a button, edit a sheet field, type into a field, pick a choice). A plugin's unit tests drive its panels with them, with no window and no GPU.
