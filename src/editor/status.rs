//! AutoCAD-style status/command bar along the bottom of the window. It echoes the current action
//! (placing a block, inserting a vertex, dragging, …) derived from the editor state each frame, so
//! the user always sees what a click will do and how to cancel.

use bevy::prelude::*;
use rail_dispatch::assets::FontHandles;

use crate::state::{EditMode, EditorSelection};

const BAR_BG: Color = Color::srgb(0.10, 0.11, 0.14);
const BAR_TEXT: Color = Color::srgb(0.85, 0.87, 0.92);

/// Idle hint, doubling as a key legend.
const IDLE_HINT: &str =
    "B: add block    Ctrl+click: add vertex    drag track: move block    Del: remove vertex    Ctrl+S: save";

#[derive(Component)]
pub(crate) struct StatusText;

pub fn setup_status_bar(fonts: Res<FontHandles>, mut commands: Commands) {
    let font = TextFont {
        font: fonts.mono.clone().into(),
        font_size: FontSize::Px(13.0),
        ..default()
    };
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                bottom: px(0),
                left: px(0),
                right: px(0),
                padding: UiRect::axes(px(8), px(4)),
                ..default()
            },
            BackgroundColor(BAR_BG),
            GlobalZIndex(50),
            Pickable::IGNORE,
        ))
        .with_children(|p| {
            p.spawn((
                StatusText,
                Text::new(IDLE_HINT),
                font,
                TextColor(BAR_TEXT),
                Pickable::IGNORE,
            ));
        });
}

/// Pick the prompt for the current state, most specific first.
fn status_message(keys: &ButtonInput<KeyCode>, selection: &EditorSelection) -> &'static str {
    let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    if selection.mode == EditMode::PlacingBlock {
        if selection.pending_start.is_none() {
            "Adding block: click the start vertex    (Esc to cancel)"
        } else {
            "Adding block: click the end vertex    (Esc to cancel)"
        }
    } else if selection.dragging {
        "Moving vertex — release to drop"
    } else if selection.block_drag.is_some() {
        "Moving block — release to drop"
    } else if ctrl {
        "Click on a block to add a vertex"
    } else if selection.hovered.is_some() {
        "Drag to move vertex    •    Del to remove it"
    } else {
        IDLE_HINT
    }
}

pub fn update_status_bar(
    keys: Res<ButtonInput<KeyCode>>,
    selection: Res<EditorSelection>,
    text: Option<Single<&mut Text, With<StatusText>>>,
) {
    let Some(mut text) = text else { return };
    let msg = status_message(&keys, &selection);
    if text.0 != msg {
        text.0 = msg.to_string();
    }
}
