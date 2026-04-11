use crate::assets::{AssetHandles, LoadingState};
use crate::common::{BlockId, LampId, RouteId};
use crate::dropdown_menu::DropDownMenu;
use crate::level::{LampData, Level, SpawnerData, SpawnerKind};
use crate::simulation::block::{BlockMap, LampUpdate, LampUpdateState};
use crate::simulation::spawner::{SpawnRequest, SpawnTrainType};
use crate::simulation::station::RouteActivationRequest;
use crate::simulation::train::TrainDespawnRequest;
use bevy::ecs::system::{SystemParam, SystemParamItem};
use bevy::input::keyboard::Key;
use bevy::prelude::*;
use itertools::Itertools;
use std::collections::HashMap;

pub const DEFAULT_LAMP_HEIGHT: f32 = 7.0;
const LAMP_COLOR_GRAY: Color = Color::srgba_u8(0x55, 0x55, 0x55, 0xFF);
const LAMP_COLOR_YELLOW: Color = Color::srgba_u8(0xFF, 0xFF, 0x40, 0xFF);
const LAMP_COLOR_RED: Color = Color::srgba_u8(0xFF, 0x20, 0x20, 0xFF);
const LAMP_COLOR_GREEN: Color = Color::srgba_u8(0x00, 0xFF, 0x00, 0xFF);

#[derive(Component)]
enum LampKind {
    Block,
    Signal,
}

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
        Lamp(lamp.id).bundle(),
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
        match self.get_kind() {
            LampKind::Block => LAMP_COLOR_RED,
            LampKind::Signal => LAMP_COLOR_GREEN,
        }
    }

    fn get_kind(&self) -> LampKind {
        if self.0 >= 100 {
            LampKind::Signal
        } else {
            LampKind::Block
        }
    }

    fn get_color(&self, state: LampUpdateState) -> Color {
        match state {
            LampUpdateState::On => self.get_base_color(),
            LampUpdateState::Off => LAMP_COLOR_GRAY,
            LampUpdateState::Pending => LAMP_COLOR_YELLOW,
        }
    }

    fn bundle(self) -> impl Bundle {
        let kind = self.get_kind();
        (self, kind)
    }
}

#[derive(Resource, Deref, DerefMut, Default)]
struct LampMapper(HashMap<LampId, Entity>);

#[derive(Component, Default)]
#[require(Pickable::IGNORE)]
struct DisplayBoard;

fn get_board_bundle(board: Handle<Image>) -> impl Bundle {
    (DisplayBoard, ImageNode::new(board), ZIndex(1))
}

#[derive(Component)]
#[require(Pickable)]
struct SpawnerUI {
    block_id: BlockId,
}

const SPAWNER_SIZE: Val2 = Val2::px(28.0, 14.0);

fn get_spawner_bundle(data: &SpawnerData) -> impl Bundle {
    (
        SpawnerUI {
            block_id: data.block_id,
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
    )
}

#[derive(EntityEvent)]
struct LampMenuEvent {
    entity: Entity,
    action: LampMenu,
}

#[derive(Component, Clone, Copy)]
enum LampMenu {
    DebugOn,
    DebugOff,
    DespawnTrain,
}

impl DropDownMenu for LampMenu {
    type Event<'a> = LampMenuEvent;
    type Context = ();

    fn create_event(&self, entity: Entity) -> Self::Event<'_> {
        LampMenuEvent { entity, action: *self }
    }

    fn get_label(&self) -> impl Into<String> {
        match self {
            LampMenu::DebugOn => "Debug Switch Lamp On",
            LampMenu::DebugOff => "Debug Switch Lamp Off",
            LampMenu::DespawnTrain => "Debug Despawn Train in Block",
        }
    }

    fn list_available_items<'w, 's>(_: Entity, _: &mut ()) -> impl IntoIterator<Item = Self> {
        [LampMenu::DebugOn, LampMenu::DebugOff, LampMenu::DespawnTrain]
    }

    fn key_filter(keyboard_input: Res<ButtonInput<Key>>) -> bool {
        keyboard_input.pressed(Key::Control)
    }
}

#[derive(EntityEvent)]
struct SpawnerMenuEvent {
    entity: Entity,
    action: SpawnerMenu,
}

#[derive(Component, Clone, Copy)]
enum SpawnerMenu {
    SpawnCargo,
    SpawnPassenger,
    SpawnLocoOnly,
}

impl DropDownMenu for SpawnerMenu {
    type Event<'a> = SpawnerMenuEvent;
    type Context = ();

    fn create_event(&self, entity: Entity) -> Self::Event<'_> {
        SpawnerMenuEvent { entity, action: *self }
    }

    fn get_label(&self) -> impl Into<String> {
        match self {
            SpawnerMenu::SpawnCargo => "Spawn Cargo Train",
            SpawnerMenu::SpawnPassenger => "Spawn Passenger Train",
            SpawnerMenu::SpawnLocoOnly => "Spawn Locomotive Only",
        }
    }

    fn list_available_items<'w, 's>(_: Entity, _: &mut ()) -> impl IntoIterator<Item = Self> {
        [
            SpawnerMenu::SpawnCargo,
            SpawnerMenu::SpawnPassenger,
            SpawnerMenu::SpawnLocoOnly,
        ]
    }
}

#[derive(EntityEvent)]
struct SignalRouteMenuEvent {
    entity: Entity,
    route_id: RouteId,
}

#[derive(Component, Clone, Copy)]
struct SignalRouteMenu(RouteId);

#[derive(SystemParam)]
struct SignalRouteContext<'w, 's> {
    handles: Res<'w, AssetHandles>,
    levels: Res<'w, Assets<Level>>,
    lamps: Query<'w, 's, &'static Lamp>,
}

impl DropDownMenu for SignalRouteMenu {
    type Event<'a> = SignalRouteMenuEvent;
    type Context = SignalRouteContext<'static, 'static>;

    fn create_event(&self, entity: Entity) -> Self::Event<'_> {
        SignalRouteMenuEvent {
            entity,
            route_id: self.0,
        }
    }

    fn get_label(&self) -> impl Into<String> {
        format!("Open route {}", self.0)
    }

    fn list_available_items(
        target: Entity,
        ctx: &mut SystemParamItem<Self::Context>,
    ) -> impl IntoIterator<Item = Self> {
        let mut items = Vec::new();
        let Ok(lamp) = ctx.lamps.get(target) else {
            return items;
        };
        let Some(level) = ctx.levels.get(&ctx.handles.level) else {
            return items;
        };
        let Some(signal) = level.signals.iter().find(|s| s.lamp_id == lamp.0) else {
            return items;
        };
        for route in level.stations.iter().flat_map(|s| s.routes.iter()) {
            if route.signal == signal.id {
                items.push(SignalRouteMenu(route.id));
            }
        }
        items
    }

    fn key_filter(keyboard_input: Res<ButtonInput<Key>>) -> bool {
        !keyboard_input.pressed(Key::Control)
    }
}

#[derive(Event)]
pub struct LevelSetupComplete;

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
    commands.add_observer(on_signal_action);
    commands.add_observer(on_spawner_action);
    commands.add_observer(on_signal_route_action);
    commands.add_observer(on_setup_complete);
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

    let signal_lamp_entities: Vec<Entity> = mapper
        .iter()
        .filter(|&(&lamp_id, _)| lamp_id >= 100)
        .map(|(_, &entity)| entity)
        .collect();
    SignalRouteMenu::register(&mut commands, signal_lamp_entities);

    commands.trigger(LevelSetupComplete);
}

fn on_setup_complete(_: On<LevelSetupComplete>, spawners: Query<Entity, With<SpawnerUI>>, mut commands: Commands) {
    SpawnerMenu::register(&mut commands, spawners);
    commands.spawn(Observer::new(on_spawner_over).with_entities(spawners));
    commands.spawn(Observer::new(on_spawner_out).with_entities(spawners));
}

fn on_spawner_action(event: On<SpawnerMenuEvent>, query: Query<&SpawnerUI>, mut commands: Commands) {
    if let Ok(spawner) = query.get(event.entity) {
        let train_type = match event.action {
            SpawnerMenu::SpawnCargo => SpawnTrainType::Cargo,
            SpawnerMenu::SpawnPassenger => SpawnTrainType::Passenger,
            SpawnerMenu::SpawnLocoOnly => SpawnTrainType::Locomotive,
        };
        commands.trigger(SpawnRequest {
            block_id: spawner.block_id,
            train_type,
        });
        info!(
            "Requested {:?} train from spawner block {}",
            train_type, spawner.block_id
        );
    }
}

fn on_spawner_over(event: On<Pointer<Over>>, mut query: Query<&mut BackgroundColor, With<SpawnerUI>>) {
    if let Ok(mut color) = query.get_mut(event.entity) {
        *color = BackgroundColor(LAMP_COLOR_GREEN);
    }
}

fn on_spawner_out(event: On<Pointer<Out>>, mut query: Query<&mut BackgroundColor, With<SpawnerUI>>) {
    if let Ok(mut color) = query.get_mut(event.entity) {
        *color = BackgroundColor(Color::WHITE);
    }
}

fn on_signal_action(
    event: On<LampMenuEvent>,
    query: Query<&Lamp>,
    block_map: If<Res<BlockMap>>,
    mut lamp_updates: MessageWriter<LampUpdate>,
    mut despawn_requests: MessageWriter<TrainDespawnRequest>,
) {
    if let Ok(lamp) = query.get(event.entity) {
        match event.action {
            LampMenu::DebugOn => {
                lamp_updates.write(LampUpdate::on(lamp.0));
            }
            LampMenu::DebugOff => {
                lamp_updates.write(LampUpdate::off(lamp.0));
            }
            LampMenu::DespawnTrain => {
                if let (_, Some(trains)) = block_map.get_lamp_info(lamp.0) {
                    despawn_requests.write_batch(trains.iter().cloned().map_into());
                }
            }
        }
    }
}

fn on_signal_route_action(event: On<SignalRouteMenuEvent>, mut requests: MessageWriter<RouteActivationRequest>) {
    requests.write(RouteActivationRequest {
        route_id: event.route_id,
    });
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
