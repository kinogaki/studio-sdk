# The headless API

`kinogaki-studio-api` is the part of the plugin API that has no UI. This page explains how its parts fit together and the rules packages follow. The rustdoc on each item is the reference.

## Environment and packages

Studio is the **environment**. With no packages installed, it is the editor, networks, undo, files, Part, Camera, an empty Material Library and a File node that can read nothing. Everything else is a **package**:

| Kind | Registers here | Example |
|---|---|---|
| Kernel | data types (`DataType`), how held edits apply to them (`HeldEdit`) and their session state (`SessionSlot`) | the mesh kernel adapter: `kinogaki.mesh.geometry` over lib-mesh |
| Node package | node types' cooks (`NodeType`) and object kinds (`ObjectKindSpec`); node descriptions and contexts elsewhere | Mesh objects, the Mesh nodes and the Mesh context |
| Bridge | conversions (`Conversion`) between two kernels' types | Tessellate: `kinogaki.cad.body` → `kinogaki.mesh.geometry` |
| Format | readers and writers (`FileFormat`) | OBJ, STEP, USD |
| Render | engines; they consume `RenderPrim`s, never kernel types | Spectre, Pixel |

The environment registers packages into a `Registry` in dependency order. The librarian resolves that order from the packages' manifests, so a kernel always comes before the packages that build on it. Node types, contexts and UI register through the librarian and the host UI interface (`studio-ui-api`, to come), not through this crate. The two share the ids defined here.

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
- its label and extensions;
- the data types it produces;
- its options: defaults by name, plus a `params::Schema` for the import dialog.

It reads a file into `Imported`, a list of `FileItem`s with parents listed before their children; an item with no data is a group. A format may also provide a `FormatWriter`.

To import, the environment passes each value to the first object kind that `adopts` it. With no formats installed, nothing can be imported.

## Session slots

Some state belongs to a package and lasts only for the session: a selection, picks, an open sketch. A package registers a `SessionSlot` for each such piece of state.

`SessionSlots` holds every slot's current value. The values are immutable and are replaced whole, so:
- `set` returns the previous value for the undo step;
- a clone of `SessionSlots` is a snapshot.

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

The source contract (`TriangleSource`, `Ranges`, `Look`, `MeshAttribute`, `AttributeDomain`) lives in `render` as a placeholder until the SDK's `render-api` crate lands; this crate then re-exports it from there. `RenderItem` stays here.

The host keeps engine scenes in step cheaply: a source with the last identity is skipped; one with the same topology is a points update narrowed by `moved_since`; looks, coordinates and attributes are resent only when their identities change. Normals are asked for lazily, after a drag ends.

## The viewport

Studio's core draws and picks. A data type describes what to draw as a `viewport::Surface`: points, polygon corners (point, face, normal), triangles over the corners, line segments with edge ids and point markers with point ids. During a drag it returns a surface `derived` from the last one, with the points that moved, and the host patches only those. Picks come back as `Hit`s and box selections as `Elements`, in the ids the surface gave.

## Self-contained

The SDK depends on the standard library alone. Every type above is the SDK's own, so a package compiles against nothing private and the SDK builds from a plain clone. Studio converts between these types and its own libraries' at the boundary (its maths, its property schemas, its project documents, its viewport and render libraries, its worker threads).

