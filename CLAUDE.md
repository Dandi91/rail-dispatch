# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Run Commands

```sh
# Run in development mode
cargo run

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

## Key Dependencies
- **bevy 0.18** — game engine
- **toml** / **serde** — level config parsing
- **event-listener** / **futures-lite** — async asset loading (`assets.rs`)
- **itertools** / **rand** — utilities

## Architecture Overview

Rail Dispatch is a railway dispatching simulation game using **Bevy 0.18** with an ECS architecture organized into plugins. Assets are loaded from the `resources/` directory (not `assets/`).

### Plugin Registration Order (src/main.rs)
`DebugOverlayPlugin` → `DropdownPlugin` → `LevelPlugin` → `AssetLoadingPlugin` → `TimeControlsPlugin` → `DisplayPlugin` → `AudioPlugin` → `TrainPlugin` → `SpawnerPlugin` → `MapPlugin` → `StationPlugin`

### Simulation vs. Display Split
- `src/simulation/` — pure game logic, no rendering: `block.rs`, `train.rs`, `signal.rs`, `spawner.rs`, `station.rs`
- `src/*.rs` — rendering and UI: `display.rs`, `debug_overlay.rs`, `dropdown_menu.rs`, `time_controls.rs`, `audio.rs`
- `src/assets.rs` — asset loading state machine; `src/level.rs` — `Level` TOML asset definition

### Central Data Structures
- **`BlockMap`** (`simulation/block.rs`): The core resource. Holds the full track topology, signal placement (`SignalMap`), switch states, and train occupancy (`BlockTracker`). Built from `level.toml` at startup.
- **`SparseVec<T>`** (`simulation/sparse_vec.rs`): Custom structure for storing objects indexed by integer IDs (blocks, signals). Optimized for append.
- **`Train`** (ECS component, `simulation/train.rs`): Physics per train — acceleration, braking, speed limit, position along track as `TrackPoint { block_id, offset }`.
- **`RailVehicle`** (`simulation/train.rs`): Struct defining physical properties (mass, power, length) of locomotives and cars.
- **`AssetHandles`** / **`SoundHandles`** (`assets.rs`): Resources holding `Handle<Level>`, `Handle<Image>` (board), and four `Handle<AudioSource>` handles (beep/error/message/notification).
- **`LoadingState`** (`assets.rs`): Bevy `State` enum — `Loading` → `Loaded` → `Instantiated`.

### Messaging System (CRITICAL)
The project uses Bevy 0.18's **buffered Message system** for all simulation inter-system communication — **not** `EventReader`/`EventWriter`.

- Define events with `#[derive(Message)]`
- Use `MessageReader<T>` and `MessageWriter<T>` (both from `bevy::prelude`)
- Register with `app.add_message::<T>()` in the plugin that owns the message type

Messages live alongside the plugin that owns them:
- `MapPlugin` (`simulation/block.rs`) registers `BlockUpdate`, `LampUpdate`, `SignalUpdate`.
- `StationPlugin` (`simulation/station.rs`) registers `SwitchUpdate` and `RouteActivationRequest`.
- `TrainPlugin` (`simulation/train.rs`) registers `TrainSpawnRequest` and `TrainDespawnRequest`.

**Exception**: UI/audio-triggered events use `#[derive(Event)]` with `commands.trigger()` and `On<T>` observers — `SpawnRequest` (`spawner.rs`) and `AudioEvent` (`audio.rs`) are intentional examples.

### Spawner Architecture
Spawner blocks are **ordinary blocks** declared in `level.toml` with one open end (a dead end in the topology). No virtual blocks or extra signals are generated at runtime — everything is declared in the level file.

Each spawner entry has a `kind`: `Spawn`, `Despawn`, or `Both`.

**Spawn side** (`Spawner` component): trains are placed at `SPAWNER_POINT_OFFSET` (400 m) from the open end of the block. The `Spawner` component tracks `block_id`, `direction` (travel direction into the map), `speed_kmh`, `spawn_point`, and current `train` occupation (via `Occupation` which counts how many blocks the train spans).

**Despawn side** (`Despawner` component): watches `block_id` and `adjacent_block_id` (the next block inward). When a train clears both, a `TrainDespawnRequest` is written. If a signal exists at the open end of the despawn block, it is permanently opened (sent `Unrestricting`) at init so incoming trains aren't speed-restricted.

`SpawnerMapper` resource maps `BlockId → Entity` for fast lookup of `BlockUpdate` messages. Approach blocks (up to `approach_len` blocks ahead) are also added to the mapper so the `Spawner` can track a multi-block train throughout its entry.

`SpawnerData` fields: `block_id`, `kind`, `approach_len` (extra blocks to watch), `speed_kmh` (initial train speed), `x`/`y` (UI button position).

### Level Format (`resources/level.toml`)
Sections: `lamps` (visual positions), `blocks` (track segments with length/ID), `connections` (topology), `switches` (branching points), `signals` (control points), `spawners` (entry/exit points), `stations` (named groups of routes, each route has a `signal` ID and a `sections` array), `background` (hex color string).

The `Level` struct is a proper Bevy `Asset` loaded via the custom `LevelLoader` in `src/level.rs`.

### Signal Types (`simulation/signal.rs`)
- **`TrackSignal`**: Signal entity on a block; holds aspect, speed control info, and a `SignalType` (`Automatic` / `Manual`).
- **`SignalMap`**: Specialized container using `SparseVec` plus a spatial hash map for fast signal lookups by position.
- **`SpeedControl`**: Computes per-signal speed restrictions propagated toward approaching trains.
- **Manual signals**: when `Forbidding`, `BlockChange(Freed)` and `SignalPropagation(_)` cannot open them — only `SignalUpdateState::Manual(_)` (route activation) can. Once open, they behave like automatic signals until closed again (e.g. train entering the guarded block).

### Common Types (`src/common.rs`)
`TrainId`, `BlockId`, `SignalId`, `LampId`, `SwitchId` are all `u32` type aliases. `Direction` enum has `Even = 1` / `Odd = -1` variants used for directional math via `apply_sign`. `HexColor` wraps `Srgba` for TOML deserialization. `SpeedConv` trait adds `.kmh()` / `.mps()` helpers to `f64`.
