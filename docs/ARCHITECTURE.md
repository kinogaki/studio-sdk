# Architecture

The SDK has two crates:
- `kinogaki-studio-api` (`crates/api`) is the headless API: everything a package registers and exchanges that has no UI.
- `kinogaki-studio-ui-api` (`crates/ui-api`, library `studio_ui_api`) is the UI layer: how a plugin's panels, cards, overlays and settings pages ask the host to draw. It is described in [The UI layer](#the-ui-layer) below.

Two leaf crates sit below them, so kernels can depend on a contract without depending on the API:
- `kinogaki-render-api` (`crates/render-api`) is the render interface between a host and its engines (see [Rendering](#rendering)).
- `kinogaki-viewport-api` (`crates/viewport-api`) is the viewport packet contract: what a data type hands the viewport to draw (see [The viewport](#the-viewport)).

This page explains how their parts fit together and the rules packages follow. The rustdoc on each item is the reference. To write a plugin step by step, start with [Write your first node plugin](FIRST-NODE-PLUGIN.md), then [Write your first context plugin](FIRST-CONTEXT-PLUGIN.md).

# The headless API

## Environment and packages

Studio is the **environment**. With no packages installed, it is the editor, networks, undo, files, Part, Camera, an empty Material Library and a File node that can read nothing. Everything else is a **package**:

| Kind | Registers here | Example |
|---|---|---|
| Kernel | data types (`DataType`), how held edits apply to them (`HeldEdit`) and their session state (`SessionSlot`) | Studio's Mesh plugin's kernel: `kinogaki.mesh.geometry` over its mesh library |
| Node package | node types (`NodeType`: description and cook), the network contexts they live in (`NetworkContext`) and object kinds (`ObjectKindSpec`); the contexts' UI through the UI layer | Mesh objects, the Mesh nodes and the Mesh context |
| Bridge | conversions (`Conversion`) between two kernels' types | Tessellate: `kinogaki.cad.body` → `kinogaki.mesh.geometry` |
| Format | readers and writers (`FileFormat`) | OBJ, STEP, USD |
| Render | engines (`Package::engines`, the `wgpu` feature); they consume `RenderPrim`s, never kernel types | Spectre, Pixel |

The environment registers packages into a `Registry` in dependency order. The loader resolves that order from the plugins' manifests, so a kernel always comes before the packages that build on it. Every package goes through the registry's checks (`Registry::check`, `Registry::register`): its id and every id it declares are new and under its vendor, each data type it names is registered, each compiled context has one compiler, no node type declares a parameter named as spares are (`cook::SPARE`), and its UI's contexts are its own. A package that fails is refused whole: Studio lists it as unavailable in Settings ▸ Plugins, says why in the Console, and keeps running.

A package with a UI carries it: `Package::ui` holds the parts the host UI interface (`kinogaki-studio-ui-api`) made, added with its `WithUi::with_ui`. The package's id, label and session slots are the `Package`'s alone.

## Plugin libraries

A plugin is a Rust dylib that exports its one `Package`, UI included, through `export_package!(make)`: the macro writes the entry Studio looks up (`entry::SYMBOL`, of type `entry::Entry`), catches a panic in `make`, and keeps `unsafe` out of the plugin's own code, so its crate keeps `unsafe_code = "forbid"`. Studio and its plugins share one standard library and one build of each shared crate, so values cross as plain Rust types and `TypeId`s agree.

A plugin is therefore built with the exact compiler this repository pins (`rust-toolchain.toml`) against the **plugin kit** of the Studio release it runs in: the build of the SDK that release ships, with its standard library and its kit id. The kit's README gives the build command. A plugin is developed and unit-tested with plain cargo against this repository, and built against the kit to ship.

Studio looks for plugins in its own plugin folder and the user's (macOS: `~/Library/Application Support/Kinogaki Studio/plugins`; Windows: `%APPDATA%\Kinogaki Studio\plugins`; Linux: `$XDG_CONFIG_HOME/kinogaki-studio/plugins`, else `~/.config/kinogaki-studio/plugins`), in one folder per kind (`kernels/`, `nodes/`, `formats/`, `renders/`). Each plugin is its library and a manifest beside it, `<name>.prisma`, naming its package id, kind, version, the kit it was built with and the packages it depends on. Studio reads the manifests first, refuses a plugin built for another kit or missing a dependency without opening it, loads the rest in dependency order, admits each package through the registry, and never unloads a plugin it loaded. Settings ▸ Plugins lists every plugin found, loaded or not, with the reason.

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

The environment never looks inside a `Data`. It only calls the `DataType` registered for the value's id. A value whose type is not registered is kept as it was read when a project is read, and written back unchanged; the Console says so (see Saving).

## Data types and facets

Every data type answers a fixed set of questions:
- its label and wire color;
- the value's stamp, memory, bounds and info rows;
- its codec.

Facets are optional:

| Facet | Used by | Notes |
|---|---|---|
| `Displayable` | the viewport | levels (`ElementKind`s), `prepare` → `Drawn` (pieces: packets with their placements, roles and how they draw while the object is open, plus type-owned detail) over the value shown before, `refine` once a value rests, `highlight` → `ElementSet`, `convert`, the status bar's counts (`tally` → `Tally`s) and a selection's box for the pivot (`bounds_of`) |
| `Pickable` | viewport picking | the host finds packet elements (`Hit`s, box `ElementSet`s); the facet turns them into `Components` and combines selections |
| `ObjectPicking` (`picking`) | the object levels | how an object holding the value is hit by a `WorldRay` and framed; without it core frames the value by its `bounds` and doesn't pick it |
| `Look` (`look`) | shading | the pieces' colors while shaded and whether the value renders as a volume; without it the value shows as clay |
| `Snappable` (`snapping`) | gizmo moves | what a move may land on in the value, for the user's `SnapTargets` (points, edges' middles, faces' centres, edges, surfaces): a `SnapQuery` answered with `SnapHit`s; the host keeps the rest of snapping (the master switch, the grid, ranking across objects, feedback) |
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

A package owns only what a node is and computes. It implements `NodeType`:
- `description()`: its label, its ports (`Port`: key, label, data type, multi, optional, why it can't cook unwired) and its parameters as a `params::Schema` at their defaults. The environment reads it once, when the package registers;
- the description's `handles`: viewport handles bound to its parameters, as Houdini's handle bindings. A `Handle` names its kind (`Translate`, `Rotate`, `Orient` and `Scale` on Studio's transform gizmo, shown while the matching transform tool is active; `Pivot`, `Distance`, `Radius`, `Direction`, `Box` and `Sphere` as dots and rings on their shape), the parameters it reads and writes (a `Vector` is one three-number parameter or three `Scalar`s; a `Scalar` is a number parameter, one number of a vector parameter, or fixed), its space (object or world), an optional placement `Frame` (Position, Up and Turn) its places are in, an anchor (which point of a shape its center names: a box built on its base), a constraint (one axis or one plane), a range for the numbers it writes, and the parameter values it shows for. Studio draws, hit-tests, snaps and drags them while the node is selected, writes the parameters as the pointer moves (the node recooks, incrementally where it can) and records one undo step a drag. A tool that needs more than a binding draws through the UI's `GizmoService` instead;
- `cook(cx, inputs)`: the outputs (`Outputs`: one `Data` per output port, the selection output 0 carries downstream, warnings) or a `CookError` (cancelled, or failed with diagnostics). An output nothing reads (`cx.wants(k)` is false) needn't be made;
- `reads(parameters)`: what besides inputs and parameters invalidates a node with those parameters, as data (time, input names, selection). What a node doesn't declare, core withholds from its `CookCx`, so it can't depend on it by accident;
- `incremental()`, optionally: `prepare` returns the outputs and a `CookPlan`, and the plan's `update` produces outputs for new light parameters (a drag) until it returns `None`. Plans live in core's cache, never in the project or undo.

`CookCx` gives a cook:
- parameters by name (`Parameters` of `params::Value`; typed reads fail with the parameter's name);
- which inputs are on which port (`port`), and input names when declared;
- the component selection the node acts on and the one input 0 carries;
- cancellation (`check`), time, and `parallel_for` / `parallel_map` on the host's shared workers (the `parallel::Workers` service, which receives each caller's location so the host can size runs per call site), so packages never start threads;
- a core-owned scratch slot per node (`scratch::<T>()`), for state such as a tessellation cache. Core may empty it at any time, so it may only make cooks faster, never change their results.

Packages keep no evaluator, worker or private cache.

## Held edits

Edit Mesh and Edit CAD hold an authored delta over their input. Core owns the mechanism: capturing the input once as the base, keeping one result per held state that its copies (undo snapshots) share, applying the delta anew when it changes, and undo. A package supplies `HeldEdit` for its deltas: the registry keys it by the deltas' data type, which a held node's type names (`NodeType::held`), so one package declares how one type of delta applies. It gives the base data type, the delta's data type (so a delta saves, stamps and counts memory like any value), which other inputs a delta reads (`reads_input`: what it projects onto), and `apply(cx, base, delta, inputs)`, which returns the node's outputs and may keep prepared work in the node's scratch slot. How deltas are authored interactively is the package's UI, through `studio-ui-api`.

## Networks and contexts

A **context** is one `ContextId` naming a kind of editing (Mesh, Comp, a package's own). It has two halves, each declared by the package that owns the context:
- **its networks**, as data: a `NetworkContext`, registered with `Package::with_context`. It says how the context's networks evaluate (`Evaluation::Cook`, node by node, or `Compile`, below), which core flags their nodes offer (`FlagSet`: Bypass, Template, Freeze), whether deleting a node heals the chain, how the network pane draws their nodes (`NodeLayout`), and the element they are stored under (`NetworkStorage`);
- **its UI**: a `ContextPart` of the package's `UiPackage` (see [The UI layer](#the-ui-layer)): what Tab, keys, menus and the viewport offer while the user works in it, its selection levels, its inspector.

A headless package declares only the first half: its context's nodes are then drawn, edited and inspected by Studio's own UI.

An **object kind** names the context its objects open into (`ObjectKindSpec::context`). Core keeps one network of that context beside each such object's body: it creates, edits, validates, cooks, undoes and saves it, as Houdini's host owns its node graphs, and the body never holds it. The object's result is what its network yields (`NetworkRead::yielded`): its `NodeRole::Output` node, else the node its display flag is on.

**Node types say where they are offered.** `Description::context` names each context whose networks offer a type, and `Description::listed` says where Tab, the menus and the shelves list it there: a section ("Create"), a `Category` that colors its tile, a family within the section and search aliases. A type without a listing is still added by code and by files, never by Tab.

**One node shape.** Every node of every context is its type, its `Parameters`, its input wires, the core flags and, optionally, a held delta or the outputs it keeps while frozen. Core stores a network as one child element of its object's element, spelt as `NetworkStorage::element` says, with an element per node: its type, name and position, its flags, its wires, its parameters in one layout every type shares, and its delta and kept outputs through their data types' codecs. Packages never write networks themselves.

**Reading and editing.** A package reads a network through `NetworkRead` (its nodes by the host's `NodeRef`s, each node's `NodeInfo`, the displayed and the yielded node) and edits it through `NetworkEdit`: add a node (`NewNode`, placed by `Place::At`, `After` a node or `Grow`n from what the level displays), rename, move, set parameters, connect and disconnect, set flags and the display, remove, hold a delta, freeze. Each call refuses what doesn't fit (an unknown node, a port the node hasn't got, a value its parameter doesn't take, a flag the context doesn't offer) and changes nothing then. New parameters go through the type's `created` and edited ones through its `edited`, on every path (Tab, the inspector, `NetworkEdit`), so all paths agree. A package's UI reaches an object's network through `HostNetwork` (`network`, `edit_network`); each `edit_network` is one undo step, checked whole when it commits (no cycles, required values) and dropped when that fails.

## Compiled contexts

A context declared `compiled()` (Comp's, shading's) isn't cooked node by node. Its node types are ordinary `NodeType`s for their descriptions, ports and rows, but core never calls their `cook`. Core flattens the network into a `Flat` (subnets resolved, nodes in cook order) and hands it, with the output to show, to the context's `NetworkCompiler`. The compiler returns a `Compiled`: the package's own plan, opaque to the host, and the render outputs the plan reads (`CompileCx::outputs` lists those the render writes). Core keeps the plan until the network's topology or a flattened node's parameters change.

The package that declares a compiled context registers its compiler (`Package::with_compiler`, exactly one per compiled context: the registry refuses a compiled context without one, and a compiler or runner for any other context). With the `wgpu` feature it also brings the plan's runner (`Package::runners`: a `PlanRunner` makes a `Runner` on the host's device, which runs plans on the GPU). A runner that shows display values asks the host's `ViewTransforms` for the view's transform as WGSL, so its images go through the same view transform as the viewport's.

## Saving

A project is a tree of elements. Each element has a kind, typed fields (scalars and flat arrays), text metadata and named children. Each data type and each object kind writes and reads its own element through its codec, `DataCodec` or `ObjectCodec`: the environment creates the element and hands the codec an `ElementWriter` to fill, or an `ElementReader` to read. Packages never see the file format; Studio implements both traits over its project documents, and `element::MemoryElement` implements them in memory for package tests.

`DataCodec` receives the base value an edit was derived from. This lets a codec write only what changed: for example, a mesh whose points moved stores no topology.

A value held in several places (two frozen nodes keeping the same mesh) is written once with `ElementWriter::share` and read once with `read_shared`, which hands every place the same `Arc`.

Codecs follow the project rules:
- reading is strict: the typed reads fail naming the field and the element;
- there are no defaults for missing values;
- there are no version fields.

`ObjectCodec::read` also receives a `ReadCx`, a read-only view of what is installed (the render engines' descriptions, `engines()`). A body that needs something no installed package provides, such as a light of a type no installed engine declares, reads as `Error::NotInstalled` with a plain message saying what is missing.

If a project uses a package that is not installed, it still opens, and nothing of it is lost. Everything no installed package reads is kept as it was read:
- the nodes of that package's types: they show grey with their type's name, don't cook (nothing after them cooks), and their parameters are read-only, though they can be moved, wired around and deleted;
- the objects of its kinds, and the bodies that read as `NotInstalled`: they show grey, and can be renamed, moved in the tree and deleted;
- its stored values, and values of its data types.

The Console says once per missing package what it kept ("Couldn't load 3 nodes and 1 object: acme.trees isn't installed"). Saving writes all of it back unchanged, so once the package is installed the project opens as it was.

## Objects

An `ObjectKindSpec` describes one kind of object. It gives:
- the label and icon;
- whether its objects are placed in the scene;
- the context of the network inside them (core keeps that network beside each object's body, and creates, edits, cooks, undoes and saves it; the body never holds it);
- the data type of their result;
- a new body;
- a codec;
- `adopts`, which turns an imported value into a new body.

An `ObjectBody` is the part of an object that the package owns: the kind's own state (a Mesh object's display settings, a Render Settings object's engine and settings). The environment keeps the object's name, its place in the hierarchy, its placement, its flags and its network. Bodies are immutable and are replaced whole, so undo keeps the previous body.

A kind whose objects take part in a render themselves says how through two optional facets, so the host's render sync and its Render mode and Render Image never read the body:
- `renders()`, a `RenderedKind`: what an object is, as `Declared` prims of types engines declare with every value it holds (`declared`), and the body holding an edit of them (`with_declared`). The host keeps, for the active engine, the values that engine's type and applied schemas declare, places the prims at the object, decodes the standard ones (UsdLux lights) for the viewport and exports, and draws the inspector rows from the types' schemas.
- `at_root()`: its objects live only at the scene's root (Fog, Render Settings).
- `render_setup()`, a `RenderSetupKind`: the render an object sets up (Render Settings), as a `RenderSetup`: the engine, the image size, the camera (an `ObjectRef`), the output path and outputs, the engine's settings and what the render shows (`Processing`: a Comp network's displayed node, one node, or the raw render). The host renders with the active one, resolving `Processing` against the Comp network it keeps beside the object. `with_setup` takes an edit back (the inspector's rows, a new object's engine and its defaults) as a new body.

The render-domain package owns these kinds; engines only declare the types and settings they render (`EngineDescription`), so an object survives a change of engine.

## Formats

A `FileFormat` declares:
- its label and extensions, and optionally `detects`, for a file recognized by its first bytes;
- the data types it produces;
- its options: defaults by name, a `params::Schema` for a `Purpose` (File ▸ Import's `Objects`, or a File node's `Node`, which leaves out options that shape objects), and how they are saved (`write_options`/`read_options`; by default one field per option).

It reads a file (`read(path, options, &Reading)`) into `Imported`, a list of `FileItem`s with parents listed before their children; an item with no data is a group, and each is placed relative to its parent and shown or hidden. A `Reading` carries the purpose, the project's `Units` (lengths convert into them), an origin a format may seed names with (a File node's id), project values the host lends by type, and a cancellation poll. For a File node a format reads one item. A format may also provide a `FormatWriter`, which writes items in given units.

`format::options` holds what formats share about options: reading a typed option (`option`, `choice`), saving a choice by its own token so a saved setting survives a reordered list (`choice_value`, `read_token`), and the rows most import dialogs show (`section`, `units`, `scale`, `toggle`). A file's materials travel beside its geometry as `render::preview_materials()`, a list of `ImportedMaterial` (the key its polygons name, a library name, a `PreviewSurface`), which File ▸ Import turns into a Material Library.

To import, the environment passes each value to the first object kind that `adopts` it. With no formats installed, nothing can be imported.

## Session slots

Some state belongs to a package and lasts only for the session: a selection, picks, an open sketch. A package declares each such piece of state as a value type implementing `Slot` and registers it (`SessionSlot::of::<T>()`, `Package::with_slot`):
- `id`, `initial`, and whether undo keeps it (`UNDOABLE`);
- `PLACE_BOUND`: another level starts it over (a selection belongs to the level it was made at);
- `restored`: after an undo, a redo or an edit, the value brought in line with where the user is (`SlotDoc`);
- `estimated_bytes` for the undo history's budget; values compare with `PartialEq`, so an unchanged value records no step.

`SessionSlots` holds every slot's value, shared and copy-on-write: a clone is a snapshot (one pointer a value), and changing a value copies it only while a snapshot still shares it. Studio keeps its undo history with these tables.

Session state is never saved in the project.

## Stored values

Some values belong to a package and to the project rather than to an object: CAD's tolerances and display quality for the project, its rows for each view. A package declares each as a value type implementing `Stored` and registers it (`StoredValue::of::<T>()`, `Package::with_stored`):
- `id`, the name of its element in the project;
- `scope`: one value for the project (`Scope::Project`) or one per view (`Scope::View`);
- `defaults`, what a new project or view starts with;
- `write` and `read`, its codec; reading is strict, as codecs' is.

The host keeps them in `StoredValues` tables (the project's, and one per view), shared and copy-on-write like session slots, and saves each into the project under an element naming its package, so a project holding a value of a package that isn't installed skips it. A package's UI reads and sets them through `HostDoc` (typed through `HostDocExt::stored`); a project value's change is a change to the project, saved with it, while a view's isn't undone. A cook reads the project's values when its node declares `Reads::project` (`CookCx`'s `Project`).

## Icons

Studio owns a named icon set, and its icons are the default: wherever the API asks for an icon by name (`ObjectKindSpec::icon`, `Description::icon`, the UI layer's buttons, painter icons and library entries), a plain name such as `"cube"` is Studio's. A package adds icons of its own with `Package::with_icon`: an `Icon` is a name within the package and an SVG document the package embeds. Others name it by the package's id, a `/` and that name (`icon::qualified`): icon `leaf` of `acme.trees` is `"acme.trees/leaf"`. A package registers icons under its own id alone, so packages' names never meet each other's or Studio's.

Studio draws a package's icon exactly as it draws its own: as pixel art, its `viewBox` its size in icon pixels (whole numbers, at most `icon::MAX_PIXELS` either way, the size of Studio's own), every pixel solid or clear and tinted by the widget that shows it, rasterized once per size and cached. So an icon is one color, drawn in white or `currentColor`.

Studio checks each icon when the package registers: its name, its size (at most `icon::MAX_BYTES`), that its document reads and that its `viewBox` fits. An icon that fails is left out and the Console says why, once; the rest of the package registers as usual. A name that finds no icon draws Studio's neutral icon, and the Console names it once.

Icons mark what has no text of its own: toolbar buttons, tab buttons, the kinds of objects and nodes in the outliner. Menus and marking menus show text alone, and a node tile shows its category by color.

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

Studio's core draws and picks. What it draws is a packet behind `kinogaki-viewport-api`'s `SurfaceSource` trait: shared point positions, vertex shading records, triangles, line segments in line classes and point markers in point classes, each class with its own look and ids, and the ranges that changed since the packet it was derived from. A kernel that needs full control (smooth and authored normals, curvature, hulls and control vertices, see-through faces, live transform previews) makes its own packets and reads them back as their own type with `packet_as`. The contract's maths is plain arrays: vectors `[f64; 3]`, quaternions `[x, y, z, w]` and matrices `[[f64; 4]; 4]`, column by column.

The simpler path is a `viewport::Surface`, a `SurfaceSource` itself: points, polygon corners (point, face, normal), triangles over the corners, line segments with edge ids and point markers with point ids. During a drag a type returns a surface `derived` from the last one, with the points that moved, and the host patches only those. Picks come back as `Hit`s and box selections as `ElementSet`s, in the ids the packet gave.

`Displayable::prepare` returns a `Drawn`: the value's pieces in drawing order. A piece is a packet, where it sits in its object (none, or several placements sharing one upload), its role (the object's own surface, or a templated node's faint result the open context marks) and how it draws while its object is open (left to the open context as its edited surface, drawn as anywhere, or shaded beside it). `prepare` receives what the type made for the value shown before, so a type whose values change a little at a time builds on it, and `refine` offers a finer display once the value rests.

## Self-contained

The SDK depends on the standard library alone, but for two optional features: `kinogaki-render-api`'s `wgpu` (the display contract), which brings wgpu 30.0.1, and `kinogaki-viewport-api`'s `bytemuck` (byte casts of the records a renderer uploads), which brings bytemuck 1.25.2, both from crates.io. `kinogaki-studio-api`'s `wgpu` feature turns the first on for packages' render engines (`Package::engines`), and is off by default. Every type above is the SDK's own, so a package compiles against nothing private and the SDK builds from a plain clone. Studio converts between these types and its own libraries' at the boundary (its maths, its property schemas, its project documents, its viewport and render libraries, its worker threads).


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
| `CardSpec`, `PopoverSpec` | Floating panels the host frames around a body (a popover as the host's raised popup or a flat panel) | a tool's parameters; a picker beside a handle; a list at the pointer |
| `ValueFields` | The heads-up boxes a drawing tool types into; `type_events` is the whole keyboard behaviour, `Ui::value_fields` draws them | a line's length and angle while drawing |
| `Ui` | Immediate widgets (labels, buttons, toggles, number fields, sliders, text fields, choices, swatches, menus) and layout | small panels the schema can't express |
| `Painter` | Segments, circles, rectangles, polylines, text and icons, single or in batches; text measuring; reserved slots | viewport overlays: handles, guides, badges, labels |
| `Input` | This frame's pointer, keys and typing, as plain data | tools reading the pointer and shortcuts |

A node's parameter schema (`params::Schema`) becomes a sheet with `From`, keyed by parameter name, so the host shows every node's parameters the same way.

A node type shapes its rows from what its node reads (`NodeType::schema` with a `SchemaCx`: which ports are wired, what each input held at the last cook, the names of the nodes behind each port's wires). It hides rows a wire or a choice makes moot, fills a row's `menu` (an attribute row's names on the input; an integer row choosing a wire, the names behind the wires), greys `unavailable` choices, marks a faulty value with its `problem`, names a code row's `language`, and adds `Button` rows. A button holds no value: pressing it asks `NodeType::pressed`, and core applies the answer as one edit on the inspector's own path, so `NodeType::edited` answers it too. A choice shows as segments while it has three entries or fewer, 18 characters together at most and none unavailable (`params::segments`), else as a menu. A held node's delta shows its own rows (`HeldEdit::rows`, over the node's held result), written back through `HeldEdit::apply_rows` and `HeldEdit::pressed`.

Every kind of value has a sheet kind, drawn by Studio's own widget for it, like Houdini's parameter templates: numbers, vectors, colors (with an emission's swatch, `Trailing::emission`), linked RGBA channels, choices, toggles, text, code, secrets, file paths, free paths in the scene's tree, references (`Kind::Reference(RefFilter)`: the field says what fits, such as objects of some kinds, results of some data types or Materials, and Studio fills the searchable picker, takes drops from the outliner and networks, and resolves the path from the sheet's holder), float and color ramps (`kinogaki_render_api::ramp`, re-exported as `params::ramp` and `sheet::ramp`, is their one encoding and maths: nodes, Studio's ramp widget and render engines evaluate them alike; a ramp row may fix one interpolation for every key), tone curves (`sheet::tone_curve` encodes them), palettes and a material input's connection button (`Socket`). A file path's folder button opens the host's file dialog as its `FileDialog` says, and the chosen file arrives as an edit of the field. Raw `Ui` widgets are for custom panels only.

Meshes, CAD surfaces and wireframes never go through the painter. They are the data types' display surfaces (the `Displayable` facet), drawn by the host's viewport; the painter is for the few hundred marks a tool puts over them.

## Rules

- **Immediate mode, on the UI thread.** A plugin calls its `Ui` every frame, inside the host's frame, and what it called is what shows. Nothing it draws persists. No call blocks; long work belongs on the host's workers.
- **Bodies run once.** Layout methods take their body as `&mut dyn FnMut(&mut dyn Ui)` so the traits stay object-safe. The host calls it exactly once before returning; menu bodies run only while the menu is open. `UiExt` wraps the layouts for closures that return a value (`horizontal_with`, `card_with` and so on).
- **Ids name what a widget is**, never where it is. Make them from stable keys with `Id::new` and `Id::with`. `UiExt::sheet_keyed` turns any hashable field key into a `FieldKey` by hashing it, so a row keeps its focus or open dropdown when rows above it come and go.
- **Colors come from the theme.** `Theme` is filled in by the host. Use the token that names the role (`surface`, `muted`, `accent`), and ink a fill with `Theme::ink_on`, so a plugin follows the host's theme. `Color` is premultiplied sRGBA bytes; its arithmetic matches the host's to the bit.
- **Icons are named.** A plain name (`"cube"`) is one of Studio's icons, the default set; a package adds icons of its own (`Package::with_icon`), named by its id, a `/` and the icon's name (`"acme.trees/leaf"`). An unknown name draws Studio's neutral icon, and the Console says so once.
- **Values are edited in place.** A widget writes what it edits into the `&mut` the plugin passed and reports `changed`. A sheet writes new values into its schema and reports `Edits`: the fields changed, the actions taken, and the gesture's start and finish, which one undo step spans.

## Viewport maths

Some pure maths the host's own tools use lives here, so a plugin's tools hit, drag and label exactly as Studio's do:
- `projection`: a view's `Projection` (clip space from the edited object's space and back, and the direction towards the viewer), filled in by the host from its camera for one view rectangle's aspect;
- `gizmo`: the transform gizmo's frame, handles, hit testing, and what a drag of a handle means (move along an axis or plane, scale factors, angle changes). The host paints the gizmo;
- `labels::point_labels`: where to put point numbers so a dense mesh shows an evenly spread, readable subset at a bounded cost.

## The host interface

A package's UI never sees the editor. Each call into it gets a context, `&mut dyn host::Cx`, reaching the host's capabilities, one trait each: `HostUndo` (gestures, undo, redo), `HostSlots` (typed through `HostSlotsExt`), `HostSelection`, `HostDoc` (where the user is, objects and their bodies, stored values), `HostNetwork` (objects' networks, read and edited), `HostView`, `HostStatus`, `HostShell`, `HostTransform` and `HostSettings` (typed through `HostSettingsExt`); plus the plain `HostPrefs`, the wake for the package's workers, and two services over the host's drawing: `GizmoService` (a view's transform gizmo) and `UnitsService` (numbers in the user's units). A view's parts get a `ViewCx` (the view as data: rectangle, response, pointer, modifiers, camera), the inspector an `InspectorCx`.

**A package's UI** (`package::UiPackage`) is a state type, its contexts' UI halves (`ContextPart<S>`, under the context's id: a `Library` and optional `Levels`, `ViewportLayer`, `Inspector`, `Vocabulary`, `ObjectRows`, `OutlineRows`, `Results`; what an object level creates is each object kind's own, `ObjectKindSpec::creations`; the network half is the context's `NetworkContext`, see [Networks and contexts](#networks-and-contexts)) and its settings sections (`Section`, `SettingsSection`); its id, label and slots are its `Package`'s. `Package::with_ui` (`package::WithUi`) adds it to the package and erases the state's type, and the host reads it back with `Registered::of`. A context whose networks are core's needs no UI half: the host gives it its network pane, Tab and menus listing the context's listed node types (with Display, Bypass and Collapse into Subnet), and its node inspector, and a `ContextPart` adds to these or replaces them (its library's items first and what it sets winning, its vocabulary running its own items, its inspector replacing the node sheet). The host keeps the state, and for each call the host takes the state out, lends it to the part with the call's context, and puts it back. Host methods never call back into a package; what would (an undo ending a live tool, an action) is queued until the call returns. Undo never holds a package's state: it restores the document and the slots.

**The library.** What a context offers the host's menus is data, read once when the package registers: its items (each tool, command or node by its stored name, with its label, family, search words, Tab section, the levels it applies at and which surfaces offer it), its key store and default bindings, its A/S/D marking menus and shelves (the user's to edit, kept under the menus' store name), its right-click menus and menu bar sections (each for a level, a selection or none, inside a held edit or outside one), how Tab lists its items (tools over the views while it edits, else nodes; a Suggested category) and the viewport chrome it takes (the shared Shading rows, its own analysis modes, the snap steps, the starting pivot). A context may share another's keys or menus (`Shared::Of`). The host builds every one of those surfaces from it. An entry that can't apply is left out, never greyed: its `When` (level, selection, held edit) and the vocabulary's `shown` decide; `label` renames an item by its state ("Repeat Bevel"). The vocabulary runs the items.

**Select All.** Edit ▸ Select All runs core's Select All in whichever context is open, and each context answers it its own way. A package's context answers it by holding a library item named `command:select_all` (core's id for Select All) and running that name in its `Vocabulary`, as an item invoked from the views (`Invoked::Views`). The item can also sit on the context's own menus. A context without that item, or without a vocabulary, doesn't answer, and Edit ▸ Select All is unavailable while it is open.

**Results.** A context's `Results` part is how it takes the host's cooks of its objects. The host cooks every object and keeps, reports and publishes what comes back; a context keeps only its gestures' part. While one of its gestures stands on what the open object shows (a drag previewing over the shown packet), `hold` keeps the next answer waiting; `takes` may refuse to show a gesture's earlier state or a refinement for now; `shown` hears whenever what the open object shows changes (a `Shown` change), so the context drops or resumes its gestures; and `wants` names nodes to cook besides what objects show, whose results come back through `cooked`.

**The edited surface.** Core renders every view. While a context's object is open, its `ViewportLayer::edited` returns, per view and frame, an `Edited`: the packet being edited, its selection, a live preview, overlay and guide layers, face normals, the soft-selection tint, the line highlight, colors and scalars, the hover, which of its parts draw (`SurfaceLook`), the view's own lines and the sketch-like grid and section planes. Everything in it is an `Arc` of the packet contract's types, so core borrows it into its renderer's scene without a copy and the renderer uploads only what changed allocation. `template_mark` says how the open object's templated pieces are marked. A context's own objects and line layers (`ContextView`, from `layers`) are packets and `SceneLines` too, its objects shaded or see-through (`ghost`).

Rules: every call runs on the UI thread and returns at once; a query gets `&S` and `&dyn Cx` and changes nothing; workers wake the host once per result and the host calls `UiPackage::poll`; no timers.

## Testing

`testing::FakeHost` stands in for Studio's host interface (slots and objects' networks with a real undo history, selections, a place, views and settings sections as plain fields, every request recorded as a `HostCall`), and `testing::Hosting` calls a registered package's parts on it as the host does.

`testing::FakeNetwork` is one object's network in memory, made from the packages whose node types it holds. It answers `NetworkRead` and `NetworkEdit` as core does, refusing what core refuses, and cooks a node through its type (`cook`). Put into `FakeHost::networks`, it is what a package's tools edit through `HostNetwork`, one undo step an edit; so a tool's network edits, and a node type in a network, are tested without Studio. It is a test double, not core: it caches nothing and cooks every node anew, and its rustdoc lists where it is simpler than core.

`testing::RecordingUi` and `testing::RecordingPainter` are headless fakes: they lay widgets out simply, record every call and every shape, and play back scripted user actions (click a button, edit a sheet field, type into a field, pick a choice). A plugin's unit tests drive its panels with them, with no window and no GPU.
