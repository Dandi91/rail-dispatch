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

## Architecture Overview

Rail Dispatch is a railway dispatching simulation game using **Bevy 0.18** with an ECS architecture organized into plugins. Assets are loaded from the `resources/` directory (not `assets/`).

### Plugin Registration Order (src/main.rs)
`DebugOverlayPlugin` → `DropdownPlugin` → `LevelPlugin` → `AssetLoadingPlugin` → `TimeControlsPlugin` → `MessagingPlugin` → `DisplayPlugin` → `TrainPlugin` → `SpawnerPlugin` → `MapPlugin`

### Simulation vs. Display Split
- `src/simulation/` — pure game logic, no rendering: `block.rs`, `train.rs`, `signal.rs`, `switch.rs`, `spawner.rs`
- `src/*.rs` — rendering and UI: `display.rs`, `debug_overlay.rs`, `dropdown_menu.rs`, `time_controls.rs`

### Central Data Structures
- **`BlockMap`** (`simulation/block.rs`): The core resource. Holds the full track topology, signal placement (`SignalMap`), switch states, and train occupancy (`BlockTracker`). Built from `level.toml` at startup.
- **`SparseVec<T>`** (`simulation/sparse_vec.rs`): Custom structure for storing objects indexed by integer IDs (blocks, signals). Optimized for append.
- **`Train`** (ECS component, `simulation/train.rs`): Physics per train — acceleration, braking, speed limit, position along track as `TrackPoint { block_id, offset }`.

### Messaging System (CRITICAL)
The project uses Bevy 0.18's **buffered Message system** for all simulation inter-system communication — **not** `EventReader`/`EventWriter`.

- Define events with `#[derive(Message)]`
- Use `MessageReader<T>` and `MessageWriter<T>` (both from `bevy::prelude`)
- Register with `app.add_message::<T>()` in `MessagingPlugin`

Current messages: `BlockUpdate`, `LampUpdate`, `SignalUpdate`, `SwitchUpdate`, `TrainSpawnRequest`, `TrainDespawnRequest`

**Exception**: UI-triggered events (e.g., `SpawnRequest` in `spawner.rs`) use `#[derive(Event)]` with `commands.trigger()` and `On<T>` observers — this is intentional.

### Spawner Architecture
Each spawner entry in `level.toml` generates a **2000m virtual block** to hold trains outside the visible map. Virtual blocks have an always-open (green) signal at the open end, so that the speed of incoming trains is not limited. The `Spawner` component tracks its virtual block ID and the direction trains travel through it.

### Level Format (`resources/level.toml`)
Sections: `lamps` (visual positions), `blocks` (track segments with length/ID), `connections` (topology), `switches` (branching points), `signals` (control points), `spawners` (entry/exit points).

### Common Types (`src/common.rs`)
`TrainId`, `BlockId`, `SignalId`, `LampId`, `SwitchId` are all `u32` type aliases. `Direction` enum has `Even = 1` / `Odd = -1` variants used for directional math via `apply_sign`.
