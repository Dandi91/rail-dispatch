use crate::assets::{AssetHandles, LoadingState};
use crate::common::LampId;
use crate::level::{LampData, Level};
use crate::simulation::messages::{LampUpdate, LampUpdateState};
use bevy::prelude::*;
use bevy::sprite::Anchor;
use std::collections::HashMap;

pub const DEFAULT_LAMP_HEIGHT: f32 = 5.0;
const LAMP_COLOR_GRAY: Color = Color::srgba_u8(0x55, 0x55, 0x55, 0xFF);
const LAMP_COLOR_YELLOW: Color = Color::srgba_u8(0xFF, 0xFF, 0x40, 0xFF);
const LAMP_COLOR_RED: Color = Color::srgba_u8(0xFF, 0x20, 0x20, 0xFF);
const LAMP_COLOR_GREEN: Color = Color::srgba_u8(0x00, 0xFF, 0x00, 0xFF);

#[derive(Bundle)]
struct LampBundle {
    lamp: Lamp,
    transform: Transform,
    sprite: Sprite,
    anchor: Anchor,
}

impl LampBundle {
    fn new(lamp: Lamp, image: Handle<Image>) -> Self {
        let transform = Transform::from_translation(lamp.position.extend(1.0));
        let color = lamp.get_initial_color();
        let sprite_size = Some(lamp.size);
        Self {
            lamp,
            transform,
            anchor: Anchor::TOP_LEFT,
            sprite: Sprite {
                color,
                image,
                custom_size: sprite_size,
                image_mode: SpriteImageMode::Sliced(TextureSlicer {
                    border: BorderRect::axes(2.0, 2.0),
                    ..default()
                }),
                ..default()
            },
        }
    }
}

#[derive(Component)]
#[require(Pickable)]
pub struct Lamp {
    pub id: LampId,
    position: Vec2,
    size: Vec2,
}

impl Lamp {
    fn get_initial_color(&self) -> Color {
        if self.id >= 100 {
            self.get_base_color()
        } else {
            LAMP_COLOR_GRAY
        }
    }

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
            LampUpdateState::Pending => unimplemented!(),
        }
    }
}

impl From<&LampData> for Lamp {
    fn from(lamp: &LampData) -> Self {
        Self {
            id: lamp.id,
            position: vec2(lamp.x, -lamp.y),
            size: vec2(lamp.width, lamp.height),
        }
    }
}

#[derive(Resource, Deref, DerefMut, Default)]
struct LampMapper(HashMap<LampId, Entity>);

pub struct LampPlugin;

impl Plugin for LampPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnExit(LoadingState::Loading), setup)
            .add_systems(Update, lamp_updates);
    }
}

fn setup(handles: Res<AssetHandles>, levels: Res<Assets<Level>>, mut commands: Commands) {
    let level = levels.get(&handles.level).unwrap();

    let mut lamp_mapper = LampMapper::default();
    for lamp in &level.lamps {
        let bundle = LampBundle::new(lamp.into(), handles.lamp.clone());
        let entity = commands.spawn(bundle).id();
        lamp_mapper.insert(lamp.id, entity);
    }
    commands.insert_resource(lamp_mapper);
}

fn lamp_updates(
    mut lamp_updates: MessageReader<LampUpdate>,
    mut query: Query<(&mut Sprite, &Lamp)>,
    lamp_mapper: If<Res<LampMapper>>,
) {
    for update in lamp_updates.read() {
        let entity = lamp_mapper[&update.lamp_id];
        let (mut sprite, lamp) = query.get_mut(entity).unwrap();
        sprite.color = lamp.get_color(update.state);
    }
}
