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
    pub residual: Vec2,
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
    let cx = lamp.x as f32 + lamp.width as f32 * 0.5;
    let cy = lamp.y as f32 + lamp_h * 0.5;
    let theta = (lamp.rotation as f32).to_radians();
    let (s, c) = theta.sin_cos();

    let body_node = Node {
        position_type: PositionType::Absolute,
        left: px(lamp.x),
        top: px(lamp.y),
        width: px(lamp.width),
        height: px(lamp_h),
        ..default()
    };
    let body_xform = UiTransform {
        rotation: Rot2::degrees(lamp.rotation as f32),
        ..default()
    };

    let width_r = lamp.width as f32 * 0.5 + WIDTH_HANDLE_OFFSET;
    let width_node = Node {
        position_type: PositionType::Absolute,
        left: px(cx + width_r * c - HANDLE_SIZE * 0.5),
        top: px(cy + width_r * s - HANDLE_SIZE * 0.5),
        width: px(HANDLE_SIZE),
        height: px(HANDLE_SIZE),
        ..default()
    };
    let width_xform = UiTransform {
        rotation: Rot2::degrees(lamp.rotation as f32 + 45.0),
        ..default()
    };

    // Rotation handle: lamp-local offset (0, -R) → canvas (cx + R*sin θ, cy - R*cos θ).
    let rotation_node = Node {
        position_type: PositionType::Absolute,
        left: px(cx + ROTATION_HANDLE_OFFSET * s - HANDLE_SIZE * 0.5),
        top: px(cy - ROTATION_HANDLE_OFFSET * c - HANDLE_SIZE * 0.5),
        width: px(HANDLE_SIZE),
        height: px(HANDLE_SIZE),
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
                    residual: Vec2::ZERO,
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
                    residual: Vec2::ZERO,
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
                    residual: Vec2::ZERO,
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
    mut handles_q: Query<&mut EditorHandle>,
    mut state: ResMut<EditorState>,
    mut respawns: MessageWriter<RespawnLamp>,
) {
    if event.button != PointerButton::Primary {
        return;
    }
    let Ok(mut handle) = handles_q.get_mut(event.entity) else {
        return;
    };
    let inv = 1.0 / state.zoom.max(0.0001);
    let Some(lamp) = state.get_mut(handle.lamp_id) else {
        return;
    };
    let dx = event.delta.x * inv;
    let dy = event.delta.y * inv;
    let mut changed = false;
    match handle.kind {
        HandleKind::Body => {
            let acc = handle.residual + Vec2::new(dx, dy);
            let step = Vec2::new(acc.x.round(), acc.y.round());
            handle.residual = acc - step;
            if step.x != 0.0 {
                lamp.x += step.x as i32;
                changed = true;
            }
            if step.y != 0.0 {
                lamp.y += step.y as i32;
                changed = true;
            }
        }
        HandleKind::Width => {
            let theta = (lamp.rotation as f32).to_radians();
            let (s, c) = theta.sin_cos();
            let dw = dx * c + dy * s;
            let acc = handle.residual.x + dw;
            let step = acc.round();
            handle.residual.x = acc - step;
            if step != 0.0 {
                lamp.width = (lamp.width + step as i32).max(4);
                changed = true;
            }
        }
        HandleKind::Rotation => {
            let theta = (lamp.rotation as f32).to_radians();
            let (s, c) = theta.sin_cos();
            let arc = dx * c + dy * s;
            let dr = arc.to_degrees() / ROTATION_HANDLE_OFFSET.max(1.0);
            let acc = handle.residual.x + dr;
            let step = acc.round();
            handle.residual.x = acc - step;
            if step != 0.0 {
                lamp.rotation += step as i32;
                changed = true;
            }
        }
    }
    if changed {
        respawns.write(RespawnLamp(handle.lamp_id));
    }
}
