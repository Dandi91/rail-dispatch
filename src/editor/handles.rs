use bevy::prelude::*;
use rail_dispatch::assets::LoadingState;
use rail_dispatch::common::LampId;

use crate::editor::{CanvasRoot, EditorState, RespawnLamp};

const HANDLE_SIZE: f32 = 8.0;
const WIDTH_HANDLE_OFFSET: f32 = 8.0;
const ROTATION_HANDLE_OFFSET: f32 = 16.0;
const COLOR_BODY: Color = Color::srgba(0.4, 0.7, 1.0, 0.35);
const COLOR_WIDTH: Color = Color::srgba(1.0, 0.9, 0.2, 0.9);
const COLOR_ROTATION: Color = Color::srgba(0.2, 1.0, 0.4, 0.9);

#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub enum HandleKind {
    Body,
    Width,
    Rotation,
}

#[derive(Component)]
pub struct EditorHandle {
    pub kind: HandleKind,
    pub lamp_id: LampId,
}

pub struct HandlesPlugin;

impl Plugin for HandlesPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, sync_handles.run_if(in_state(LoadingState::Loaded)));
    }
}

fn sync_handles(
    state: Res<EditorState>,
    canvas_root: Option<Res<CanvasRoot>>,
    handles_query: Query<(Entity, &EditorHandle)>,
    mut commands: Commands,
) {
    let Some(canvas_root) = canvas_root else { return };
    let Some(selected) = state.selected else {
        for (e, _) in handles_query.iter() {
            commands.entity(e).despawn();
        }
        return;
    };
    let Some(lamp) = state.get(selected) else { return };

    // Lamp center in canvas coords (rotation pivot for UiTransform is node center).
    let lamp_h = rail_dispatch::display::DEFAULT_LAMP_HEIGHT;
    let cx = lamp.x + lamp.width * 0.5;
    let cy = lamp.y + lamp_h * 0.5;
    let theta = lamp.rotation.to_radians();
    let (s, c) = theta.sin_cos();

    let body_node = Node {
        position_type: PositionType::Absolute,
        left: Val::Px(lamp.x),
        top: Val::Px(lamp.y),
        width: Val::Px(lamp.width),
        height: Val::Px(lamp_h),
        ..default()
    };
    let body_xform = UiTransform {
        rotation: Rot2::degrees(lamp.rotation),
        ..default()
    };

    let width_r = lamp.width * 0.5 + WIDTH_HANDLE_OFFSET;
    let width_node = Node {
        position_type: PositionType::Absolute,
        left: Val::Px(cx + width_r * c - HANDLE_SIZE * 0.5),
        top: Val::Px(cy + width_r * s - HANDLE_SIZE * 0.5),
        width: Val::Px(HANDLE_SIZE),
        height: Val::Px(HANDLE_SIZE),
        ..default()
    };
    let width_xform = UiTransform {
        rotation: Rot2::degrees(lamp.rotation + 45.0),
        ..default()
    };

    // Rotation handle: lamp-local offset (0, -R) → canvas (cx + R*sin θ, cy - R*cos θ).
    let rotation_node = Node {
        position_type: PositionType::Absolute,
        left: Val::Px(cx + ROTATION_HANDLE_OFFSET * s - HANDLE_SIZE * 0.5),
        top: Val::Px(cy - ROTATION_HANDLE_OFFSET * c - HANDLE_SIZE * 0.5),
        width: Val::Px(HANDLE_SIZE),
        height: Val::Px(HANDLE_SIZE),
        border_radius: BorderRadius::MAX,
        ..default()
    };

    // Update existing handles for the selected lamp; despawn any that belong to a different lamp.
    let mut have = [false; 3];
    for (e, h) in handles_query.iter() {
        if h.lamp_id != selected {
            commands.entity(e).despawn();
            continue;
        }
        match h.kind {
            HandleKind::Body => {
                have[0] = true;
                commands.entity(e).insert((body_node.clone(), body_xform));
            }
            HandleKind::Width => {
                have[1] = true;
                commands.entity(e).insert((width_node.clone(), width_xform));
            }
            HandleKind::Rotation => {
                have[2] = true;
                commands.entity(e).insert(rotation_node.clone());
            }
        }
    }

    // Spawn any missing handles already positioned, so they don't appear at (0,0) for one frame.
    if !have[0] {
        let body = commands
            .spawn((
                EditorHandle {
                    kind: HandleKind::Body,
                    lamp_id: selected,
                },
                body_node,
                body_xform,
                BackgroundColor(COLOR_BODY),
                Pickable::default(),
                ZIndex(50),
            ))
            .observe(on_drag)
            .id();
        commands.entity(canvas_root.0).add_child(body);
    }
    if !have[1] {
        let width = commands
            .spawn((
                EditorHandle {
                    kind: HandleKind::Width,
                    lamp_id: selected,
                },
                width_node,
                width_xform,
                BackgroundColor(COLOR_WIDTH),
                Pickable::default(),
                ZIndex(51),
            ))
            .observe(on_drag)
            .id();
        commands.entity(canvas_root.0).add_child(width);
    }
    if !have[2] {
        let rotation = commands
            .spawn((
                EditorHandle {
                    kind: HandleKind::Rotation,
                    lamp_id: selected,
                },
                rotation_node,
                BackgroundColor(COLOR_ROTATION),
                Pickable::default(),
                ZIndex(51),
            ))
            .observe(on_drag)
            .id();
        commands.entity(canvas_root.0).add_child(rotation);
    }
}

fn on_drag(
    event: On<Pointer<Drag>>,
    handles_q: Query<&EditorHandle>,
    mut state: ResMut<EditorState>,
    mut respawns: MessageWriter<RespawnLamp>,
) {
    if event.button != PointerButton::Primary {
        return;
    }
    let Ok(handle) = handles_q.get(event.entity) else {
        return;
    };
    let inv = 1.0 / state.zoom.max(0.0001);
    let Some(lamp) = state.get_mut(handle.lamp_id) else {
        return;
    };
    let dx = event.delta.x * inv;
    let dy = event.delta.y * inv;
    match handle.kind {
        HandleKind::Body => {
            lamp.x += dx;
            lamp.y += dy;
        }
        HandleKind::Width => {
            let theta = lamp.rotation.to_radians();
            let (s, c) = theta.sin_cos();
            let dw = dx * c + dy * s;
            lamp.width = (lamp.width + dw).max(4.0);
        }
        HandleKind::Rotation => {
            let theta = lamp.rotation.to_radians();
            let (s, c) = theta.sin_cos();
            let arc = dx * c + dy * s;
            lamp.rotation += arc.to_degrees() / ROTATION_HANDLE_OFFSET.max(1.0);
        }
    }
    respawns.write(RespawnLamp(handle.lamp_id));
}
