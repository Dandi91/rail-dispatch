mod editor;
mod handles;
mod save;
mod sidebar;
mod text_input;

use bevy::asset::AssetPlugin;
use bevy::prelude::*;
use bevy::window::ExitCondition;
use rail_dispatch::assets::AssetLoadingPlugin;
use rail_dispatch::level::LevelPlugin;

use editor::EditorPlugin;

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
                        title: "Rail Dispatch — Map Editor".to_string(),
                        ..default()
                    }),
                    exit_condition: ExitCondition::OnPrimaryClosed,
                    ..default()
                }),
        )
        .add_plugins((LevelPlugin, AssetLoadingPlugin, EditorPlugin))
        .run();
}
