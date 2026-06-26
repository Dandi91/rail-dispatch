use bevy::asset::AssetPlugin;
use bevy::prelude::*;
use bevy::window::ExitCondition;
use rail_dispatch::assets::{AssetLoadingPlugin, LoadingState};
use rail_dispatch::level::LevelPlugin;
use rail_dispatch::panel::{CameraControlPlugin, ScreenScalePlugin};

mod constraints;
mod interaction;
mod render;
mod save;
mod state;
mod status;

/// Interactive map editor: create blocks, connect them, drag geometry, save to `level.toml`.
///
/// Renders its own editable layer (not the static [`SchematicPlugin`]) over the loaded level,
/// reusing the panel's camera, screen-constant scaling, and pixel↔world transform.
struct EditorPlugin;

impl Plugin for EditorPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((ScreenScalePlugin, CameraControlPlugin))
            .add_systems(Startup, render::setup_render_assets)
            .add_systems(
                OnExit(LoadingState::Loading),
                (state::init_model, render::setup_guides, status::setup_status_bar).chain(),
            )
            .add_systems(
                Update,
                (
                    interaction::update_hover,
                    interaction::handle_drag,
                    interaction::handle_delete,
                    interaction::handle_new_block,
                    save::save_system,
                    render::redraw,
                    render::highlight_vertices,
                    render::update_ghost,
                    status::update_status_bar,
                )
                    .chain()
                    .run_if(resource_exists::<state::EditorModel>),
            );
    }
}

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
