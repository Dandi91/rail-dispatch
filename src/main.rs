use bevy::asset::AssetPlugin;
use bevy::gilrs::GilrsPlugin;
use bevy::picking::mesh_picking::MeshPickingPlugin;
use bevy::prelude::*;
use bevy::window::ExitCondition;
use rail_dispatch::assets::AssetLoadingPlugin;
use rail_dispatch::audio::AudioPlugin;
use rail_dispatch::dropdown_menu::DropdownPlugin;
use rail_dispatch::fps_counter::FpsCounterPlugin;
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
                })
                // Keyboard and mouse only: skip gamepad discovery.
                .disable::<GilrsPlugin>(),
            FpsCounterPlugin,
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
