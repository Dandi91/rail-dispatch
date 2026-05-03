use bevy::ecs::hierarchy::ChildOf;
use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use rail_dispatch::assets::LoadingState;
use rail_dispatch::common::LampId;
use rail_dispatch::level::LampData;

use crate::editor::{EditorState, RespawnLamp};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldKind {
    X,
    Y,
    Width,
    Rotation,
}

impl FieldKind {
    pub fn read(self, l: &LampData) -> i32 {
        match self {
            Self::X => l.x,
            Self::Y => l.y,
            Self::Width => l.width,
            Self::Rotation => l.rotation,
        }
    }
    pub fn write(self, l: &mut LampData, v: i32) {
        match self {
            Self::X => l.x = v,
            Self::Y => l.y = v,
            Self::Width => l.width = v.max(4),
            Self::Rotation => l.rotation = v,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::X => "x",
            Self::Y => "y",
            Self::Width => "w",
            Self::Rotation => "r",
        }
    }
}

#[derive(Component)]
pub struct StepperButton {
    pub kind: FieldKind,
    pub delta: i32,
}

#[derive(Component)]
pub struct InputBox {
    pub kind: FieldKind,
    pub buffer: String,
    pub cursor: usize,
}

#[derive(Component)]
pub struct CharCell {
    pub index: usize,
}

#[derive(Resource, Default)]
pub struct FocusedInput(pub Option<Entity>);

const FIELD_FONT_SIZE: f32 = 12.0;
const CURSOR_HEIGHT: f32 = 14.0;

pub struct TextInputPlugin;

impl Plugin for TextInputPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FocusedInput>().add_systems(
            Update,
            (
                keyboard_input,
                apply_input_to_state,
                sync_inputs_from_state,
                rebuild_inputs,
            )
                .chain()
                .run_if(in_state(LoadingState::Loaded)),
        );
    }
}

pub fn number_field(parent: &mut ChildSpawnerCommands, kind: FieldKind, step: i32) {
    parent
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            column_gap: px(4.0),
            align_items: AlignItems::Center,
            ..default()
        })
        .with_children(|p| {
            stepper(p, kind, -step, format!("{} -{}", kind.label(), step));
            p.spawn((
                Button,
                InputBox {
                    kind,
                    buffer: String::new(),
                    cursor: 0,
                },
                Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    flex_grow: 1.0,
                    min_width: px(48.0),
                    height: px(20.0),
                    padding: UiRect::axes(px(4.0), px(2.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.1, 0.1, 0.15, 1.0)),
            ))
            .observe(on_input_box_click);
            stepper(p, kind, step, format!("{} +{}", kind.label(), step));
        });
}

fn stepper(parent: &mut ChildSpawnerCommands, kind: FieldKind, delta: i32, label: String) {
    parent
        .spawn((
            Button,
            StepperButton { kind, delta },
            Node {
                padding: UiRect::all(px(4.0)),
                min_width: px(40.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.2, 0.2, 0.3, 1.0)),
        ))
        .with_children(|p| {
            p.spawn((
                Text::new(label),
                TextFont::from_font_size(11.0),
                TextColor(Color::WHITE),
                Pickable::IGNORE,
            ));
        })
        .observe(on_stepper_click);
}

fn on_stepper_click(
    event: On<Pointer<Click>>,
    buttons: Query<&StepperButton>,
    mut state: ResMut<EditorState>,
    mut respawns: MessageWriter<RespawnLamp>,
    mut focus: ResMut<FocusedInput>,
) {
    let Ok(b) = buttons.get(event.entity) else {
        return;
    };
    let Some(id) = state.selected else { return };
    let Some(lamp) = state.get_mut(id) else { return };
    let new_val = b.kind.read(lamp) + b.delta;
    b.kind.write(lamp, new_val);
    respawns.write(RespawnLamp(id));
    focus.0 = None;
}

fn on_input_box_click(event: On<Pointer<Click>>, mut focus: ResMut<FocusedInput>, mut inputs: Query<&mut InputBox>) {
    if event.original_event_target() != event.entity {
        return;
    }
    let Ok(mut input) = inputs.get_mut(event.entity) else {
        return;
    };
    input.cursor = input.buffer.len();
    focus.0 = Some(event.entity);
}

fn on_cell_click(
    event: On<Pointer<Click>>,
    cells: Query<(&CharCell, &ChildOf)>,
    mut focus: ResMut<FocusedInput>,
    mut inputs: Query<&mut InputBox>,
) {
    let Ok((cell, child_of)) = cells.get(event.entity) else {
        return;
    };
    let Ok(mut input) = inputs.get_mut(child_of.0) else {
        return;
    };
    input.cursor = cell.index.min(input.buffer.len());
    focus.0 = Some(child_of.0);
}

fn keyboard_input(
    mut events: MessageReader<KeyboardInput>,
    mut focus: ResMut<FocusedInput>,
    mut inputs: Query<&mut InputBox>,
) {
    let Some(entity) = focus.0 else {
        events.clear();
        return;
    };
    let Ok(mut input) = inputs.get_mut(entity) else {
        focus.0 = None;
        events.clear();
        return;
    };
    for ev in events.read() {
        if ev.state != ButtonState::Pressed {
            continue;
        }
        match &ev.logical_key {
            Key::Character(s) => {
                for c in s.chars() {
                    if c.is_ascii_digit() {
                        let cur = input.cursor;
                        input.buffer.insert(cur, c);
                        input.cursor = cur + 1;
                    } else if c == '-' && input.cursor == 0 && !input.buffer.starts_with('-') {
                        input.buffer.insert(0, '-');
                        input.cursor = 1;
                    }
                }
            }
            Key::Backspace => {
                if input.cursor > 0 {
                    let cur = input.cursor - 1;
                    input.buffer.remove(cur);
                    input.cursor = cur;
                }
            }
            Key::Delete => {
                if input.cursor < input.buffer.len() {
                    let cur = input.cursor;
                    input.buffer.remove(cur);
                }
            }
            Key::ArrowLeft => {
                if input.cursor > 0 {
                    input.cursor -= 1;
                }
            }
            Key::ArrowRight => {
                let len = input.buffer.len();
                if input.cursor < len {
                    input.cursor += 1;
                }
            }
            Key::Home => input.cursor = 0,
            Key::End => input.cursor = input.buffer.len(),
            Key::Enter => {
                focus.0 = None;
                break;
            }
            Key::Escape => {
                focus.0 = None;
                break;
            }
            _ => {}
        }
    }
}

fn apply_input_to_state(
    inputs: Query<&InputBox, Changed<InputBox>>,
    mut state: ResMut<EditorState>,
    mut respawns: MessageWriter<RespawnLamp>,
) {
    let Some(id) = state.selected else { return };
    let mut updates: Vec<(FieldKind, i32)> = Vec::new();
    for input in inputs.iter() {
        let Ok(v) = input.buffer.parse::<i32>() else { continue };
        let Some(lamp) = state.get(id) else { return };
        if input.kind.read(lamp) == v {
            continue;
        }
        updates.push((input.kind, v));
    }
    if updates.is_empty() {
        return;
    }
    let Some(lamp) = state.get_mut(id) else { return };
    for (kind, v) in updates {
        kind.write(lamp, v);
    }
    respawns.write(RespawnLamp(id));
}

fn sync_inputs_from_state(
    state: Res<EditorState>,
    mut focus: ResMut<FocusedInput>,
    mut inputs: Query<(Entity, &mut InputBox)>,
    mut last_selected: Local<Option<LampId>>,
) {
    if !state.is_changed() {
        return;
    }
    let selection_changed = *last_selected != state.selected;
    *last_selected = state.selected;
    if selection_changed {
        focus.0 = None;
    }
    let lamp = state.selected.and_then(|id| state.get(id));
    for (entity, mut input) in inputs.iter_mut() {
        if !selection_changed && focus.0 == Some(entity) {
            continue;
        }
        let new = lamp.map(|l| input.kind.read(l).to_string()).unwrap_or_default();
        if input.buffer != new {
            input.buffer = new;
            input.cursor = input.buffer.len();
        }
    }
}

fn rebuild_inputs(mut commands: Commands, focus: Res<FocusedInput>, inputs: Query<(Entity, Ref<InputBox>)>) {
    let focus_changed = focus.is_changed();
    for (entity, input) in inputs.iter() {
        if !focus_changed && !input.is_changed() {
            continue;
        }
        commands.entity(entity).despawn_children();

        let focused = focus.0 == Some(entity);
        let cursor = input.cursor;
        let char_count = input.buffer.chars().count();
        commands.entity(entity).with_children(|p| {
            for (i, c) in input.buffer.chars().enumerate() {
                if focused && cursor == i {
                    spawn_cursor(p);
                }
                spawn_cell(p, i, c);
            }
            if focused && cursor == char_count {
                spawn_cursor(p);
            }
            spawn_tail(p, char_count);
        });
    }
}

fn spawn_cell(p: &mut ChildSpawnerCommands, index: usize, ch: char) {
    p.spawn((CharCell { index }, Node::default(), Pickable::default()))
        .observe(on_cell_click)
        .with_children(|c| {
            c.spawn((
                Text::new(ch.to_string()),
                TextFont::from_font_size(FIELD_FONT_SIZE),
                TextColor(Color::WHITE),
                Pickable::IGNORE,
            ));
        });
}

fn spawn_cursor(p: &mut ChildSpawnerCommands) {
    p.spawn((
        Node {
            width: px(1.0),
            height: px(CURSOR_HEIGHT),
            ..default()
        },
        BackgroundColor(Color::WHITE),
        Pickable::IGNORE,
    ));
}

fn spawn_tail(p: &mut ChildSpawnerCommands, end_index: usize) {
    p.spawn((
        CharCell { index: end_index },
        Node {
            flex_grow: 1.0,
            min_width: px(8.0),
            height: px(CURSOR_HEIGHT),
            ..default()
        },
        Pickable::default(),
    ))
    .observe(on_cell_click);
}
