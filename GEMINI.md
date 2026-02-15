# Rail Dispatch Project Context

## Overview
**Rail Dispatch** is a railway dispatching simulation game built with **Rust** and the **Bevy game engine (v0.18.0)**. The project simulates train movements, signaling, track switching, and train spawning on a predefined level layout.

## Project Structure & Architecture
The application follows the **Bevy ECS (Entity Component System)** architecture, organized into modular plugins.

### Core Modules (`src/`)
*   **`main.rs`**: Application entry point. Configures the Bevy app, windows, and registers all plugins.
*   **`level.rs`**: Defines the data structures for parsing the level configuration from TOML files (`resources/level.toml`).
*   **`assets.rs`**: Manages async asset loading (textures, level data) and handles loading states (`LoadingState`).
*   **`common.rs`**: Shared type definitions (`BlockId`, `TrainId`, `Direction`, etc.) and utility functions.
*   **`display.rs`**: Renders the game world using Bevy's UI system. Handles lamp visualization and user interactions (clicks).
*   **`debug_overlay.rs`**: Provides an on-screen debug information overlay (e.g., train details when hovering over lamps).
*   **`dropdown_menu.rs`**: Generic UI component for context menus.
*   **`time_controls.rs`**: Manages simulation time speed and pausing.

### Simulation Core (`src/simulation/`)
This module contains the game's logic, decoupled from rendering.

*   **`mod.rs`**: Module exports.
*   **`block.rs`**: Defines `BlockMap` and `Block`.
    *   **`BlockMap`**: The central resource representing the track network. It manages track topology (connections), signal placement, switch states, and train occupancy (`BlockTracker`).
    *   **`from_level`**: Initializes the map, including generating virtual blocks for spawners via `generate_spawners`.
*   **`train.rs`**: Implements train physics (`Train` component) and movement.
    *   **`Train`**: Handles acceleration, braking, speed limits, and position updates along the track.
    *   **`RailVehicle`**: struct defining physical properties (mass, power, length) of locomotives and cars.
*   **`signal.rs`**: Manages signal logic (`TrackSignal`, `SignalMap`) and speed control (`SpeedControl`).
    *   **`SignalMap`**: Specialized container using `SparseVec` and a spatial hash map for fast signal lookups.
*   **`switch.rs`**: Defines `Switch` structures for track branching.
*   **`spawner.rs`**: Logic for spawning and despawning trains at the edges of the map.
    *   **`SpawnerPlugin`**: Systems for handling `SpawnRequest` messages and auto-despawning trains in virtual blocks.
*   **`messages.rs`**: Defines the communication protocol between systems.
*   **`sparse_vec.rs`**: A custom, efficient data structure (`SparseVec`) for storing game objects indexed by integer IDs (like blocks and signals). Optimized for append operations.

## Messaging System & Conventions (CRITICAL)
The project uses Bevy 0.18's **Message** system for buffered inter-system communication.

*   **Event Handling**: Use **`MessageReader<T>`** and **`MessageWriter<T>`** to read and write events.
    *   **DO NOT** use `EventReader` or `EventWriter` (these are for Observer-based immediate events in newer Bevy versions, but this project uses the buffered Message pattern for simulation steps).
    *   `MessageReader` and `MessageWriter` are available in `bevy::prelude`. **Do not import them from `crate::simulation::messages`**.
*   **Event Definition**: Event structs must derive `Message` (e.g., `#[derive(Message)]`).
*   **Registration**: Events are registered in `MessagingPlugin` using `app.add_message::<T>()`.

For **UI** interactions, it is still okay to use non-buffered `Event`s and `Observer`s.

### Current Messages
*   `BlockUpdate`: Notifies when a train enters or leaves a block.
*   `LampUpdate`: Updates the visual state of track lamps.
*   `SignalUpdate`: Propagates signal aspect changes.
*   `SwitchUpdate`: Notifies switch state changes.
*   `SpawnRequest`: Request to spawn a train (payload: `spawner_block_id`, `train_type`).

## Data Model
*   **Level Format (`resources/level.toml`)**:
    *   `lamps`: Visual xy positions.
    *   `blocks`: Logical track segments (length, ID).
    *   `connections`: Topology (start -> end).
    *   `switches`: Branching points.
    *   `signals`: Control points on blocks.
    *   `spawners`: Entry/exit points.
*   **Spawners**:
    *   Spawners are logical entities attached to the ends of the physical track.
    *   The simulation generates a **2000m virtual block** for each spawner to hold trains entering/leaving the map.
    *   Virtual blocks are connected via an **Always Open (Green)** signal to the main track.

## Key Dependencies
*   **Bevy**: 0.18.0 (Game engine).
*   **toml**: Level configuration parsing.
*   **serde**: Serialization.
*   **event-listener**, **futures-lite**: Async asset loading.
*   **itertools**, **rand**: Utilities.

## Development Status
*   **Implemented**: Basic simulation loop, level loading, train physics, signaling, track switching, UI rendering, train spawning/despawning via UI.
*   **TODO**: Routing signals, speed restrictions per route, advanced train AI.
