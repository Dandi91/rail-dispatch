// mod clock;
mod common;
mod display;
// mod game_state;
mod assets;
mod debug_overlay;
mod level;
mod simulation;
mod time_controls;

use crate::simulation::block::MapPlugin;
use crate::simulation::train::TrainPlugin;
use assets::AssetLoadingPlugin;
use bevy::asset::AssetPlugin;
use bevy::dev_tools::fps_overlay::{FpsOverlayConfig, FpsOverlayPlugin, FrameTimeGraphConfig};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use debug_overlay::DebugOverlayPlugin;
use display::DisplayPlugin;
use level::LevelPlugin;
use simulation::messages::MessagingPlugin;
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
            MapPlugin,
        ))
        .add_systems(Startup, camera_setup)
        .run();
}

fn camera_setup(mut commands: Commands, mut window: Single<&mut Window, With<PrimaryWindow>>) {
    window.title = "Rail Dispatch".to_string();
    commands.spawn(Camera2d);
}
