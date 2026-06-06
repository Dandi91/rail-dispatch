use bevy::asset::AssetPlugin;
use bevy::prelude::*;
use bevy::window::ExitCondition;
use rail_dispatch::assets::AssetLoadingPlugin;
use rail_dispatch::level::LevelPlugin;
use rail_dispatch::panel::{CameraControlPlugin, SchematicPlugin};

fn main() {
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(AssetPlugin {
                    file_path: "resources".to_string(),
                    ..default()
                })
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Rail Dispatch — Map Viewer".to_string(),
                        ..default()
                    }),
                    exit_condition: ExitCondition::OnPrimaryClosed,
                    ..default()
                }),
        )
        .add_plugins((LevelPlugin, AssetLoadingPlugin, SchematicPlugin, CameraControlPlugin))
        .run();
}
