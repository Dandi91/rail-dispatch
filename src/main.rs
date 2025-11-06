// mod clock;
mod common;
// mod consts;
mod display;
// mod event;
// mod game_state;
mod assets;
mod level;
mod simulation;
// pub mod signal;
// pub mod speed_table;
mod time_controls;

use crate::assets::{AssetHandles, AssetLoadingPlugin, LoadingState};
use crate::display::lamp::{LAMP_COLOR_GRAY, LAMP_COLOR_RED, LampId};
use crate::level::{Level, LevelPlugin};
use crate::simulation::block::BlockMap;
use crate::simulation::train::{NextTrainId, Train, spawn_train};
use crate::simulation::updates::UpdateQueues;
use crate::time_controls::TimeControlsPlugin;
use bevy::asset::AssetPlugin;
use bevy::dev_tools::fps_overlay::{FpsOverlayConfig, FpsOverlayPlugin, FrameTimeGraphConfig};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use std::collections::HashMap;

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
                        target_fps: 60.0,
                        ..default()
                    },
                    ..default()
                },
            },
        ))
        .add_plugins((LevelPlugin, AssetLoadingPlugin, TimeControlsPlugin))
        .add_systems(OnExit(LoadingState::Loading), setup)
        .add_systems(
            Update,
            (keyboard_handling, block_updates).run_if(in_state(LoadingState::Loaded)),
        )
        .add_systems(FixedUpdate, update.run_if(in_state(LoadingState::Loaded)))
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
            translation: to_world_space(Vec2::ZERO, size, window.size()).extend(0.0),
            scale: Vec3::ONE / scale,
            ..default()
        },
    ));

    let level = levels.get(&handles.level).unwrap();
    commands.insert_resource(BlockMap::from_level(level));
    commands.insert_resource(NextTrainId::new());
    commands.insert_resource(UpdateQueues::new());

    let mut lamp_mapper = LampMapper(HashMap::new());
    for lamp in level.lamps.iter() {
        let size = Vec2::new(lamp.width, lamp.height);
        let pos = to_world_space(Vec2::new(lamp.x, -lamp.y - 1.0), size, window.size());
        let entity = commands
            .spawn((
                Lamp,
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
            ))
            .id();
        lamp_mapper.insert(lamp.id, entity);
    }

    commands.insert_resource(lamp_mapper);
}

#[derive(Component)]
struct Lamp;

#[derive(Resource, Deref, DerefMut)]
struct LampMapper(HashMap<LampId, Entity>);

fn to_world_space(pos: Vec2, size: Vec2, window_size: Vec2) -> Vec2 {
    (window_size - size) * Vec2::new(-0.5, 0.5) + pos
}

fn keyboard_handling(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    block_map: Res<BlockMap>,
    mut train_id: ResMut<NextTrainId>,
    mut update_queues: ResMut<UpdateQueues>,
    mut commands: Commands,
) {
    if keyboard_input.just_pressed(KeyCode::KeyG) {
        let train = spawn_train(train_id.next(), &block_map, &mut update_queues);
        info!("Train {} spawned with ID {}", train.number, train.id);
        commands.spawn(train);
    }
}

fn update(
    query: Query<&mut Train>,
    time: Res<Time>,
    block_map: Res<BlockMap>,
    mut update_queues: ResMut<UpdateQueues>,
) {
    for mut train in query {
        train.update(time.delta_secs_f64(), &block_map, &mut update_queues.block_updates);
    }
}

fn block_updates(
    mut query: Query<&mut Sprite, With<Lamp>>,
    lamp_mapper: Res<LampMapper>,
    mut block_map: ResMut<BlockMap>,
    mut update_queues: ResMut<UpdateQueues>,
) {
    block_map
        .process_updates(&mut update_queues.block_updates)
        .for_each(|(lamp_id, state)| {
            let color = if state { LAMP_COLOR_RED } else { LAMP_COLOR_GRAY };
            let entity = lamp_mapper[&lamp_id];
            query.get_mut(entity).unwrap().color = color;
        })
}
