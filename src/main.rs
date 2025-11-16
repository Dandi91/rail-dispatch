// mod clock;
mod common;
mod display;
// mod game_state;
mod assets;
mod debug_overlay;
mod level;
mod simulation;
mod time_controls;

use crate::simulation::train::TrainPlugin;
use assets::{AssetHandles, AssetLoadingPlugin, LoadingState};
use bevy::asset::AssetPlugin;
use bevy::dev_tools::fps_overlay::{FpsOverlayConfig, FpsOverlayPlugin, FrameTimeGraphConfig};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use debug_overlay::DebugOverlayPlugin;
use display::DisplayPlugin;
use level::{Level, LevelPlugin};
use simulation::block::BlockMap;
use simulation::messages::{BlockUpdate, LampUpdate, MessagingPlugin};
use time_controls::TimeControlsPlugin;

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins.set(AssetPlugin {
                file_path: "resources".to_string(),
                ..default()
            }),
            FpsOverlayPlugin {
                config: FpsOverlayConfig {
                    text_config: TextFont::from_font_size(20.0),
                    text_color: Color::srgb(0.0, 1.0, 0.0),
                    frame_time_graph_config: FrameTimeGraphConfig {
                        enabled: true,
                        target_fps: 60.0,
                        ..default()
                    },
                    ..default()
                },
            },
        ))
        .add_plugins((
            DebugOverlayPlugin,
            LevelPlugin,
            AssetLoadingPlugin,
            TimeControlsPlugin,
            MessagingPlugin,
            DisplayPlugin,
            TrainPlugin,
        ))
        .add_systems(Startup, camera_setup)
        .add_systems(OnExit(LoadingState::Loading), setup)
        .add_systems(Update, block_updates.run_if(in_state(LoadingState::Loaded)))
        .run();
}

fn camera_setup(
    mut commands: Commands,
    mut window: Single<&mut Window, With<PrimaryWindow>>,
) {
    window.title = "Rail Dispatch".to_string();
    commands.spawn(Camera2d);
}

fn setup(
    mut commands: Commands,
    handles: Res<AssetHandles>,
    levels: Res<Assets<Level>>,
) {
    let level = levels.get(&handles.level).unwrap();
    commands.insert_resource(BlockMap::from_level(level));
}

fn block_updates(
    mut block_map: ResMut<BlockMap>,
    mut block_updates: MessageReader<BlockUpdate>,
    mut lamp_updates: MessageWriter<LampUpdate>,
) {
    block_map.process_updates(&mut block_updates, &mut lamp_updates);
}
