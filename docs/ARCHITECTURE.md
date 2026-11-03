# Architecture

The SDK has two crates:
- `kinogaki-studio-api` (`crates/api`) is the headless API: everything a package registers and exchanges that has no UI.
- `kinogaki-studio-ui-api` (`crates/ui-api`, library `studio_ui_api`) is the UI layer: how a plugin's panels, cards, overlays and settings pages ask the host to draw. It is described in [The UI layer](#the-ui-layer) below.

Leaf crates sit below them, so kernels can depend on a contract without depending on the API:
- `kinogaki-value` (`crates/value`) is the value model: declared ids, `Data`, and the one `Value` and `ValueKind` every node parameter, format option, render setting, declared prim value and material port holds (see [Values](#values)). It depends on nothing.
- `kinogaki-geometry` (`crates/geometry`) holds the standard geometry values (see [Standard data types](#standard-data-types)).
- `kinogaki-edit` (`crates/edit`, depending on `kinogaki-geometry` alone) holds the edit contracts core and the kernels share: selection rules and modes, deformations, a drag's moving points (see [Edit contracts](#edit-contracts)). A kernel (polygons, CAD) depends on `kinogaki-geometry` and `kinogaki-edit` only.
- `kinogaki-render-api` (`crates/render-api`, depending on `kinogaki-value` and `kinogaki-geometry`) is the render interface between a host and its engines (see [Rendering](#rendering)).
- `kinogaki-viewport-api` (`crates/viewport-api`) is the viewport packet contract: what a data type hands the viewport to draw (see [The viewport](#the-viewport)).

This page explains how their parts fit together and the rules packages follow. The rustdoc on each item is the reference. To write a plugin step by step, start with [Write your first node plugin](FIRST-NODE-PLUGIN.md), then [Write your first context plugin](FIRST-CONTEXT-PLUGIN.md).

# The headless API

## Values

Every value the SDK describes is a `Value` of a `ValueKind` (`kinogaki-value`, re-exported as `kinogaki_studio_api::value` and through `params`): a node's parameters and a format's options (`Parameters`, values by name, described by a `params::Schema` of `Parameter`s), an engine's settings and its declared prims' values (`SettingDescriptor`s: a name, a kind, a default, when it shows and applies as `Condition`s), a material node's settings and its sockets' unwired values (`ParamSpec`, `SocketSpec`), an engine's presets. One model, so a host draws, checks, keys over time and saves every one of them the same way, and an engine's setting is a parameter with no conversion (`Parameter::setting`).

- **Exact.** Numbers are `f64` and whole numbers `i64` (`Value::Integer`) wherever they are held, authored or sent; an engine narrows what it keeps.
- **Choices by key.** A `Value::Choice` holds its `Choice::key`; files save the key, so relabelling or reordering entries never changes a value.
- **Typed structures.** A ramp is a `Ramp` of keys (any number; an engine's capacity is its kind's `most`, and a longer ramp is refused, never cut short), tone curves are `ToneCurve`s, a color is `Value::Color` (linear: the working space as authored, ACEScg as an engine receives it).
- **Units are owned.** `Unit::Suffix` holds its text (`" EV"`, `" K"`); the host shows lengths in the project's unit.
- **Files and textures are two things.** An image is authored as its file's path (`ValueKind::File`, a `Value::Text`); an engine receives the Texture prim the host decoded it to, in the declared prim's or material node's `textures`, never the path.
- **Kinds carry their options** (a choice's entries and unavailable keys, an attribute row's names, a code row's language), checked by `ValueKind::check`, which core runs on every value it is handed. How a value is presented (segments or a menu) is the UI layer's.

## Environment and packages

Studio is the **environment**. With no packages installed, it is the editor, networks, undo, files, Part, Camera, an empty Material Library and a File node that can read nothing. Everything else is a **package**:

| Kind | Registers here | Example |
|---|---|---|
| Kernel | data types (`DataType`), how held edits apply to them (`HeldEdit`) and their session state (`SessionSlot`) | Studio's Mesh plugin's kernel: `kinogaki.mesh.geometry` over its mesh library |
| Node package | node types (`NodeType`: description and cook), the network contexts they live in (`NetworkContext`) and object kinds (`ObjectKindSpec`); the contexts' UI through the UI layer | Mesh objects, the Mesh nodes and the Mesh context |
| Format | readers and writers (`FileFormat`) | OBJ, STEP, USD |
| Render | engines (`Package::engines`, their descriptions, and `Package::makers`, the `wgpu` feature); they consume `RenderPrim`s, never kernel types | Spectre, Pixel |

The environment registers packages into a `Registry` in dependency order. The loader resolves that order from the plugins' manifests, so a kernel always comes before the packages that build on it. Every package goes through the registry's checks (`Registry::check`, `Registry::register`): its id and every id it declares (its stored values' and render engines' too) are new and under its vendor; each data type it names (its ports' among them) and each context it names (where its node types are offered, what its object kinds hold) is its own or an earlier package's; each compiled context has one compiler; no node type declares a parameter named as spares are (`cook::SPARE`); each of its contexts adopts a node type at most once; its icons are well formed and named once, and each package icon it shows is one it or an earlier package carries; and its UI's contexts are its own. A package that fails is refused whole: Studio lists it as unavailable in Settings ▸ Plugins, says why in the Console, and keeps running.

A package with a UI carries it: `Package::ui` holds the parts the host UI interface (`kinogaki-studio-ui-api`) made, added with its `WithUi::with_ui`. The package's id, label and session slots are the `Package`'s alone.

## Plugin libraries

A plugin is a Rust dylib that exports its one `Package`, UI included, through `export_package!(make)`: the macro writes the entry Studio looks up (`entry::SYMBOL`, of type `entry::Entry`), catches a panic in `make`, and keeps `unsafe` out of the plugin's own code, so its crate keeps `unsafe_code = "forbid"`. Studio and its plugins share one standard library and one build of each shared crate, so values cross as plain Rust types and `TypeId`s agree.

A plugin is therefore built with the exact compiler this repository pins (`rust-toolchain.toml`) against the **plugin kit** of the Studio release it runs in: the build of the SDK that release ships, with its standard library. The plugin's root names `kinogaki_shared::link!()` once, which links the kit's shared library and records the build of it the plugin is compiled against. The kit's README gives the build command. A plugin is developed and unit-tested with plain cargo against this repository, and built against the kit to ship.

Studio looks for plugins in its own plugin folder and the user's (macOS: `~/Library/Application Support/Kinogaki Studio/plugins`; Windows: `%APPDATA%\Kinogaki Studio\plugins`; Linux: `$XDG_CONFIG_HOME/kinogaki-studio/plugins`, else `~/.config/kinogaki-studio/plugins`), in one folder per kind (`kernels/`, `nodes/`, `formats/`, `renders/`). Each plugin is its library and a manifest beside it, `<name>.prisma`, naming its package id, kind, version and the packages it depends on. Studio reads the manifests first, refuses a plugin missing a dependency without opening it, opens the rest in dependency order, refuses one whose library records another build of the shared library than the one Studio runs with before calling anything in it, admits each package through the registry, and never unloads a plugin it loaded. Settings ▸ Plugins lists every plugin found, loaded or not, with the reason.

## Ids

An id is written `<vendor>.<package>.<name>`, for example `kinogaki.mesh.extrude`. Each id is declared once in code and never derived from a Rust name. Project files, shelves and settings store ids, so renaming code changes nothing a user has saved.

Each kind of thing has its own id type:
- `DataTypeId`
- `ObjectKindId`
- `ContextId`
- `FormatId`
- `NodeTypeId`
- `SlotId`
- `StoredId`
- `MesherId`
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

## Standard data types

Studio defines each kind of geometry once, in the `kinogaki-geometry` crate (re-exported as `kinogaki_studio_api::geometry` and `kinogaki_render_api::geometry`), and every package works on the same values. Core registers them as data types (`kinogaki.core.geometry` and `standard::BREP`; read and wrapped through `standard::geometry()` and `standard::brep()`), and a camera's lens beside them (`standard::CAMERA`, `standard::camera()`: render-api's `Camera`, unplaced, in UsdGeomCamera's units, made by `standard::lens`, which checks it; what core's Camera objects hold and formats read and write), so they exist with no package installed, and core displays, picks, tabulates, saves and renders them. Kernels are operations over them: a kernel never registers a type of its own for the same data, and several kernels may offer operations over one type with no conversion between them.

| Value | What |
|---|---|
| `Geometry` | polygons, polylines and curves over shared points: positions, an `Arc<Primitives>` (one row table: each primitive's `Kind`, a `Basis` and closed, in any order; `CurveData` with degrees, knots and weights when a curve is there; its canonical `Index`, a cheap `edge_count`, `polygons()` and fingerprint) and `Attributes`. `Geometry::curve` reads a primitive as a `Curve`: its domain, its `Span`s and the one exact evaluation (`point`, `on` a span) every display, engine and kernel shares; how finely a view samples it is the viewport's. One NURBS knot rule serves its curves and the B-rep's (clamped as a parameter) |
| `brep::Brep` | boundary-representation bodies (`brep::Body`: shared `brep::Geometry`, `BrepTopology`, persistent `Names` and `Attributes`, placements and the positional tolerance it was built to), free curves (a `Geometry` of curves and polylines, no polygons) and datums; its faces, edges and vertices are numbered as one across bodies, free curves and datums (`Brep::count`, `Brep::element`, a datum plane a face, an axis or curve an edge, a datum point a vertex), and `Brep::fingerprint` is what a selection of it keeps (its bodies' structure and persistent names alone, never geometry, so a resize keeps a selection and a face added or split makes it stale); made by `BrepBuilder`, which checks structure and consistency at ends and joins. The module's rustdoc is the contract two kernels agree on: charts, senses, loops, seams, poles, tolerances. A kernel's mesher turns a body into `Geometry`; core doesn't tessellate |
| `Attributes` | named, typed columns (`Attribute` of an `AttributeType`: `Storage` F32, F64, I32, I64 or indexed `Text` × width 1–255 × `Meaning` None, Point, Vector, Normal, Color, TextureCoord, Rotation, Transform, optionally an array per element) and groups (`Elements`) per `Domain` (Detail, Object, Instance, Primitive, Edge, Point, Vertex); `Attribute::tuples` adopts a `Vec<[f32; 3]>` without copying and `as_tuples` views it typed; one fixed table of standard names (`P`, `N`, `uv`, `Cd`, `Alpha`, `v`, `id` (unique within its domain), `pscale`, `scale`, `orient`, `up`, `rest`, `width`, `material`, `name`, `part`, the `sharp`/`smooth` Edge groups and the `.hidden` Primitive group) fixes each one's type and domains |
| `Elements`, `Selection`, `Incidence` | `Elements` is the one set of a domain's elements (every one, a short id list or bits; equal by members), what groups and selections hold. A `Selection` is `Elements` of one domain, the active one, the fingerprint of the value it was made on (`Selection::validate` refuses it on another) and, when that value has stable ids, the members' ids, so `kinogaki_edit::select::reresolve` finds it again after a change upstream; a standard type's `Components` hold one (`Components::selection`). `Incidence` is a value's structure and identity as selection rules read it: elements per domain, which touch which, the fingerprint, the stable ids; `Geometry` and `Brep` implement it |
| `Detail` | how fine geometry is made for a view or a render: `Viewport(DisplayQuality)` or `Render { chord, angle }` (see Rendering) |
| `DisplayQuality` | how finely an object whose display is tessellated shows: `Coarse`, `Medium`, `Fine`, `Finest`, which a producer maps to presets of its own; the project's (a project value core keeps), unless the object overrides it |
| `Tolerances` | the project's modeling tolerances (positional, fit, tangent, curvature, with their ranges, and the model unit), a project value core keeps |

**Validated values.** Every value's fields are private; it is made only through a constructor or builder that checks its rules (each type's rustdoc lists them), and refused with an `Error` naming the element at fault (the SDK's one error: an `ErrorKind` a caller branches on and the `Diagnostic` the user reads, so a kernel's operations and a node's cook return the same `Error` and pass it on with `?`). So code reading a value relies on its rules: offsets in range, points that exist, attributes sized to their domain. Enums are `#[non_exhaustive]`, and each says what a reader does with a variant it doesn't know. The product limits (`limits`: 2³² − 2 points, primitives and vertices, so every id fits a `u32` with `u32::MAX` free as "no element"; 256-byte names; finite positions within 1e100) hold for every value; memory is the budget's to bound.

**Shared and immutable.** Arrays are `Column`s: shared buffers that adopt a `Vec` without copying it. Edits make new values sharing whatever they didn't touch: `Geometry::with_positions` keeps the primitive table and attributes, `Geometry::with_topology` reuses another value's checked topology and checks only the new columns, `Attributes::with` shares every other attribute. An attribute checks its own values once, when it is made, and a value's `with_attributes` checks only the entries it doesn't already hold, so replacing one column checks that column alone. A drag of 1M points re-checks positions and nothing else. Text is an index per element into a table of distinct strings; an empty string is no value (`Text::NONE`), and a renderer uses the index as a slot.

**Caches.** A kernel's acceleration data is a cache derived from a standard value, never the value:
1. A cache is keyed by the identity of the shared part it was derived from: the `Primitives` table for adjacency, edges, half-edges and triangulation; the positions `Column` for BVHs and bounds; an attribute's `Column` for anything built from it.
2. It lives in that part's typed slot (`Primitives::derived`, `Column::derived`): as long as the part, shared by every value sharing the part, so a points-only edit keeps every topology cache. Never in a global map keyed by address.
3. Only the crate that defines a cache's type reads it; a slot is never a channel between plugins.
4. A cache is never saved, compared or treated as the value.
5. `key()` and `ptr_eq` hold within a session while the part lives; the fingerprint or a stamp is for anything longer.
6. Two kernels' caches over one part are two slots.

The topology's `Index` (edges in canonical order, polygons and polylines per edge and per point, each primitive's edges) is the one standard cache: every kernel and core share its numbering, and Edge attributes follow it.

**Saving.** The SDK defines how the standard types save: `standard::GeometryCodec`, `standard::BrepCodec`, `standard::CameraCodec` and `standard::LightCodec` are the codecs core registers (core's Camera and Light objects save their lens and light through them too; a light value is unplaced and names a dome's image by its file: the Texture prim a host decodes it to is one scene's, as an Image Texture node's is), writing each topology once per save for every value sharing it and reading every rule again. Projects save only frozen and held results; everything else is cooked again on load.

## Edit contracts

Core owns the geometric values; kernels bring the operations and the maths; plugins bring tools and nodes. What core and a kernel must agree on when a value is selected and changed is `kinogaki-edit` (re-exported as `kinogaki_studio_api::edit`), so `kinogaki-geometry` stays pure values:

| Module | What |
|---|---|
| `select` | `convert` (the standard rule: whole into Primitive and Object and from Point into Edge, any otherwise) and `convert_by` a `Coverage` (`Any`, `Whole`) turn a selection into another domain over any `Incidence` (polygons and B-rep alike); `resolve` spells it out; `reresolve` finds a selection on a later value of the one it was made on (itself while the numbering holds, by its members' stable ids after a change upstream, `None` without ids). A context's level declares a `Mode`: the domain it selects and how a pick `Expand`s there (`None`, or `Connected`: the pieces it touches, joined through shared points, a non-blank `name` kept together); `expand` grows a selection so. Mesh's Pieces level is `Mode { domain: Primitive, expand: Connected }`, a selection mode, not a domain |
| `deform` | a move as data: a `Deform` is a `Move` (translation, then rotation and scale about a pivot, along its axes), each moved point's weight (a soft move) and a `Symmetry` (a mirror plane on an `Axis`, its primary `Side`). `Deform::place` puts one point where the move takes it at its weight; `apply` places a run of points. The viewport's drag preview and the kernel's commit place points with the same function, so the preview is the commit |
| `ids` | stable ids, one rule for every kernel: kept elements keep their `id`, new or copied ones take `fresh` ids above every id the value holds, and `merged` gives a Merge's inputs' ids (the first input's kept, later collisions and missing ones fresh) |
| `preview` | `Moving`: the points a drag moves (resolved once when it begins) and its `Deform`, which a context hands as `Edited::moving` and core previews over the shown value with no cook |

**Conformance.** The conformance suite (the companion crate `kinogaki-studio-api-testing`, as the UI layer's fakes are `kinogaki-studio-ui-api-testing`: test support, only ever a dev-dependency) runs every producer's values through every consumer: a kernel implements `Producer` in its own repository, an engine or a format `Consumer`, and each runs beside the SDK's `Fixtures` (every shape and attribute kind), `Codecs` (a bit-exact round trip) and `Invariants` (rebuilt through the validating constructors, canonical edges).

## Data types and facets

Every data type answers a fixed set of questions:
- its label and wire color;
- the value's stamp, memory, bounds and info rows;
- its codec;
- optionally its empty value (`empty`: what a bypassed node gives) and whether a value holds nothing (`is_empty`: the menus' "Create geometry first"). Hosts ask `is_empty` of shown values every frame, so it reads the value's counts; a stamp hashes the whole value and belongs to cooks.

Facets are optional:

| Facet | Used by | Notes |
|---|---|---|
| `Displayable` | the viewport | levels (`ElementKind`s), `prepare` → `Drawn` (pieces: packets with their placements, roles and how they draw while the object is open, plus type-owned detail) over the value shown before, `refine` once a value rests, `highlight` → `ElementSet`, `convert`, the status bar's counts (`tally` → `Tally`s) and a selection's box for the pivot (`bounds_of`) |
| `Pickable` | viewport picking | the host finds packet elements (`Hit`s, box `ElementSet`s); the facet turns them into `Components` and combines selections |
| `ObjectPicking` (`picking`) | the object levels | how an object holding the value is hit by a `Ray` (at its world `Xform`) and framed; without it core frames the value by its `bounds` and doesn't pick it |
| `Look` (`look`) | shading | the pieces' colors while shaded and whether the value renders as a volume; without it the value shows as clay |
| `Snappable` (`snapping`) | gizmo moves | what a move may land on in the value, for the user's `SnapTargets` (a set of `SnapKind`s: points, edges' middles, faces' centers, crossings, edges, surfaces): a `SnapQuery` answered with `SnapHit`s, a surface's with its normal there; the host keeps the rest of snapping (the master switch, the grid, ranking across objects, feedback) |
| `Tabular` | the spreadsheet | tables, columns, and cells read a range of rows at a time |
| `Renderable` | render sync | `RenderItem::Shape` over a `ShapeSource` (triangles or curves), or `RenderItem::Instances` of other items |

If a type does not provide a facet, the surface that needs it simply leaves the type's values out.

`Components` is a component selection described in the data type's own terms. A mesh uses element indices. A CAD body uses persistent topology names. The environment stores, undoes and counts a selection, but only the data type that made it reads what is inside.

A `Mesher` (`Package::with_mesher`) is a kernel's display mesh of a B-rep body (`takes` names the data type it meshes, the B-rep's, which the host shows at a display quality while it is installed): its faces' polygons in face order (`face`), then one polyline per edge over the same shared points (`edge`), at a `Detail` (in a view, `Viewport` of the object's display quality, one mesher preset a step) and the project's `Tolerances`, with the faces it couldn't mesh and why. The B-rep value holds no triangles; core caches each body's mesh, and draws, picks, renders and exports from it. `Registry::meshers` lists them in registration order; which one meshes is the host's choice (a user setting), never a vendor's. With none installed a body shows grey, without faces.

## Cooking

Studio's core owns the cooking DAG for every context:
- evaluation order, dirty propagation and invalidation;
- the per-node result cache, its memory budget and reuse;
- latest-wins scheduling, cancellation, worker threads and wakes;
- time dependency and held edits.

A package owns only what a node is and computes. It implements `NodeType`:
- `description()`: its label, its ports (`Port`: key, label, data type, multi, optional, why it can't cook unwired) and its parameters as a `params::Schema` at their defaults. The environment reads it once, when the package registers;
- the description's `handles`: viewport handles bound to its parameters, as Houdini's handle bindings. A `Handle` names its kind (`Translate`, `Rotate`, `Orient` and `Scale` on Studio's transform gizmo, shown while the matching transform tool is active; `Pivot`, `Distance`, `Radius`, `Direction`, `Angle`, `Turn`, `Box` and `Sphere` as dots and rings on their shape), the parameters it reads and writes (a `Vector` is one three-number parameter or three `Scalar`s; a `Scalar` is a number parameter, one number of a vector parameter, or fixed), its space (object or world), an optional placement `Frame` (Position, Up and Turn) its places are in, an anchor (which point of a shape its center names: a box built on its base), a constraint (one axis or one plane), a range for the numbers it writes, and the parameter values it shows for. Studio draws, hit-tests, snaps and drags them while the node is selected, writes the parameters as the pointer moves (the node recooks, incrementally where it can) and records one undo step a drag (inside a live tool's gesture, the drag is part of it). A handle whose place is on what the cook made (an extrusion's arrow on its profile, a revolve's ring about its axis) comes from the cook: `Outputs::handles` holds handles with fixed places and numbers bound by name, which Studio shows after the declared ones while the node is active and runs the same way; one that names a parameter the node type doesn't have, or one of another kind, is left out with a warning on the node (`Outputs::check_handles`). `NodeInfo::handles` reads them back while the node's stamp holds. A tool that needs more than a binding draws its own handles;
- `cook(cx, inputs)`: the outputs (`Outputs`: one `Data` per output port, the selection output 0 carries downstream, warnings) or an `Error` (cancelled, or failed with its diagnostic). An output nothing reads (`cx.wants(k)` is false) needn't be made;
- `reads(parameters)`: what besides inputs and parameters invalidates a node with those parameters, as data (time, input names, selection). What a node doesn't declare, core withholds from its `CookCx`, so it can't depend on it by accident;
- `incremental()`, optionally: `prepare` returns the outputs and a `CookPlan`, and the plan's `update` produces outputs for new light parameters (a drag) until it returns `None`. Plans live in core's cache, never in the project or undo.

`CookCx` gives a cook:
- parameters by name (`Parameters` of `params::Value`; typed reads, `CookCx::number`, `integer`, `toggle`, `choice` (its key), `text`, fail with the parameter's name);
- which inputs are on which port (`port`), and input names when declared;
- the component selection the node acts on and the one input 0 carries;
- cancellation (`check`) and time; parallel work goes through `geometry::parallel` (`chunks_mut`, `map_blocks`) onto the `Workers` the host installs once per process, which receive each caller's location so the host can size runs per call site, so packages never start threads;
- a core-owned scratch slot per node (`scratch::<T>()`), for state such as a tessellation cache. Core may empty it at any time, so it may only make cooks faster, never change their results.

Packages keep no evaluator, worker or private cache.

## Held edits

Edit Mesh and Edit CAD hold an authored delta over their input. Core owns the mechanism: capturing the input once as the base, keeping one result per held state that its copies (undo snapshots) share, applying the delta anew when it changes, and undo. A package supplies `HeldEdit` for its deltas: the registry keys it by the deltas' data type, which a held node's type names (`NodeType::held`), so one package declares how one type of delta applies. It gives the base data type, the delta's data type (so a delta saves, stamps and counts memory like any value), which other inputs a delta reads (`reads_input`: what it projects onto), and `apply(cx, base, delta, inputs)`, which returns the node's outputs and may keep prepared work in the node's scratch slot. How deltas are authored interactively is the package's UI, through `studio-ui-api`.

## Networks and contexts

A **context** is one `ContextId` naming a kind of editing (Mesh, Comp, a package's own). It has two halves, each declared by the package that owns the context:
- **its networks**, as data: a `NetworkContext`, registered with `Package::with_context`. It says how the context's networks evaluate (`Evaluation::Cook`, node by node, or `Compile`, below), which core flags their nodes offer (`Flags`: Bypass, Template, Frozen), whether its networks are chains, where deleting a node heals the chain, or libraries of results (`NetworkKind`), how the network pane draws their nodes (`NodeLayout`), the element they are stored under (`NetworkStorage`), and other packages' node types they offer too, each under a section of the context's (`adopts`: CAM's networks take Mesh's Import; a node of an adopted type that isn't installed loads as a placeholder);
- **its UI**: a `ContextPart` of the package's `UiPackage` (see [The UI layer](#the-ui-layer)): what Tab, keys, menus and the viewport offer while the user works in it, its selection levels, its inspector.

A headless package declares only the first half: its context's nodes are then drawn, edited and inspected by Studio's own UI.

An **object kind** names the context its objects open into (`ObjectKindSpec::context`). Core keeps one network of that context beside each such object's body: it creates, edits, validates, cooks, undoes and saves it, as Houdini's host owns its node graphs, and the body never holds it. The object's result is what its network yields (`NetworkRead::yielded`): its `NodeRole::Output` node, else the node its display flag is on.

**Node types say where they are offered.** `Description::context` names each context whose networks offer a type, and `Description::listed` says where Tab, the menus and the shelves list it there: a section ("Create"), a `Category` that colors its tile, a family within the section and search aliases. A type without a listing is still added by code and by files, never by Tab.

**Data parameters.** A node whose authored state is a structured record (a sketch's curves and constraints, a stroke) holds it as one parameter of a registered data type: `ValueKind::Data(DataTypeId)` holding `Value::Data`, declared with `Parameter::data` at the type's empty value. Values compare by type and allocation (an edit always makes a new `Arc`), by the type's `stamp` where content counts (across a save and reopen). The inspector shows no row for it unless the node's schema adds its own (a summary, buttons); its node's tools set it through `set_parameter`, once per gesture (one undo step), and it is never keyed. Core saves it through the type's codec as a `data` child of the node's element, includes its stamp in the node's cook key and counts its memory through the type. A node whose data type isn't installed is kept as read and saved back verbatim.

**One node shape.** Every node of every context is its type, its `Parameters`, its input wires, the core flags and, optionally, a held delta or the outputs it keeps while frozen. Core stores a network as the `network` child of its object's element, typed as `NetworkStorage::element` says, with an element per node: its type, name and position, its flags, its wires, its parameters in one layout every type shares, and its delta and kept outputs through their data types' codecs. Packages never write networks themselves.

**Reading and editing.** A package reads a network through `NetworkRead` (its nodes by the host's `NodeRef`s, each node's `NodeInfo`, the displayed and the yielded node) and edits it through `NetworkEdit`: add a node (`NewNode`, placed by `Place::At`, `After` a node or `Grow`n from what the level displays), rename, move, set parameters, connect and disconnect, set flags and the display, remove, hold a delta, freeze. Each call refuses what doesn't fit (an unknown node, a port the node hasn't got, a value its parameter doesn't take, a flag the context doesn't offer) and changes nothing then. New parameters go through the type's `created` and edited ones through its `edited`, on every path (Tab, the inspector, `NetworkEdit`), so all paths agree. A package's UI reaches an object's network through `HostNetwork` (`network`, `edit_network`); each `edit_network` is one undo step, checked whole when it commits (no cycles, required values) and dropped when that fails.

## Compiled contexts

A context declared `compiled()` (Comp's, shading's) isn't cooked node by node. Its node types are ordinary `NodeType`s for their descriptions, ports and rows, but core never calls their `cook`. Core flattens the network into a `Flat` (subnets resolved, nodes in cook order) and hands it, with the output to show, to the context's `NetworkCompiler`. The compiler returns a `Compiled`: the package's own plan, opaque to the host, and the render outputs the plan reads (`CompileCx::outputs` lists those the render writes). Core keeps the plan until the network's topology or a flattened node's parameters change.

A compiler sees the installed node types through `CompileCx::node_type`. It reads what a type declares (its description, ports and spares) through the trait first. `NodeType` is `Any`, so the compiler may upcast a type to `&dyn Any` and downcast it to its own concrete type, but only for what that type alone knows (a Comp operator's lib-composit type, a shader's engine vocabulary).

Shader node types (an engine's material vocabulary as the nodes of a Material's Shading network) are the render package's, not the SDK's: the SDK carries the engine's vocabulary (`kinogaki_render_api::vocabulary`) and the Shading context's id, and Phase 3's engine-neutral material DAG takes over the rest.

The package that declares a compiled context registers its compiler (`Package::with_compiler`, exactly one per compiled context: the registry refuses a compiled context without one, and a compiler or runner for any other context). With the `wgpu` feature it also brings the plan's runner (`Package::runners`: a `PlanRunner` (`network::run`, the `wgpu` feature) makes a `Runner` on the host's device, which runs plans on the GPU). A runner that shows display values asks the host's color service for the view's transform as WGSL, so its images go through the same view transform as the viewport's.

**Color.** The host owns color management and lends it as one `color::ColorService`: the project's working space, conversions between spaces named as its configuration names them (`convert`, `to_working`, `working_to`), view transforms as WGSL, and a revision that moves whenever any of them would. Scene colors are linear in the project's working space and engines take and output linear ACEScg (render-api's `RENDER_SPACE`); formats convert a file's colors through `Reading::colors` and `Writing::colors`, cooks through the project they read (`CookCx::colors`), and the host's render target converts the working space to `RENDER_SPACE`. No package carries color maths of its own; `conformance::Unconverted` stands in for checks that convert nothing.

## Saving

A project is a tree of elements. Each element has a kind, typed fields (scalars and flat arrays), text metadata and named children. Each data type and each object kind writes and reads its own element through its codec, `DataCodec` or `ObjectCodec`: the environment creates the element and hands the codec an `ElementWriter` to fill, or an `ElementReader` to read. Packages never see the file format; Studio implements both traits over its project documents, and `element::MemoryElement` implements them in memory for package tests.

`DataCodec` receives the base value an edit was derived from. This lets a codec write only what changed: for example, a mesh whose points moved stores no topology.

A value held in several places (two frozen nodes keeping the same mesh) is written once with `ElementWriter::share` and read once with `read_shared`, which hands every place the same `Arc`.

Codecs follow the project rules:
- reading is strict both ways: the typed reads fail naming the field and the element, and Studio refuses a field, metadata entry or child a codec's read leaves unread, so a codec writes only what it reads back;
- the element is the codec's alone: the host keeps its own fields elsewhere (an object's placement on the object's element, its body in a `body` child);
- there are no defaults for missing values;
- there are no version fields.

`ObjectCodec::read` also receives a `ReadCx`, a read-only view of what is installed (the render engines' descriptions, `engines()`). A body that needs something no installed package provides, such as a light of a type no installed engine declares, reads as an `ErrorKind::Missing` error with a plain message saying what is missing.

If a project uses a package that is not installed, it still opens, and nothing of it is lost. Everything no installed package reads is kept as it was read:
- the nodes of that package's types: they show grey with their type's name, don't cook (nothing after them cooks), and their parameters are read-only, though they can be moved, wired around and deleted;
- the objects of its kinds, and the bodies that read as `Missing`: they show grey, and can be renamed, moved in the tree and deleted;
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
- `adopted` and `adopts` (none by default): the data types it can be made from, and what an object made from an imported value starts as (an `Adoption`: its body, its network's content as a `NetworkSeed` when the kind builds it, and the references to the file's keys that now resolve inside it);
- for a kind that holds no network, `value(body, path)`: what a reference naming the object reads (a printer's description). The path is the object's own, so a rename, like a body change, makes core read it again and recook its readers;
- `materials(body)`: Materials the object publishes (a printer's slots), each a `PublishedMaterial` under the object's path by its key (`/X1C/AMS 1 · 2`; a key is any `is_path_step`, spaces and `·` included), read-only, with a `PreviewSurface` and, while it can't be used, why (`unavailable`: a path naming it then names nothing). Core asks again whenever the body changes, and pickers and renders take them as they take a Material Library's;
- `creations` and `create`, the ways an object level makes its objects, given a `CreateCx`: the installed engines, the active one, and the user's stored values (`CreateCx::user`: a printer saved in Settings becomes a creation).

An `ObjectBody` is the part of an object that the package owns: the kind's own state (a Mesh object's display settings, a Render Settings object's engine and settings). The environment keeps the object's name, its place in the hierarchy, its placement, its flags and its network. Bodies are immutable and are replaced whole, so undo keeps the previous body.

A kind whose objects take part in a render themselves says how through two optional facets, so the host's render sync and its Render mode and Render Image never read the body:
- `renders()`, a `RenderedKind`: what an object is, as `Declared` prims of types engines declare with every value it holds (`declared`), and the body holding an edit of them (`with_declared`). The host keeps, for the active engine, the values that engine's type and applied schemas declare, places the prims at the object, decodes the standard ones (UsdLux lights) for the viewport and exports, and draws the inspector rows from the types' schemas.
- `at_root()`: its objects live only at the scene's root (Fog, Render Settings).
- `render_setup()`, a `RenderSetupKind`: the render an object sets up (Render Settings), as a `RenderSetup`: the engine, the image size, the camera (an `ObjectRef`), the output path and outputs, the engine's settings and what the render shows (`Processing`: a Comp network's displayed node, one node, or the raw render). The host renders with the active one, resolving `Processing` against the Comp network it keeps beside the object. `with_setup` takes an edit back (the inspector's rows, a new object's engine and its defaults) as a new body.

The render-domain package owns these kinds; engines only declare the types and settings they render (`EngineDescription`), so an object survives a change of engine.

## Formats

A `FileFormat` declares:
- its label and extensions, and optionally `detects`, for a file recognized by its first bytes;
- the data types it produces (none for a format that only writes, such as G-code);
- its options: defaults by name, a `params::Schema` for a `Purpose` (File ▸ Import's `Objects`, or a File node's `Node`, which leaves out options that shape objects), and how they are saved (`write_options`/`read_options`; by default one field per option).

It reads a file (`read(path, options, &Reading)`) into `Imported`, a list of `FileItem`s with parents listed before their children; an item with no data is a group, and each is placed relative to its parent and shown or hidden. A `Reading` carries the purpose, the project's `Units` (lengths convert into them), an origin a format may seed names with (a File node's id), project values the host lends by type, a `CancelToken` long reads poll, and for a File node the data type it wants (`wants`: a STEP file read as B-rep or as polygons). For a File node a format reads one item. A format may also provide a `FormatWriter`, which writes items (`write(path, items, &Writing, options)`, returning its warnings: what was written with loss or skipped) with options of its own: defaults by name (`FormatWriter::options`, none by default) and an Export dialog (`FormatWriter::schema`). Export hands a writer its own options, never the format's reading ones. A `Writing` carries the project's `Units`, a `CancelToken` long writes (and their meshing) poll, `meshed` (a value as polygons through the installed mesher, for a writer that writes only those) and, for a writer whose `frames()` says it writes ranges, the range as `Frames` (frames a second, how many, the items at each).

`format::options` holds what formats share about options: reading a typed option (`option`; a choice is held and saved by its key, so a saved setting survives a reordered list), and the rows most import dialogs show (`scale`, `toggle`). A file's materials travel beside its geometry as `standard::preview_materials()` (`kinogaki.core.preview_materials`, which core registers with `PreviewMaterialsCodec`), a list of `ImportedMaterial` (the key its polygons name, a library name, a `PreviewSurface`); File ▸ Import is to turn it into a Material Library (P90b).

To import, the environment passes each value to the first object kind that `adopts` it, lending it the engines (a Material Library adopts a file's materials as Materials, each empty until the render package fills it). Formats are offered the types kinds declare as `adopted`, not their results: what an object can be made from and what it gives are separate. With no formats installed, nothing can be imported.

## Session slots

Some state belongs to a package and lasts only for the session: a selection, picks, an open sketch. A package declares each such piece of state as a value type implementing `Slot` and registers it (`SessionSlot::of::<T>()`, `Package::with_slot`):
- `id`, `initial`, and whether undo keeps it (`UNDOABLE`);
- `PLACE_BOUND`: another level starts it over (a selection belongs to the level it was made at);
- `restored`: after an undo, a redo or an edit, the value brought in line with where the user is (`SlotDoc`);
- `estimated_bytes` for the undo history's budget; values compare with `PartialEq`, so an unchanged value records no step.

`SessionSlots` holds every slot's value, shared and copy-on-write: a clone is a snapshot (one pointer a value), and changing a value copies it only while a snapshot still shares it. Studio keeps its undo history with these tables.

Session state is never saved in the project.

## Stored values

Some values belong to a package and to the project rather than to an object: CAD's tolerances for the project, its rows for each view; and some to the user, across projects, read by cooks: CAM's printers and filaments. A package declares each as a value type implementing `Stored` and registers it (`StoredValue::of::<T>()`, `Package::with_stored`):
- `id`, a `StoredId` (`kinogaki.cad.tolerances`) naming its element in the project, under the package's vendor and declared once among the installed packages (the registry refuses a second);
- `scope`: one value for the project (`Scope::Project`), one per view (`Scope::View`), or one for the user (`Scope::User`, kept in the user's settings folder, never in a project);
- `defaults`, what a new project or view starts with;
- `write` and `read`, its codec; reading is strict, as codecs' is.

The host keeps them in `StoredValues` tables (the project's, and one per view), shared and copy-on-write like session slots, and saves each into the project under an element naming its package, so a project holding a value of a package that isn't installed skips it. Core keeps one view value of its own in every view's table: what the view draws, its display popover's state with the per-view display options the user set there (see "The display popover and display options"). A package's UI reads and sets them through `HostDoc` (typed through `HostDocExt::stored`); a project value's change is a change to the project, saved with it, while a view's isn't undone and a user's is saved with the user's settings. A cook reads the project's values and the user's when its node declares `Reads::project` (`CookCx`'s `Project`: `stored` and `user`); a change to either recooks it.

## Icons

Studio owns a named icon set, and its icons are the default: wherever the API asks for an icon by name (`ObjectKindSpec::icon`, `Description::icon`, the UI layer's buttons, painter icons and library entries), a plain name such as `"cube"` is Studio's. A package adds icons of its own with `Package::with_icon`: an `Icon` is a name within the package and an SVG document the package embeds. Others name it by the package's id, a `/` and that name: icon `leaf` of `acme.trees` is `"acme.trees/leaf"`. A package registers icons under its own id alone, so packages' names never meet each other's or Studio's.

Studio draws a package's icon exactly as it draws its own: as pixel art, its `viewBox` its size in icon pixels (whole numbers, at most `icon::MAX_PIXELS` either way, the size of Studio's own), every pixel solid or clear and tinted by the widget that shows it, rasterized once per size and cached. So an icon is one color, drawn in white or `currentColor`.

Studio checks each icon when the package registers: its name, its size (at most `icon::MAX_BYTES`), that its document reads and that its `viewBox` fits. An icon that fails is left out and the Console says why, once; the rest of the package registers as usual. A name that finds no icon draws Studio's neutral icon, and the Console names it once.

Icons mark what has no text of its own: toolbar buttons, tab buttons, the kinds of objects and nodes in the outliner. Menus and marking menus show text alone, and a node tile shows its category by color.

## Cancellation and time

Long work polls a `CancelToken`: preparing a display, converting, reading a file.

`Time` is the scene moment that cooks and render prims read: a whole number of ticks (1/705,600,000 s, so frames at the common rates are exact); `Rate` is frames a second, checked finite and above zero, and converts between the two. A value across a frame's shutter is `Sampled` (one sample holds still): render prims' placements are `Sampled<Xform>`.

## Rendering

Engines never see kernel types. A data type's `Renderable` facet is asked through a `RenderCx` (the scene time, a `Detail` of `Viewport(DisplayQuality)`, the object's, or `Render { chord, angle }` that says how fine to make geometry such as a CAD body's tessellation, a standard value that render-api re-exports, the project's `Tolerances` an approximate value is made to, and a cancel token), and returns `RenderItem`s:
- `RenderItem::Shape { part, source, label }`: `part` is a key stable across recooks (a CAD body's index), so the host matches the items of successive values; `source` is a `ShapeSource`, an immutable snapshot with shared columns; `label` is the name Cryptomatte gives the object.
- `RenderItem::Instances { part, prototypes, which, xforms, primvars, label }`: placements of other items, which an engine instances: what it made of each prototype is shared however many instances place it.

The source contract (`ShapeSource`, `Topology`, `Ranges`) is `kinogaki-render-api`'s, the render interface engines build on, and `render` re-exports it; so a source a data type makes is what a host hands its engines, with no conversion. `RenderItem` stays here.

The host keeps engine scenes in step cheaply: a source with the last identity is skipped; one with the same topology is a points update narrowed by `moved_since`; primvars and subsets are resent only when their columns change. Shading normals (`N`) are asked for lazily, after a drag ends.

## The viewport

Studio's core draws and picks. What it draws is a packet behind `kinogaki-viewport-api`'s `SurfaceSource` trait: shared point positions, vertex shading records, triangles, line segments in line classes and point markers in point classes, each class with its own look and ids, and the ranges that changed since the packet it was derived from. A kernel that needs full control (smooth and authored normals, curvature, hulls and control vertices, see-through faces, live transform previews) makes its own packets and reads them back as their own type with `packet_as`. The contract speaks the SDK's geometry vocabulary: points `Vec3`, placements `Xform`, boxes `Bounds`, quaternions `[x, y, z, w]`; bulk columns stay plain arrays.

The simpler path is a `viewport::Surface`, a `SurfaceSource` itself: points, polygon corners (point, face, normal), triangles over the corners, and line segments with edge ids. During a drag a type returns a surface `derived` from the last one, with the points that moved, and the host patches only those. Picks come back as `Hit`s and box selections as `ElementSet`s, in the ids the packet gave.

`Displayable::prepare` returns a `Drawn`: the value's pieces in drawing order. A piece is a packet, where it sits in its object (none, or several placements sharing one upload), its role (the object's own surface, or a templated node's faint result the open context marks). Core draws every piece while its object is open too. `prepare` receives what the type made for the value shown before, so a type whose values change a little at a time builds on it, and `refine` offers a finer display once the value rests.

## Self-contained

The SDK depends on the standard library and two crates.io crates its standard geometry values need, at exact versions: blake3 1.8.7 (the topology fingerprint) and bytemuck 1.25.2 (plain records and free casts into columns). Beyond those, two optional features: `kinogaki-render-api`'s `wgpu` (the display contract), which brings wgpu 30.0.1, and `kinogaki-viewport-api`'s `bytemuck` (byte casts of the records a renderer uploads), from crates.io. `kinogaki-studio-api`'s `wgpu` feature turns the first on for the makers of packages' render engines (`Package::makers`; their descriptions, `Package::engines`, are headless), and is off by default. Every type above is the SDK's own, so a package compiles against nothing private and the SDK builds from a plain clone. Studio converts between these types and its own libraries' at the boundary (its maths, its property schemas, its project documents, its viewport and render libraries, its worker threads).


# The UI layer

`kinogaki-studio-ui-api` is how a plugin shows things: an inspector, a tool's card, viewport overlays, a settings page, heads-up value fields.

## The host draws

**Plugins never see Studio's UI library or egui.** Every type a plugin draws with is this crate's own: geometry, colors, the theme, fonts and icons by name, input, a painter and the widgets. Studio implements the `Ui` and `Painter` traits over its private UI library, and each call maps to exactly the call it replaces there. So the SDK stays self-contained, a plugin compiles against nothing private, and Studio can change how it draws without breaking a plugin.

The crate depends on `kinogaki-studio-api` (for parameter values, units and diagnostics) and the standard library, nothing else.

## What a plugin uses

From the most declarative to the most direct:

| Part | What it is | Used for |
|---|---|---|
| `sheet::Schema<K>` | Sections and typed fields with their values, as data; the host draws it with `Ui::sheet` and reports `Edits` | inspectors, a tool's card body, settings pages (`PageRows::rows(…) -> Schema`, then `apply` the `Edits`) |
| `CardSpec`, `PopoverSpec` | Floating panels the host frames around a body (a popover as the host's raised popup or a flat panel) | a tool's parameters; a picker beside a handle; a list at the pointer |
| `ValueFields` | The heads-up boxes a drawing tool types into; `Ui::value_fields` draws them and holds the keys while shown | a line's length and angle while drawing |
| `Ui` | Immediate widgets (labels, buttons, toggles, number fields, sliders, text fields, choices, swatches, menus) and layout | small panels the schema can't express |
| `Painter` | Segments, circles, rectangles, polylines, text and icons, single or in batches; text measuring; reserved slots | viewport overlays: handles, guides, badges, labels |
| `Input` | This frame's pointer, keys and typing, as plain data | tools reading the pointer and shortcuts |

A node's parameter schema (`params::Schema`) becomes a sheet with `From`, keyed by parameter name, so the host shows every node's parameters the same way.

A node type shapes its rows from what its node reads (`NodeType::schema` with a `SchemaCx`: which ports are wired, what each input held at the last cook, what the node itself made then while its stamp holds (`SchemaCx::output`: an Auto choice can name what it came to), the names of the nodes behind each port's wires). It hides rows a wire or a choice makes moot, fills its kinds' options (`ValueKind::Attribute`'s `menu` of the input's attribute names; `ValueKind::Integer`'s `menu`, the names behind the wires, for an integer choosing a wire; `ValueKind::Choice`'s `unavailable` keys; `ValueKind::Code`'s `language`), marks a faulty value with its `problem`, and adds `Button` rows. A button holds no value: pressing it asks `NodeType::pressed`, and core applies the answer as one edit on the inspector's own path, so `NodeType::edited` answers it too. A choice shows as segments while it has three entries or fewer, 18 characters together at most and none unavailable (`sheet::segments`), else as a menu. A held node's delta shows its own rows (`HeldEdit::rows`, over the node's held result), written back through `HeldEdit::apply_rows` and `HeldEdit::pressed`. `SchemaCx::purpose` says where the rows show: the inspector, or a tool's card (`RowsFor::Card { more }`), where a type may show fewer.

Every kind of value has a sheet kind, drawn by Studio's own widget for it, like Houdini's parameter templates: numbers, vectors, colors (with an emission's swatch, `Trailing::emission`), linked RGBA channels, choices, toggles, text, code, secrets, file paths, free paths in the scene's tree, references (`Kind::Reference(RefFilter)`: the field says what fits, such as objects of some kinds, results of some data types or Materials, and Studio fills the searchable picker, takes drops from the outliner and networks, and resolves the path from the sheet's holder), float and color ramps (`Value::Ramp`; `kinogaki_value::ramp`, re-exported as `params::ramp` and `sheet::ramp`, is their one maths: nodes, Studio's ramp widget and render engines evaluate them alike; a ramp row may fix one interpolation for every key), tone curves (`Value::ToneCurves`) and a material input's connection button (`Socket`). A file path's folder button opens the host's file dialog as its `FileDialog` says, and the chosen file arrives as an edit of the field. A field's `animation` (`Animation`: static, animated, keyed at the current time, or overridden: set from elsewhere than its own) is its value field's tint; Studio sets it on the rows it animates (node parameters, objects' placement and kind rows) and marks them `keyable`, so an Alt+click on the label reports `Action::Key` and Studio sets or removes the key there. A package's own rows are not keyable unless it marks them. Raw `Ui` widgets are for custom panels only.

Meshes, CAD surfaces and wireframes never go through the painter. They are the data types' display surfaces (the `Displayable` facet), drawn by the host's viewport; the painter is for the few hundred marks a tool puts over them.

## Rules

- **Immediate mode, on the UI thread.** A plugin calls its `Ui` every frame, inside the host's frame, and what it called is what shows. Nothing it draws persists. No call blocks; long work belongs on the host's workers.
- **Bodies run once.** Layout methods take their body as `&mut dyn FnMut(&mut dyn Ui)` so the traits stay object-safe. The host calls it exactly once before returning; menu bodies run only while the menu is open. `UiExt` wraps the layouts for closures that return a value (`horizontal_with` and so on).
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

A package's UI never sees the editor. Each call into it gets a context, `&mut dyn host::Cx`, reaching the host's capabilities, one trait each: `HostUndo` (gestures, undo, redo), `HostSlots` (typed through `HostSlotsExt`: `slot`, and the one mutator `edit_slot(label, |t| …)`, an undo step named at the call, part of the open gesture's), `HostSelection`, `HostDoc` (where the user is, objects and their bodies, a body set as one undo step with `set_object_body`, stored values, an object's shown result and the polygons it draws as with `shown_polygons`), `HostNetwork` (objects' networks, read and edited), `HostView`, `HostStatus`, `HostShell`, `HostTransform` and `HostSettings` (typed through `HostSettingsExt`); plus the plain `HostPrefs`, the wake for the package's workers, latest-wins jobs on the host's lanes, typed by a `LatestKey` (`HostLatestExt::latest(asked, job)`, `take_latest() -> Option<(Asked, Out)>`: the host runs a job only when what it is asked for changes, a newer ask cancels the older, only the newest ask's result is given, and the host wakes the package when it lands, so a package asks every frame and compares nothing itself), the host's current theme (`Cx::theme`, the one `Ui::theme` draws with), and a service over the host's drawing: `UnitsService` (numbers in the user's units). The viewport parts' calls about one view (`LiveTools::tools`, `Panels::panels`, `Display::overlay` and `edited`) get a `ViewCx` (the view as data: its rectangle and the pane's, its response, what the pointer is over as a `PointerOver`, its camera; this frame's `Input`; its snaps and its display options), the inspector an `InspectorCx`. Among the capabilities: `HostUndo::begin_gesture` gives the gesture's `GestureToken`, which alone finishes or cancels it (none while another's gesture is open: what follows is part of that one, so a drag inside a live tool's run stays the tool's step), `HostUndo::back_to(token.start())` puts back what the gesture changed and keeps it open (a live tool recomputing from its start stays one step), `HostView::rendering` says whether a view renders, `HostStatus::console` writes a Console record about a subject the context's `Inspector::select_subject` resolves, and `HostShell::copy_text`/`paste_text` use the clipboard. Core's node sheet draws a node's rows; `Inspector::row_action` (a group or pick row's pick in the viewport, `RowAction::Pick`, from the inspector or a tool's card) and `Inspector::typing` (the text as typed, for live feedback) tell the context, and core owns file, path and reference rows.

**A package's UI** (`package::UiPackage`) is a state type, its contexts' UI halves (`ContextPart<S>`, under the context's id: a `Library` and optional parts, each a capability: `Levels`; in the viewport `LiveTools`, `Panels`, `Transforms`, `Display`, `Timeline`, `NumericBar`; `Inspector`, `Vocabulary`, `ObjectRows`; a context has the parts it gives and nothing else; what an object level creates is each object kind's own, `ObjectKindSpec::creations`; the network half is the context's `NetworkContext`, see [Networks and contexts](#networks-and-contexts)) and its pages of the Settings window (`SettingsPage { scope, label, rows }`: `PageRows` over a draft of one scope's stored values, drawn and applied lent the package's state and a `Cx`, so a page shows what is live; a `SettingsPage::user` page is a page of the window, its values saved with the user's settings and never undone; a `SettingsPage::project` page is a section of Settings ▸ Project over the project's values, the standard ones such as `ColorConfiguration` and the package's own of `Scope::Project`, and OK or Apply makes its changes one document edit); `ObjectRows::opened` tells the context when one of its objects is first inspected; `Levels::hovered` and `Levels::picked` hand it what the host picked at the active level (a ray, box or lasso through the open value's `Pickable`, with the host's pick policy and a `Combine` from Shift and Command), and the context decides what the hit selects; its id, label and slots are its `Package`'s. `Package::with_ui` (`package::WithUi`) adds it to the package and erases the state's type, and the host reads it back with `package::hosting::Registered::of` (the erased forms of the parts are made from each part trait's one definition, so no call can be left unforwarded; the hosting module is the host's alone). A context whose networks are core's needs no UI half: the host gives it its network pane, Tab and menus listing the context's listed node types (with Display, Bypass and Collapse into Subnet), and its node inspector, and a `ContextPart` adds to these or replaces them (its library's items first and what it sets winning, its vocabulary running its own items, its inspector drawn under the node sheet). The host keeps the state, and for each call the host takes the state out, lends it to the part with the call's context, and puts it back. Host methods never call back into a package; what would (an undo ending a live tool, an action) is queued until the call returns. Undo never holds a package's state: it restores the document and the slots.

**The library.** What a context offers the host's menus is data, read once when the package registers: its items (each tool, command or node by its stored name, with its label, family, search words, Tab section, the levels it applies at and which surfaces offer it), its key store and default bindings, its A/S/D marking menus and shelves (the user's to edit, kept under the menus' store name), its right-click menus and menu bar sections (each for a level, a selection or none, inside a held edit or outside one), how Tab lists its items (tools over the views while it edits, else nodes; a Suggested category) and the viewport chrome it takes (`viewport`: `None` for core's view of the scene, `ViewportSpec::scene()`, or exactly a spec of its own: its display options, the snap steps, the starting pivot, the host's transform tools). A context may share another's keys or menus (`Shared::Of`). The host builds every one of those surfaces from it. An entry that can't apply is left out, never greyed: its `When` (level, selection, held edit) and the vocabulary's `shown` decide; `label` renames an item by its state ("Repeat Bevel"). The vocabulary runs the items.

**Select All.** Edit ▸ Select All runs core's Select All in whichever context is open, and each context answers it its own way. A package's context answers it by holding a library item named `command:select_all` (core's id for Select All) and running that name in its `Vocabulary`, as an item invoked from the views (`Invoked::Views`). The item can also sit on the context's own menus. A context without that item, or without a vocabulary, doesn't answer, and Edit ▸ Select All is unavailable while it is open.

**The edited surface.** Core renders every view, and draws the open object's value itself (its drag previews too). While a context's object is open, its `Display::edited` (a query on its state, after `Display::settle` ran once that frame) returns, per view and frame, an `Edited`: what it adds over that value, never a surface of its own: overlay layers (each bound to the topology it was made for, so one made for the last cook drops out while the rest show) and guides, whether its points show at any level (`points`), which of the value's own pieces draw (`pieces`: a print's layers up to the Timeline's, chosen without building anything); which nodes' cooked values draw ghosted (`ghosts`: a tool's input beside what it makes, each in a `GhostTone` the host draws in its theme's roles: `Neutral` plain, `Add` the accent, `Remove` the error color); core draws the selection (`Levels::components`, one set per element kind, each in its kind's style: edges and faces of a Shift-mixed selection both show) and the hover (the hit it handed `Levels::hovered`) itself through the data type's `Displayable::highlight`. Everything in it is an `Arc` of the packet contract's types, so core borrows it into its renderer's scene without a copy and the renderer uploads only what changed allocation. A drag of polygons' points (the host's gizmo handing `Transforms::transform` its steps, or a tool's own drag) shows through `Edited::moving`, an `edit::preview::Moving`: the context resolves the moved points and their soft weights once when the drag begins and hands the same `Arc`s each frame with the current step in its `edit::deform::Deform` (its `Symmetry` for a symmetric move), placed by the function its commit cooks with. Core previews it over the shown value (no cook, no cached value, only the moved points' polygons sent to the GPU) and keeps the last step shown while the committed edit cooks.

**Live tools.** The rest of the viewport is core's: the left toolbar (the same manipulation tools everywhere), the status bar, framing, the gizmo, the pivot (the user's choice per context, starting at the spec's `default_pivot`, offered on the numeric bar while `Levels::pivot` gives a frame, with Pick through the value's `Snappable`; `HostTransform::pivot` is where it resolves, and `Levels::pivot_changed` tells a live tool it moved) and component picks. Which gizmo shows is one question, `Transforms::gizmo` (`GizmoUse::Selection`: the host's gizmo at the selection's pivot, its drags handed to `Transforms::transform`; `NodeHandles`: the selected node's handles in the transform tool's place; `None`); a context without a `Transforms` part shows the node's handles. A context's `LiveTools` add its live tools. A frame asks `holds` (a live drag keeps the other views and Space), then each view's `Panels::panels` (where its floating panels are, `Panel { id, rect, scope }`, a `View`'s own or over the whole `Pane`; the pointer over one is the panel's) and `LiveTools::tools` before the camera's navigation: the tool reads its view (`ViewCx::view`: `PointerOver::Handle` says a handle has the pointer, so a tool leaves the press to it) and the frame's input (`ViewCx::input`), and says what it `Takes` (`picks`, `gizmo`, `handles`, `hover`, `wheel`, `menu`, each kept from core apart). Then core's gizmo, handles and picks as the tools left them, and the frame's commands; then `Display::settle` once (where the context adopts what changed and asks its latest-wins jobs), then each drawn view's `Display::overlay` (paint only: heads-up values, labels), each panel drawn once (`Panels::panel`: a tool's card through `Ui::card`, its body the tool's node's rows drawn by the host with `Cx::node_rows`, `RowsFor::Card`: a node type's `schema` may leave out rows the run isn't adjusted by, `NodeRows::hidden` says some were and the card offers More; their edits take the inspector's path, inside the tool's gesture), and each view's `Display::edited`. Keys come first: `LiveTools::key` sees a key or typed text before any binding (`HostShell::keyboard_taken` says a field has the keys), never Escape. Escape goes to the first of a handle drag (it puts back the drag), the host's open popup, popover, menu or Tab browser, the transform tool's Pick, and then `LiveTools::cancel(Ended::Escape)`; an undo or a redo cancels the tools first (`Ended::Undo`), and so does the location leaving their context (`Ended::Left`). A tool snaps its own drags with `ViewCx::snap` (a screen point, the targets, a radius and what to snap on, `SnapOn`: the open object, another object's shown value, or a node's cooked value, a tool's input upstream of what its object shows): the host searches that value through its type's `Snappable` exactly as the gizmo does, adds the open object's own targets (`LiveTools::own_targets`: control points, a sketch's points and lines as a polygons value), answers in the open object's space and marks the hit as it marks the gizmo's. A node's value is cooked and prepared for display as an object's shown value is, kept while it stays; until it is there the snap finds nothing. The same values draw ghosted in a view through `Edited::ghosts`. While `NumericBar::fields` gives fields, they stand in the transform tool's under the active view and `NumericBar::apply` takes what is typed. While `LiveTools::live_node` names the tool's node, the open network's templated branches are pickable: a click wires one into the node's operand input inside the tool's gesture, and core marks the branch under the pointer and those wired in. A context's own time is its `Timeline`. Tab over the views lists the context's tools while its `TabSpec::tools` holds.

**The display popover and display options.** Each view has core's display popover beside its Camera menu, its rows drawn by core's one popover drawing (property sheets for its fields) and placed for tests at `["Popover", "Display", label]`. It holds what the view draws, each view's own (saved with the project's view, never undone): Wireframe or Solid (Render is the Camera menu's), the grid, and under Solid Flat or Shaded, the wireframe over Solid, back faces tinted; back faces culled; and under Shaded the analysis modes (Zebra, Isophotes, Curvature, Draft, Normal colors) with the chosen mode's settings. Under **Overlays** come the open context's display options (`ViewportSpec::display`, each a `DisplayOption`: an id, a label, a scope and a kind): a `Toggle` or a `Choice`, each `PerView` (the view's own, kept in core's view value and saved with the project) or `Context` (the same in every view, kept in the user's settings); a section with nothing to show is absent. Neither is undone and nothing cooks when one changes. A view call reads one with `ViewCx::displayed(id)`: as the user set it in its scope, else the option's default (a toggle's 1 or 0, a choice's index); 0 for one the open context doesn't declare. The context draws what an option shows itself, in `edited` or `overlay`. The Gear at the foot of the left toolbar holds the transform tool's settings here: core's space row while the context takes the host's transform tools (has a `Transforms` part), then the context's own sheet (`Transforms::gear`, its edits handed to `apply_gear`); it shows while it has rows.

Rules: every call runs on the UI thread and returns at once; a query gets `&S` and `&dyn Cx` and changes nothing; workers wake the host once per result and the host calls `UiPackage::poll`; no timers.

## Testing

`studio_ui_api_testing::FakeHost` stands in for Studio's host interface (slots and objects' networks with a real undo history, selections, a place, views and stored values as plain fields, every request recorded as a `HostCall`), and `studio_ui_api_testing::Hosting` calls a registered package's parts on it as the host does.

`FakeHost` also keeps the clipboard, the views that render, the display options' values in each view (`displayed`, what `ViewCx::displayed` answers), the view call's `input`, and the latest-wins jobs (an ask equal to its kind's last runs nothing), which `run_latest` runs as the host's lanes would; `studio_ui_api_testing::view` and `camera` build a view to call the viewport parts about.

`studio_ui_api_testing::FakeNetwork` is one object's network in memory, made from the packages whose node types it holds. It answers `NetworkRead` and `NetworkEdit` as core does, refusing what core refuses, and cooks a node through its type (`cook`). Put into `FakeHost::networks`, it is what a package's tools edit through `HostNetwork`, one undo step an edit; so a tool's network edits, and a node type in a network, are tested without Studio. It is a test double, not core: it caches nothing and cooks every node anew, and its rustdoc lists where it is simpler than core.

`studio_ui_api_testing::RecordingUi` and `studio_ui_api_testing::RecordingPainter` are headless fakes: they lay widgets out simply, record every call and every shape, and play back scripted user actions (click a button, edit a sheet field, type into a field, pick a choice). A plugin's unit tests drive its panels with them, with no window and no GPU.
