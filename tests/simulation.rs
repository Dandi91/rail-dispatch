//! Headless smoke tests for the simulation plugins, driven by the shipped level.
//!
//! These add the real plugins (not individual systems), so missing resources, query conflicts
//! and ordering mistakes between plugins surface here rather than on the first game frame.

use bevy::prelude::*;
use bevy::state::app::StatesPlugin;
use rail_dispatch::assets::{AssetHandles, LoadingState};
use rail_dispatch::common::Direction;
use rail_dispatch::level::{Level, LevelPlugin};
use rail_dispatch::simulation::block::{BlockMap, MapPlugin};
use rail_dispatch::simulation::signal::SignalAspect;
use rail_dispatch::simulation::spawner::SpawnerPlugin;
use rail_dispatch::simulation::station::{RouteActivationRequest, StationPlugin};
use rail_dispatch::simulation::train::TrainPlugin;

const SHIPPED_LEVEL: &str = include_str!("../resources/level.toml");

/// Builds an app with the simulation plugins and the shipped level, advanced to `Instantiated`.
fn simulation_app() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default(), StatesPlugin, LevelPlugin))
        .init_state::<LoadingState>()
        .add_plugins((TrainPlugin, SpawnerPlugin, MapPlugin, StationPlugin));

    let level: Level = toml::from_str(SHIPPED_LEVEL).expect("shipped level parses");
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
