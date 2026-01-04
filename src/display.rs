use crate::assets::{AssetHandles, LoadingState};
use crate::common::LampId;
use crate::debug_overlay::UpdateObservers;
use crate::level::{LampData, Level};
use crate::simulation::messages::{LampUpdate, LampUpdateState};
use bevy::prelude::*;
use bevy::sprite::Anchor;
use std::collections::HashMap;

pub const DEFAULT_LAMP_HEIGHT: f32 = 7.0;
const LAMP_COLOR_GRAY: Color = Color::srgba_u8(0x55, 0x55, 0x55, 0xFF);
const LAMP_COLOR_YELLOW: Color = Color::srgba_u8(0xFF, 0xFF, 0x40, 0xFF);
const LAMP_COLOR_RED: Color = Color::srgba_u8(0xFF, 0x20, 0x20, 0xFF);
const LAMP_COLOR_GREEN: Color = Color::srgba_u8(0x00, 0xFF, 0x00, 0xFF);

#[derive(Bundle)]
struct LampBundle {
    lamp: Lamp,
    transform: Transform,
    sprite: Sprite,
}

impl From<&LampData> for LampBundle {
    fn from(value: &LampData) -> Self {
        Self::from(Lamp::from(value))
    }
}

impl From<Lamp> for LampBundle {
    fn from(lamp: Lamp) -> Self {
        let transform = Transform::from_translation(lamp.position.extend(-1.0))
            .with_rotation(Quat::from_rotation_z(lamp.rotation.to_radians()));
        let sprite_size = lamp.size;
        Self {
            lamp,
            transform,
            sprite: Sprite::from_color(LAMP_COLOR_GRAY, sprite_size),
        }
    }
}

#[derive(Component)]
#[require(Pickable, Anchor::TOP_LEFT)]
pub struct Lamp {
    pub id: LampId,
    position: Vec2,
    size: Vec2,
    rotation: f32,
}

impl Lamp {
    fn get_base_color(&self) -> Color {
        if self.id >= 100 {
            LAMP_COLOR_GREEN
        } else {
            LAMP_COLOR_RED
        }
    }

    fn get_color(&self, state: LampUpdateState) -> Color {
        match state {
            LampUpdateState::On => self.get_base_color(),
            LampUpdateState::Off => LAMP_COLOR_GRAY,
            LampUpdateState::Pending => LAMP_COLOR_YELLOW,
        }
    }
}

impl From<&LampData> for Lamp {
    fn from(lamp: &LampData) -> Self {
        Self {
            id: lamp.id,
            position: vec2(lamp.x, -lamp.y),
            size: vec2(lamp.width, DEFAULT_LAMP_HEIGHT),
            rotation: lamp.rotation,
        }
    }
}

#[derive(Resource, Deref, DerefMut, Default)]
struct LampMapper(HashMap<LampId, Entity>);

#[derive(Component, Default)]
#[require(Transform, Anchor::TOP_LEFT)]
struct DisplayBoard;

#[derive(Bundle)]
struct DisplayBoardBundle {
    display_board: DisplayBoard,
    sprite: Sprite,
}

impl DisplayBoardBundle {
    fn new(image: Handle<Image>) -> Self {
        Self {
            display_board: Default::default(),
            sprite: Sprite::from(image),
        }
    }
}

pub struct DisplayPlugin;

impl Plugin for DisplayPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LampMapper>()
            .add_systems(Startup, startup)
            .add_systems(OnExit(LoadingState::Loading), setup)
            .add_systems(Update, lamp_updates);
    }
}

fn startup(mut commands: Commands) {
    commands.spawn(Camera2d);
}

fn setup(
    handles: Res<AssetHandles>,
    levels: Res<Assets<Level>>,
    images: Res<Assets<Image>>,
    mut camera_transform: Single<&mut Transform, With<Camera2d>>,
    mut clear_color: ResMut<ClearColor>,
    mut mapper: ResMut<LampMapper>,
    mut commands: Commands,
) {
    let level = levels.get(&handles.level).expect("assets had been loaded");
    let board_size = images.get(&handles.board).expect("assets had been loaded").size_f32();
    *clear_color = ClearColor(level.background.into());
    camera_transform.translation = (board_size * Anchor::BOTTOM_RIGHT.as_vec()).extend(0.0);

    commands.spawn(DisplayBoardBundle::new(handles.board.clone()));
    for lamp in &level.lamps {
        let entity = commands.spawn(LampBundle::from(lamp)).id();
        mapper.insert(lamp.id, entity);
    }

    commands.trigger(UpdateObservers);
}

fn lamp_updates(
    mut lamp_updates: MessageReader<LampUpdate>,
    mut query: Query<(&mut Sprite, &Lamp)>,
    lamp_mapper: Res<LampMapper>,
) {
    for update in lamp_updates.read() {
        if let Some(&entity) = lamp_mapper.get(&update.lamp_id) {
            let (mut sprite, lamp) = query.get_mut(entity).expect("invalid lamp entity");
            sprite.color = lamp.get_color(update.state);
        }
    }
}
