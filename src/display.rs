use crate::assets::{AssetHandles, LoadingState};
use crate::common::{BlockId, LampId};
use crate::debug_overlay::UpdateDebugObservers;
use crate::dropdown_menu::DropDownMenu;
use crate::level::{LampData, Level, SpawnerData, SpawnerKind};
use crate::simulation::messages::{LampUpdate, LampUpdateState};
use bevy::prelude::*;
use bevy::sprite::Anchor;
use bevy::ui::FocusPolicy;
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
#[require(Transform, Pickable::IGNORE, Anchor::TOP_LEFT)]
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
            sprite: image.into(),
        }
    }
}

#[derive(Component)]
#[require(Node, Button, BackgroundColor(Color::WHITE))]
struct Spawner {
    block_id: BlockId,
    kind: SpawnerKind,
}

#[derive(Bundle)]
struct SpawnerBundle {
    spawner: Spawner,
    transform: Transform,
}

const SPAWNER_SIZE: Vec2 = vec2(28.0, 14.0);

impl SpawnerBundle {
    fn new(data: &SpawnerData) -> Self {
        Self {
            spawner: Spawner {
                block_id: data.block_id,
                kind: data.kind,
            },
            transform: Transform::from_translation(vec3(data.x, -data.y, 10.0)),
        }
    }

    fn spawn(commands: &mut Commands, data: &SpawnerData) {
        commands.spawn(SpawnerBundle::new(data)).with_child((
            Sprite::from_color(LAMP_COLOR_GRAY, SPAWNER_SIZE),
            Anchor::TOP_LEFT,
            Pickable::default(),
        ));
    }
}

#[derive(EntityEvent)]
struct LampMenuEvent {
    entity: Entity,
    action: LampMenu,
}

#[derive(Component, Clone, Copy)]
enum LampMenu {
    SpawnTrain,
    DebugOn,
    DebugOff,
}

impl DropDownMenu for LampMenu {
    type Event<'a> = LampMenuEvent;

    fn create_event(&self, entity: Entity) -> Self::Event<'_> {
        LampMenuEvent { entity, action: *self }
    }

    fn get_label(&self) -> impl Into<String> {
        match self {
            LampMenu::SpawnTrain => "Spawn Train",
            LampMenu::DebugOn => "Debug Switch Lamp On",
            LampMenu::DebugOff => "Debug Switch Lamp Off",
        }
    }

    fn list_available_items() -> impl IntoIterator<Item = Self> {
        vec![LampMenu::SpawnTrain, LampMenu::DebugOn, LampMenu::DebugOff]
    }
}

pub struct DisplayPlugin;

impl Plugin for DisplayPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LampMapper>()
            .add_systems(Startup, startup)
            .add_systems(OnExit(LoadingState::Loading), setup)
            .add_systems(Update, (lamp_updates, update_spawners));
    }
}

fn startup(mut commands: Commands) {
    commands.spawn(Camera2d);
    commands.add_observer(on_signal_action);
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
    LampMenu::register(&mut commands, mapper.values().cloned());
    commands.trigger(UpdateDebugObservers);

    for spawner in &level.spawners {
        if matches!(spawner.kind, SpawnerKind::Despawn) {
            continue;
        };
        SpawnerBundle::spawn(&mut commands, spawner);
    }
}

fn on_signal_action(event: On<LampMenuEvent>, query: Query<&Lamp>, mut lamp_updates: MessageWriter<LampUpdate>) {
    if let Ok(lamp) = query.get(event.entity) {
        match event.action {
            LampMenu::SpawnTrain => {}
            LampMenu::DebugOn => {
                lamp_updates.write(LampUpdate::on(lamp.id));
            }
            LampMenu::DebugOff => {
                lamp_updates.write(LampUpdate::off(lamp.id));
            }
        }
        info!(
            "Used '{}' on lamp ID {} ({:?})",
            event.action.get_label().into(),
            lamp.id,
            event.entity
        );
    }
}

fn update_spawners(query: Query<(&Interaction, &Spawner), Changed<Interaction>>) {
    for (interaction, spawner) in query {
        info!("Update spawner {}, interaction {:?}", spawner.block_id, interaction);
        match interaction {
            Interaction::Pressed => {
                info!("Spawn train on block {}", spawner.block_id);
            }
            Interaction::Hovered => {
                // sprite.color = Color::WHITE;
            }
            Interaction::None => {
                // sprite.color = LAMP_COLOR_GRAY;
            }
        }
    }
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
