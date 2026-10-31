//! Headless smoke tests for the simulation plugins, driven by the reference level
//! (`tests/fixtures/reference_level.toml`, a frozen copy of the map, so tests can name its ids).
//!
//! These add the real plugins (not individual systems), so missing resources, query conflicts
//! and ordering mistakes between plugins surface here rather than on the first game frame.

use std::time::Duration;

use bevy::prelude::*;
use bevy::state::app::StatesPlugin;
use bevy::time::TimeUpdateStrategy;
use rail_dispatch::assets::{AssetHandles, LoadingState};
use rail_dispatch::common::{BlockId, Direction};
use rail_dispatch::level::{Level, LevelPlugin};
use rail_dispatch::simulation::block::{BlockMap, MapPlugin};
use rail_dispatch::simulation::signal::SignalAspect;
use rail_dispatch::simulation::spawner::{SpawnRequest, SpawnTrainType, SpawnerPlugin};
use rail_dispatch::simulation::station::{RouteActivationRequest, StationPlugin};
use rail_dispatch::simulation::train::{Train, TrainPlugin};

const REFERENCE_LEVEL: &str = include_str!("fixtures/reference_level.toml");

/// Builds an app with the simulation plugins and the reference level, advanced to `Instantiated`.
fn simulation_app() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default(), StatesPlugin, LevelPlugin))
        .init_state::<LoadingState>()
        .add_plugins((TrainPlugin, SpawnerPlugin, MapPlugin, StationPlugin));

    let level: Level = toml::from_str(REFERENCE_LEVEL).expect("reference level parses");
    let handle = app.world_mut().resource_mut::<Assets<Level>>().add(level);
    app.insert_resource(AssetHandles { level: handle });
    app.world_mut()
        .resource_mut::<NextState<LoadingState>>()
        .set(LoadingState::Loaded);

    for _ in 0..3 {
        app.update();
    }
    assert_eq!(
        *app.world().resource::<State<LoadingState>>().get(),
        LoadingState::Instantiated,
        "simulation should be instantiated after loading"
    );
    app
}

fn aspect(app: &App, signal_id: u32) -> SignalAspect {
    app.world()
        .resource::<BlockMap>()
        .signal(signal_id)
        .expect("signal exists")
        .speed_ctrl
        .aspect
}

fn activate(app: &mut App, route_id: u32) {
    app.world_mut().write_message(RouteActivationRequest { route_id });
    app.update();
}

#[test]
fn plugins_run_several_frames() {
    let mut app = simulation_app();
    for _ in 0..5 {
        app.update();
    }
    assert_eq!(
        aspect(&app, 50),
        SignalAspect::Forbidding,
        "manual signals start closed"
    );
}

#[test]
fn route_activation_throws_switch_and_opens_signal_in_the_same_frame() {
    let mut app = simulation_app();
    // Route 2: signal 50, switch 1 (base block 4) to the side leg (block 20).
    activate(&mut app, 2);

    let block_map = app.world().resource::<BlockMap>();
    let next = block_map
        .next_block(4, Direction::Even)
        .expect("switch base has a successor");
    assert_eq!(next.id, 20, "switch 1 thrown to the side leg");
    assert_ne!(aspect(&app, 50), SignalAspect::Forbidding, "route signal opened");
}

#[test]
fn conflicting_route_is_rejected() {
    let mut app = simulation_app();
    // Routes 3 and 5 share sections 4 and 5.
    activate(&mut app, 3);
    assert_ne!(aspect(&app, 51), SignalAspect::Forbidding);

    activate(&mut app, 5);
    assert_eq!(
        aspect(&app, 52),
        SignalAspect::Forbidding,
        "conflicting route must not open"
    );
}

/// Switches the app to manual time: every `update()` advances the simulation by one second
/// (250 ms frames at 4x speed, i.e. 64 fixed train steps per update).
fn use_fast_manual_time(app: &mut App) {
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(250)));
    app.world_mut().resource_mut::<Time<Virtual>>().set_relative_speed(4.0);
}

fn head_blocks(app: &mut App) -> Vec<BlockId> {
    let world = app.world_mut();
    world.query::<&Train>().iter(world).map(Train::head_block).collect()
}

/// Runs simulated seconds until `done` holds, panicking after `max_secs`.
fn run_until(app: &mut App, what: &str, max_secs: u32, mut done: impl FnMut(&mut App) -> bool) {
    for _ in 0..max_secs {
        app.update();
        if done(app) {
            return;
        }
    }
    panic!(
        "timed out after {max_secs} s waiting for {what}; trains at {:?}",
        head_blocks(app)
    );
}

fn spawn_locomotive(app: &mut App, block_id: BlockId) {
    // A despawn's final `Exited` moves reach the spawner a frame later; let them land first.
    app.update();
    app.world_mut().trigger(SpawnRequest {
        block_id,
        train_type: SpawnTrainType::Locomotive,
    });
    app.update();
    assert_eq!(head_blocks(app).len(), 1, "train spawned at block {block_id}");
}

fn activate_open(app: &mut App, route_id: u32, signal_id: u32) {
    activate(app, route_id);
    assert_ne!(
        aspect(app, signal_id),
        SignalAspect::Forbidding,
        "route {route_id} should open signal {signal_id}"
    );
}

/// Drives one train each way through the station: west to east over the side track
/// (routes 2 and 6), then east to west over the main track (routes 3 and 7).
#[test]
fn trains_run_through_the_station_both_ways() {
    let mut app = simulation_app();
    use_fast_manual_time(&mut app);

    // Even train from the block 1 spawner: CH -> side track, CH2 -> exit towards block 11.
    spawn_locomotive(&mut app, 1);
    activate_open(&mut app, 2, 50);
    run_until(&mut app, "train 1 to reach CH2 (block 22)", 900, |app| {
        head_blocks(app) == [22]
    });
    activate_open(&mut app, 6, 53);
    run_until(&mut app, "train 1 to despawn at block 11", 900, |app| {
        head_blocks(app).is_empty()
    });

    // Odd train from the block 11 spawner: N -> main track, N1 -> exit towards block 1.
    spawn_locomotive(&mut app, 11);
    activate_open(&mut app, 3, 51);
    run_until(&mut app, "train 2 to reach N1 (block 6)", 900, |app| {
        head_blocks(app) == [6]
    });
    activate_open(&mut app, 7, 54);
    run_until(&mut app, "train 2 to despawn at block 1", 900, |app| {
        head_blocks(app).is_empty()
    });
}
