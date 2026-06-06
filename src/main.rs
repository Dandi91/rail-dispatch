use bevy::asset::AssetPlugin;
use bevy::dev_tools::fps_overlay::{FpsOverlayConfig, FpsOverlayPlugin, FrameTimeGraphConfig};
use bevy::picking::mesh_picking::MeshPickingPlugin;
use bevy::prelude::*;
use bevy::window::ExitCondition;
use rail_dispatch::assets::AssetLoadingPlugin;
use rail_dispatch::audio::AudioPlugin;
use rail_dispatch::dropdown_menu::DropdownPlugin;
use rail_dispatch::level::LevelPlugin;
use rail_dispatch::panel::PanelPlugin;
use rail_dispatch::simulation::block::MapPlugin;
use rail_dispatch::simulation::spawner::SpawnerPlugin;
use rail_dispatch::simulation::station::StationPlugin;
use rail_dispatch::simulation::train::TrainPlugin;
use rail_dispatch::time_controls::TimeControlsPlugin;

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
                    exit_condition: ExitCondition::OnPrimaryClosed,
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
            MeshPickingPlugin,
            DropdownPlugin,
            LevelPlugin,
            AssetLoadingPlugin,
            TimeControlsPlugin,
            PanelPlugin,
            AudioPlugin,
            TrainPlugin,
            SpawnerPlugin,
            MapPlugin,
            StationPlugin,
        ))
        .run();
}
