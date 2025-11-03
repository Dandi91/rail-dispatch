// mod clock;
mod common;
// mod consts;
mod display;
// mod event;
// mod game_state;
mod assets;
mod level;
// mod simulation;

use crate::assets::{AssetHandles, AssetLoadingPlugin, LoadingState};
use crate::display::lamp::{LAMP_COLOR_GRAY, LAMP_COLOR_RED};
use crate::level::{Level, LevelPlugin};
use bevy::asset::AssetPlugin;
use bevy::dev_tools::fps_overlay::{FpsOverlayConfig, FpsOverlayPlugin, FrameTimeGraphConfig};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

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
        .add_plugins((LevelPlugin, AssetLoadingPlugin))
        .add_systems(OnExit(LoadingState::Loading), setup)
        .run();
}

fn setup(
    mut commands: Commands,
    mut window: Single<&mut Window, With<PrimaryWindow>>,
    handles: Res<AssetHandles>,
    images: Res<Assets<Image>>,
    levels: Res<Assets<Level>>,
) {
    window.title = "Rail Dispatch".to_string();

    commands.spawn(Camera2d);

    let board = handles.board.clone();
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

    let level = levels.get(&handles.level).unwrap();
    for lamp in level.lamps.iter() {
        let size = Vec2::new(lamp.width, lamp.height);
        let pos = to_world_space(Vec2::new(lamp.x, -lamp.y - 1.0), size, window.size());
        commands.spawn((
            Sprite {
                image: handles.lamp.clone(),
                color: lamp.get_color(false),
                image_mode: SpriteImageMode::Sliced(TextureSlicer {
                    border: BorderRect::axes(3.0, 2.0),
                    ..default()
                }),
                custom_size: Some(size),
                ..default()
            },
            Transform {
                translation: pos.extend(1.0),
                ..default()
            },
        ));
    }
}

pub fn to_world_space(pos: Vec2, size: Vec2, window_size: Vec2) -> Vec2 {
    (window_size - size) * Vec2::new(-0.5, 0.5) + pos
}
