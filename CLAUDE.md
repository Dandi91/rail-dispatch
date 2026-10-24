# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Run Commands

```sh
# Run the game (default binary)
cargo run

# Run the map editor binary
cargo run --bin map-editor

# Run tests
cargo test

# Run a single test
cargo test <test_name>

# Build release
cargo build --release

# Build for WASM (wasm-release profile)
cargo build --profile wasm-release --target wasm32-unknown-unknown

# Check formatting
cargo fmt --check

# Format code
cargo fmt
```

**Code style**: max line width is 120 (configured in `rustfmt.toml`).

## Binaries
Two binaries are declared in `Cargo.toml`:
- **`rail-dispatch`** (`src/main.rs`) — the game.
- **`map-editor`** (`src/editor/main.rs`) — interactive level geometry editor (see Map Editor below).

Shared library code lives in `src/lib.rs` (`pub mod assets, audio, common, dropdown_menu, fps_counter, level, panel, simulation, time_controls`).

## Key Dependencies
- **bevy 0.20** — game engine
- **toml** / **toml_edit** / **serde** / **serde_repr** — level config parsing and writing
- **event-listener** / **futures-lite** — async asset loading (`assets.rs`)
- **itertools** / **rand** / **arrayvec** / **thiserror** — utilities

## Architecture Overview

Rail Dispatch is a railway dispatching simulation game using **Bevy 0.20** with an ECS architecture organized into plugins. Assets are loaded from the `resources/` directory (not `assets/`).

### Plugin Registration Order (src/main.rs)
After `DefaultPlugins` (with `AssetPlugin.file_path = "resources"`) and `FpsCounterPlugin` (`fps_counter.rs`, a small `DiagnosticsStore`-fed FPS text):
`MeshPickingPlugin` → `DropdownPlugin` → `LevelPlugin` → `AssetLoadingPlugin` → `TimeControlsPlugin` → `PanelPlugin` → `AudioPlugin` → `TrainPlugin` → `SpawnerPlugin` → `MapPlugin` → `StationPlugin`

### Simulation vs. Rendering Split
- `src/simulation/` — pure game logic, no rendering: `block.rs`, `train.rs`, `signal.rs`, `spawner.rs`, `station.rs`, `sparse_vec.rs`.
- `src/panel.rs` — the dispatcher panel: a **vector schematic** renderer (no board image). Split into `SchematicPlugin` (simulation-agnostic static draw built from the `Level` asset) and `PanelPlugin` (the game layer: message-driven track/signal colouring, train describers, picking/menus/tooltips). Also defines `ScreenScalePlugin` (camera + screen scale) and `CameraControlPlugin` (pan/zoom).
- Other UI/IO: `dropdown_menu.rs`, `time_controls.rs`, `fps_counter.rs`, `audio.rs`.
- `src/assets.rs` — asset loading state machine; `src/level.rs` — `Level` TOML asset definition.
- `src/editor/` — the standalone map editor (see Map Editor below).

### Entity Hierarchies — use `bsn!` (PREFERRED)
Bevy 0.19+ ships **BSN (Bevy Scene Notation)** and the `bsn!` macro. **`bsn!` is the primary way to
define component hierarchies in this project** — prefer it over `commands.spawn((...)).with_children(|p| {...})`
closures for any new entity tree (UI nodes, tooltips, menus, spawner buttons, status bars, etc.). The
closure style still exists in older code (`panel.rs`, `dropdown_menu.rs`, `time_controls.rs`, `editor/`);
migrate toward `bsn!` when touching those sites.

`bsn!` returns an `impl Scene` (a single entity); `bsn_list!` returns an `impl SceneList` (multiple). Spawn
with `world.spawn_scene(...)` / `commands.spawn_scene(...)`, or turn a scene-returning fn into a system with
`.spawn()` (e.g. `app.add_systems(Startup, my_panel.spawn())`).

Core syntax:
- **Components**: whitespace-separated on one entity; bare type uses `Default`, `Type { field: val }` sets fields,
  tuple structs as `Type(val)`. Field values take arbitrary Rust via `{ expr }` and get an implicit `.into()`
  (so `Foo("hello")` works for `Foo(String)`).
  ```rust
  bsn! {
      ContextMenuItem
      Node { padding: {UiRect::all(px(4.0))} }
      Pickable
      Children [
          (Text({label}) TextFont { font_size: {12.0} } Pickable::IGNORE),
      ]
  }
  ```
- **Associated consts and constructors** *are* first-class entries: `Type::CONST` (e.g. `Pickable::IGNORE`) and
  `Type::func(args)` (e.g. `BorderColor::all(Color::WHITE)`) both work — **but only when the constructor/const
  resolves on the component's `FromTemplate::Template` type.** For "simple" components the template *is* the type
  itself, so it works. Components with a *generated* template struct (e.g. `TextFont` → `TextFontTemplate`) do not
  expose their inherent constructors on the template, so `TextFont::from_font_size(12.0)` fails to compile — use the
  field form `TextFont { font_size: {12.0} }` instead. When unsure, just compile: the error names the template type.
- **Children / relationships**: `Children [ a, b ]` (comma-separated entities; group multi-component children in
  `( ... )`). Any relationship works the same way, e.g. a custom `Inventory [ ... ]`. To attach children to an
  *existing* entity, `EntityCommands::queue_spawn_related_scenes::<Children>(bsn_list![...])`.
- **Composition (patches)**: scene-returning fns compose — calling `button()` inside another `bsn!` merges
  component data field-by-field, so layered scenes override only what they set. Build reusable widgets this way.
- **`#Name`**: `#Root` adds a `Name("Root")` and lets other entities in the same scope reference it.
- **`@Type`**: spawns a `SceneComponent`'s associated scene template (with optional `@prop:` props).
- **`on(...)`**: attach an observer inline, e.g. `on(|p: On<Pointer<Press>>| { info!("pressed") })`.

Note: this release is code-driven only (no `.bsn` asset loader yet). Types need `FromTemplate` (auto-derived for
`Default + Clone`); asset-dependent scenes use `queue_spawn_scene` for async loading.

### Central Data Structures
- **`BlockMap`** (`simulation/block.rs`): The core resource. Holds the full track topology, signal placement (`SignalMap`), switch states, and train occupancy (`BlockTracker`). Built from `level.toml` at startup.
- **`SparseVec<T>`** (`simulation/sparse_vec.rs`): Custom structure for storing objects indexed by integer IDs (blocks, signals). Optimized for append.
- **`Train`** (ECS component, `simulation/train.rs`): Physics per train — acceleration, braking, speed limit, position along track as `TrackPoint { block_id, offset }`.
- **`RailVehicle`** (`simulation/train.rs`): Struct defining physical properties (mass, power, length) of locomotives and cars.
- **`AssetHandles`** (`assets.rs`): holds the `Handle<Level>`. **`FontHandles`**: holds `mono` font. **`SoundHandles`**: four `Handle<AudioSource>` handles (beep/error/message/notification).
- **`LoadingState`** (`assets.rs`): Bevy `State` enum — `Loading` → `Loaded` → `Instantiated`.

### Messaging System (CRITICAL)
The project uses Bevy 0.20's **buffered Message system** for all simulation inter-system communication — **not** `EventReader`/`EventWriter`.

- Define events with `#[derive(Message)]`
- Use `MessageReader<T>` and `MessageWriter<T>` (both from `bevy::prelude`)
- Register with `app.add_message::<T>()` in the plugin that owns the message type

Messages live alongside the plugin that owns them:
- `MapPlugin` (`simulation/block.rs`) registers `TrackUpdate`, `SignalUpdate`, `SignalAspectChanged`.
- `StationPlugin` (`simulation/station.rs`) registers `RouteActivationRequest`, `RoutePending`, `SwitchUpdate`.
- `TrainPlugin` (`simulation/train.rs`) registers `TrainMove`, `TrainSpawnRequest`, `TrainDespawnRequest`.

**`TrackUpdate` vs. `TrainMove`** (important distinction):
- `TrainMove { block_id, train_id, kind: Entered | Exited }` — emitted by trains (and `BlockMap::despawn_train`) every time a train steps into or off a block. Consumed by `Spawner`/`Despawner` and by `BlockMap::process_train_moves`.
- `TrackUpdate { block_id, train_id, state: Freed | Occupied, train_direction, train_number, section_ctx }` — emitted **only** by `process_train_moves` when a train move actually flips a block's (or section's) status. For single-block sections, one update per block; for multi-block sections, `Freed` is produced only when all blocks in the section clear. The optional `section_ctx: Option<SectionContext>` carries the owning section's id and block list. Consumed by signal propagation, the `StationMap` route/section trackers, and the panel renderer.
- `SignalAspectChanged { signal_id, aspect }` — emitted by signal propagation to notify the panel of the **settled** aspect (vs. `SignalUpdate`, which is a *request*).
- `MapPlugin` system chain (`Update`, gated on `LoadingState::Instantiated`): `switch_updates → train_moves → track_updates → signal_updates`.

**Exception**: UI/audio-triggered events use `#[derive(Event)]` with `commands.trigger()` and `On<T>` observers — `SpawnRequest` (`spawner.rs`) and `AudioEvent` (`audio.rs`) are intentional examples.

### Spawner Architecture
Spawner blocks are **ordinary blocks** declared in `level.toml` with one open end (a dead end in the topology). No virtual blocks or extra signals are generated at runtime — everything is declared in the level file.

Each spawner entry has a `kind`: `Spawn`, `Despawn`, or `Both`.

**Spawn side** (`Spawner` component): trains are placed at `SPAWNER_POINT_OFFSET` (400 m) from the open end of the block. The `Spawner` component tracks `block_id`, `direction` (travel direction into the map), `speed_kmh`, `spawn_point`, and current `train` occupation (via `Occupation` which counts how many blocks the train spans).

**Despawn side** (`Despawner` component): watches `block_id` and `adjacent_block_id` (the next block inward). When a train clears both, a `TrainDespawnRequest` is written. If a signal exists at the open end of the despawn block, it is permanently opened (sent `Unrestricting`) at init so incoming trains aren't speed-restricted.

`SpawnerMapper` resource maps `BlockId → Entity` for fast lookup of `TrainMove` messages. Approach blocks (up to `approach_len` blocks ahead) are also added to the mapper so the `Spawner` can track a multi-block train throughout its entry.

`SpawnerData` fields: `block_id`, `kind`, `approach_len` (extra blocks to watch), `speed_kmh` (initial train speed), `x`/`y` (UI button position).

### Level Format (`resources/level.toml`)
TOML using array-of-arrays (positional) entries. Tables:
- `blocks` — `[id, length]` track segments.
- `connections` — `[start_block, end_block]` topology (in the even direction).
- `switches` — branching points (`id, base, straight, side, direction`).
- `signals` — control points (`id, block_id, offset_m, name, direction, signal_type`).
- `spawners` — entry/exit points.
- `sections` — `[id, [block_ids]]` visual block groupings.
- `stations` — named groups of routes; each `RouteData` is `[id, signal, [section_ids], target_block, [switches]]`.
- `geometry` — per-block schematic polyline: `BlockGeometry { id, points: Vec<Vec2> }` in level pixel space; the panel maps these to world space.
- `row_spacing` (f32, default 12) — vertical pixel spacing between schematic rows; the editor snaps vertices to a grid at multiples of this value.
- `background` — hex color string.

A `RouteData` has `id`, `signal` (the protecting signal ID), `sections` (the section IDs the route covers), `target` (the destination block), and `switches` (a list of `SwitchSetting { switch_id, position }` to enforce). The route's block set is derived as the union of its sections' blocks.

The `Level` struct is a proper Bevy `Asset` loaded via the custom `LevelLoader` in `src/level.rs`.

### Routes & Sections
Routes and sections are both owned by `StationMap` (`simulation/station.rs`); they are tightly coupled.

`Route` has a `RouteState`: `Inactive` → `Active` (signal opened, awaiting train) → `Used` (train entered, signal auto-closed) → `Inactive` (all route blocks freed). It carries `section_ids` (the sections it covers) and a derived `block_ids` (union of those sections' blocks, used for occupation/conflict checks).

`Section` carries its `blocks`, an `occupied` set, and an `active` flag. A section becomes active when a route covering it is activated, and inactive when the owning route returns to `Inactive`. Because conflicting routes (those sharing any block) cannot be simultaneously active, at most one active route can ever claim a given section at a time.

Activation flow (`handle_route_activation`): validates `Inactive` state, all route blocks free, no conflicting route active (using precomputed `blocks_to_routes` and `conflicting_routes`); writes `SwitchUpdate`s; opens the signal via `SignalUpdate::Manual(Unrestricting)`; flips `active = true` on each route section; emits `RoutePending` so the panel paints the pending (green) route path.

Track colouring is **message-driven in the panel**, not via a lamp model. The panel resolves each block's colour by precedence (occupied > pending > free): occupancy follows `TrackUpdate`, the pending path follows `RoutePending`, and the green pending path is consumed block-by-block as occupancy arrives. `track_route_state` in `StationMap` updates `route.occupied`/`section.occupied` on every `TrackUpdate` to drive route state transitions.

### Signal Types (`simulation/signal.rs`)
- **`TrackSignal`**: Signal entity on a block; holds aspect, speed control info, and a `SignalType` (`Automatic` / `Manual`).
- **`SignalMap`**: Specialized container using `SparseVec` plus a spatial hash map for fast signal lookups by position.
- **`SpeedControl`**: Computes per-signal speed restrictions propagated toward approaching trains.
- **Manual signals**: when `Forbidding`, `BlockChange(Freed)` and `SignalPropagation(_)` cannot open them — only `SignalUpdateState::Manual(_)` (route activation) can. Once open, they behave like automatic signals until closed again (e.g. train entering the guarded block). In the panel, only manual (route-protecting) signals are drawn — a triangle, green when open and subdued red when closed (driven by `SignalAspectChanged`); closed signals stay clickable to set a route.

### Map Editor (`src/editor/`)
A separate `map-editor` binary that mounts the level's `ScreenScalePlugin` + `CameraControlPlugin` and renders its own **editable** layer (not the static `SchematicPlugin`) over the loaded level. It is an interactive geometry editor over a rows model.
- `state.rs` — `EditorModel` (rows model: `BlockRow`, `GeomRow`, derived from the `Level`), `EditMode`, `EditorSelection`, `BlockDrag`, `VertexRef`; `init_model` builds the model from the loaded `Level`. Snaps to the `row_spacing` grid.
- `interaction.rs` — hover, drag, delete, and new-block input handlers.
- `constraints.rs` — `normalize` keeps geometry consistent given anchor vertices.
- `render.rs` — draws the editable schematic, ghosts, guides, vertex highlights.
- `save.rs` — writes the model back out (via `toml_edit`).
- `status.rs` — the editor status bar (uses `FontHandles`).

### Common Types (`src/common.rs`)
`TrainId`, `BlockId`, `SignalId`, `SwitchId`, `SectionId`, `StationId`, `RouteId` are all `u32` type aliases. `SwitchPosition` and `SignalType` (`Automatic` / `Manual`) enums. `Direction` enum has `Even = 1` / `Odd = -1` variants used for directional math via `apply_sign`. `HexColor` wraps `Srgba` for TOML deserialization. `SpeedConv` trait adds `.kmh()` / `.mps()` helpers to `f64`. `Profiler` is a small timing helper.
