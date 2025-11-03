// mod clock;
mod common;
// mod consts;
mod display;
// mod event;
// mod game_state;
mod level;
mod assets;
// mod simulation;

use bevy::dev_tools::fps_overlay::{FpsOverlayConfig, FpsOverlayPlugin, FrameTimeGraphConfig};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_asset::AssetPlugin;
use crate::assets::{AssetLoadingPlugin, LoadingHandles, LoadingState};

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
                        enabled: false,
                        ..default()
                    },
                    ..default()
                },
            },
        ))
        .add_plugins(AssetLoadingPlugin)
        .add_systems(OnExit(LoadingState::Loading), setup)
        .run();
}

fn setup(
    mut commands: Commands,
    mut window: Single<&mut Window, With<PrimaryWindow>>,
    image_handles: Res<LoadingHandles>,
    images: Res<Assets<Image>>,
) {
    window.title = "Rail Dispatch".to_string();

    commands.spawn(Camera2d);

    let board = image_handles.board_handle.clone();
    let scale = 2.0;
    let size = images.get(&board).unwrap().size_f32() / scale;
    commands.spawn((
        Sprite::from(board),
        Transform {
            translation: ((window.size() - size) * Vec2::new(-0.5, 0.5)).extend(0.0),
            scale: Vec3::ONE / scale,
            ..default()
        },
    ));

    commands.spawn((
        Sprite::from_color(Color::WHITE, Vec2::ONE),
        Transform {
            translation: Vec3::new(0.0, 0.0, 1.0),
            scale: Vec3::new(120.0, 20.0, 1.0),
            ..default()
        },
    ));
}
