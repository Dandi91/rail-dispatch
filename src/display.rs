use crate::assets::{AssetHandles, LoadingState};
use crate::common::{BlockId, LampId};
use crate::debug_overlay::UpdateDebugObservers;
use crate::dropdown_menu::DropDownMenu;
use crate::level::{LampData, Level, SpawnerData, SpawnerKind};
use crate::simulation::messages::{LampUpdate, LampUpdateState};
use bevy::ecs::system::entity_command::observe;
use bevy::prelude::*;
use std::collections::HashMap;

pub const DEFAULT_LAMP_HEIGHT: f32 = 7.0;
const LAMP_COLOR_GRAY: Color = Color::srgba_u8(0x55, 0x55, 0x55, 0xFF);
const LAMP_COLOR_YELLOW: Color = Color::srgba_u8(0xFF, 0xFF, 0x40, 0xFF);
const LAMP_COLOR_RED: Color = Color::srgba_u8(0xFF, 0x20, 0x20, 0xFF);
const LAMP_COLOR_GREEN: Color = Color::srgba_u8(0x00, 0xFF, 0x00, 0xFF);

#[derive(Component)]
#[require(Pickable)]
pub struct Lamp(pub LampId);

fn get_lamp_bundle(lamp: &LampData) -> impl Bundle {
    let rotation = if lamp.rotation != 0.0 {
        Rot2::degrees(lamp.rotation)
    } else {
        Rot2::IDENTITY
    };

    (
        Lamp(lamp.id),
        UiTransform { rotation, ..default() },
        Node {
            position_type: PositionType::Absolute,
            left: px(lamp.x),
            top: px(lamp.y),
            width: px(lamp.width),
            height: px(DEFAULT_LAMP_HEIGHT),
            ..default()
        },
        BackgroundColor(LAMP_COLOR_GRAY),
    )
}

impl Lamp {
    fn get_base_color(&self) -> Color {
        if self.0 >= 100 {
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

#[derive(Resource, Deref, DerefMut, Default)]
struct LampMapper(HashMap<LampId, Entity>);

#[derive(Component, Default)]
#[require(Pickable::IGNORE)]
struct DisplayBoard;

fn get_board_bundle(board: Handle<Image>) -> impl Bundle {
    (DisplayBoard::default(), ImageNode::new(board), ZIndex(1))
}

#[derive(Component)]
#[require(Pickable)]
struct Spawner {
    block_id: BlockId,
    kind: SpawnerKind,
}

const SPAWNER_SIZE: Val2 = Val2::px(28.0, 14.0);

fn get_spawner_bundle(data: &SpawnerData) -> impl Bundle {
    (
        Spawner {
            block_id: data.block_id,
            kind: data.kind,
        },
        Node {
            position_type: PositionType::Absolute,
            left: px(data.x),
            top: px(data.y),
            width: SPAWNER_SIZE.x,
            height: SPAWNER_SIZE.y,
            ..default()
        },
        BackgroundColor(Color::WHITE),
        Interaction::None,
    )
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
    mut clear_color: ResMut<ClearColor>,
    mut mapper: ResMut<LampMapper>,
    mut commands: Commands,
) {
    let level = levels.get(&handles.level).expect("assets had been loaded");
    *clear_color = ClearColor(level.background.into());

    commands
        .spawn(Node {
            width: vw(100.0),
            height: vh(100.0),
            align_items: AlignItems::Center,
            align_content: AlignContent::Center,
            justify_content: JustifyContent::Center,
            ..default()
        })
        .with_children(|p| {
            p.spawn(Node {
                align_items: AlignItems::Center,
                align_content: AlignContent::Center,
                justify_content: JustifyContent::Center,
                ..default()
            })
            .with_children(|p| {
                p.spawn(get_board_bundle(handles.board.clone()));
                for lamp in &level.lamps {
                    let entity = p.spawn(get_lamp_bundle(lamp)).id();
                    mapper.insert(lamp.id, entity);
                }
                for spawner in &level.spawners {
                    if spawner.kind != SpawnerKind::Despawn {
                        p.spawn(get_spawner_bundle(spawner));
                    }
                }
            });
        });

    LampMenu::register(&mut commands, mapper.values().cloned());
    commands.trigger(UpdateDebugObservers);
}

fn on_signal_action(event: On<LampMenuEvent>, query: Query<&Lamp>, mut lamp_updates: MessageWriter<LampUpdate>) {
    if let Ok(lamp) = query.get(event.entity) {
        match event.action {
            LampMenu::SpawnTrain => {}
            LampMenu::DebugOn => {
                lamp_updates.write(LampUpdate::on(lamp.0));
            }
            LampMenu::DebugOff => {
                lamp_updates.write(LampUpdate::off(lamp.0));
            }
        }
        info!(
            "Used '{}' on lamp ID {} ({:?})",
            event.action.get_label().into(),
            lamp.0,
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
    mut query: Query<(&mut BackgroundColor, &Lamp)>,
    lamp_mapper: Res<LampMapper>,
) {
    for update in lamp_updates.read() {
        if let Some(&entity) = lamp_mapper.get(&update.lamp_id) {
            let (mut color, lamp) = query.get_mut(entity).expect("invalid lamp entity");
            *color = lamp.get_color(update.state).into();
        }
    }
}
