mod assets;
mod common;
mod debug_overlay;
mod display;
mod level;
mod simulation;
mod time_controls;

use crate::simulation::block::MapPlugin;
use crate::simulation::train::TrainPlugin;
use assets::AssetLoadingPlugin;
use bevy::asset::AssetPlugin;
use bevy::dev_tools::fps_overlay::{FpsOverlayConfig, FpsOverlayPlugin, FrameTimeGraphConfig};
use bevy::prelude::*;
use debug_overlay::DebugOverlayPlugin;
use display::DisplayPlugin;
use level::LevelPlugin;
use simulation::messages::MessagingPlugin;
use time_controls::TimeControlsPlugin;

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins
                .set(AssetPlugin {
                    file_path: "resources".to_string(),
                    ..default()
                })
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Rail Dispatch".to_string(),
                        ..default()
                    }),
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
        .run();
}
