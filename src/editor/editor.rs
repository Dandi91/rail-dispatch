use bevy::input::mouse::{MouseMotion, MouseScrollUnit, MouseWheel};
use bevy::prelude::*;
use rail_dispatch::assets::{AssetHandles, LoadingState};
use rail_dispatch::common::LampId;
use rail_dispatch::display::{Lamp, LampKind, get_lamp_bundle};
use rail_dispatch::level::{LampData, Level};
use std::collections::HashMap;

use crate::handles::HandlesPlugin;
use crate::sidebar::SidebarPlugin;

#[derive(Resource)]
pub struct EditorState {
    pub lamps: Vec<LampData>,
    pub selected: Option<LampId>,
    pub zoom: f32,
    pub pan: Vec2,
}

impl Default for EditorState {
    fn default() -> Self {
        Self {
            lamps: Vec::new(),
            selected: None,
            zoom: 1.0,
            pan: Vec2::ZERO,
        }
    }
}

#[derive(Resource, Default, Deref, DerefMut)]
struct LampEntities(HashMap<LampId, Entity>);

#[derive(Resource)]
pub struct CanvasRoot(pub Entity);

const ZOOM_MIN: f32 = 1.0;
const ZOOM_MAX: f32 = 8.0;
const ZOOM_STEP: f32 = 2.0;

#[derive(Message)]
pub struct RespawnLamp(pub LampId);

impl EditorState {
    pub fn get(&self, id: LampId) -> Option<&LampData> {
        self.lamps.iter().find(|l| l.id == id)
    }

    pub fn get_mut(&mut self, id: LampId) -> Option<&mut LampData> {
        self.lamps.iter_mut().find(|l| l.id == id)
    }

    pub fn next_block_id(&self) -> LampId {
        let used: Vec<LampId> = self.lamps.iter().map(|l| l.id).filter(|&id| id < 100).collect();
        (1..100).find(|id| !used.contains(id)).unwrap_or(1)
    }

    pub fn next_signal_id(&self) -> LampId {
        let used: Vec<LampId> = self.lamps.iter().map(|l| l.id).filter(|&id| id >= 100).collect();
        (100..).find(|id| !used.contains(id)).unwrap_or(100)
    }
}

#[derive(EntityEvent)]
pub struct SelectLamp {
    pub entity: Entity,
}

pub struct EditorPlugin;

impl Plugin for EditorPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<EditorState>()
            .init_resource::<LampEntities>()
            .add_message::<RespawnLamp>()
            .add_plugins((HandlesPlugin, SidebarPlugin))
            .add_systems(Startup, startup)
            .add_systems(OnExit(LoadingState::Loading), setup)
            .add_systems(
                Update,
                (zoom_input, pan_input, apply_canvas_transform, handle_respawn_lamp)
                    .run_if(in_state(LoadingState::Loaded)),
            );
    }
}

fn startup(mut commands: Commands) {
    commands.spawn(Camera2d);
    commands.add_observer(on_select_lamp);
}

fn setup(
    handles: Res<AssetHandles>,
    levels: Res<Assets<Level>>,
    mut clear_color: ResMut<ClearColor>,
    mut state: ResMut<EditorState>,
    mut lamp_entities: ResMut<LampEntities>,
    mut commands: Commands,
) {
    let level = levels.get(&handles.level).expect("assets had been loaded");
    *clear_color = ClearColor(level.background.into());
    state.lamps = level.lamps.clone();

    let mut canvas = Entity::PLACEHOLDER;
    commands
        .spawn(Node {
            width: vw(80.0),
            height: vh(100.0),
            align_items: AlignItems::Center,
            align_content: AlignContent::Center,
            justify_content: JustifyContent::Center,
            ..default()
        })
        .with_children(|p| {
            canvas = p
                .spawn((
                    Node {
                        align_items: AlignItems::Center,
                        align_content: AlignContent::Center,
                        justify_content: JustifyContent::Center,
                        ..default()
                    },
                    Pickable::default(),
                ))
                .observe(|event: On<Pointer<Click>>, mut state: ResMut<EditorState>| {
                    if event.button == PointerButton::Primary && event.original_event_target() == event.entity {
                        state.selected = None;
                    }
                })
                .with_children(|p| {
                    p.spawn((
                        Node { ..default() },
                        ImageNode::new(handles.board.clone()),
                        ZIndex(-1),
                        Pickable::IGNORE,
                    ));
                })
                .id();
        });
    commands.insert_resource(CanvasRoot(canvas));

    for lamp in &state.lamps {
        let entity = spawn_lamp_entity(&mut commands, canvas, lamp, &handles);
        lamp_entities.insert(lamp.id, entity);
    }
}

fn spawn_lamp_entity(commands: &mut Commands, canvas: Entity, lamp: &LampData, handles: &AssetHandles) -> Entity {
    let id = commands.spawn(get_lamp_bundle(lamp, handles)).id();
    commands.entity(canvas).add_child(id);
    commands
        .entity(id)
        .observe(|trigger: On<Pointer<Click>>, mut commands: Commands| {
            commands.trigger(SelectLamp { entity: trigger.entity });
        });
    id
}

fn handle_respawn_lamp(
    mut events: MessageReader<RespawnLamp>,
    state: Res<EditorState>,
    canvas_root: Res<CanvasRoot>,
    handles: Res<AssetHandles>,
    mut lamp_entities: ResMut<LampEntities>,
    mut commands: Commands,
) {
    for RespawnLamp(id) in events.read() {
        if let Some(old) = lamp_entities.remove(id) {
            commands.entity(old).despawn();
        }
        if let Some(lamp) = state.get(*id) {
            let entity = spawn_lamp_entity(&mut commands, canvas_root.0, lamp, &handles);
            lamp_entities.insert(*id, entity);
        }
    }
}

fn on_select_lamp(event: On<SelectLamp>, lamps: Query<&Lamp>, mut state: ResMut<EditorState>) {
    if let Ok(lamp) = lamps.get(event.entity) {
        state.selected = Some(lamp.0);
        info!("selected lamp {}", lamp.0);
    }
}

pub fn lamp_kind_for(id: LampId) -> LampKind {
    if id >= 100 { LampKind::Signal } else { LampKind::Block }
}

fn zoom_input(mut wheel: MessageReader<MouseWheel>, mut state: ResMut<EditorState>) {
    let mut dy = 0.0;
    for ev in wheel.read() {
        dy += match ev.unit {
            MouseScrollUnit::Line => ev.y,
            MouseScrollUnit::Pixel => ev.y / 40.0,
        };
    }
    if dy == 0.0 {
        return;
    }
    let factor = ZOOM_STEP.powf(dy);
    state.zoom = (state.zoom * factor).clamp(ZOOM_MIN, ZOOM_MAX);
}

fn pan_input(
    buttons: Res<ButtonInput<MouseButton>>,
    mut motion: MessageReader<MouseMotion>,
    mut state: ResMut<EditorState>,
) {
    if !buttons.pressed(MouseButton::Middle) {
        motion.clear();
        return;
    }
    let mut delta = Vec2::ZERO;
    for ev in motion.read() {
        delta += ev.delta;
    }
    if delta != Vec2::ZERO {
        state.pan += delta;
    }
}

fn apply_canvas_transform(state: Res<EditorState>, canvas: Option<Res<CanvasRoot>>, mut commands: Commands) {
    if !state.is_changed() {
        return;
    }
    let Some(canvas) = canvas else { return };
    commands.entity(canvas.0).insert(UiTransform {
        translation: Val2::px(state.pan.x, state.pan.y),
        scale: Vec2::splat(state.zoom),
        ..default()
    });
}
