use bevy::prelude::*;
use rail_dispatch::assets::LoadingState;
use rail_dispatch::display::LampKind;
use rail_dispatch::level::LampData;

use crate::editor::{EditorState, RespawnLamp, lamp_kind_for};
use crate::save::save_level;
use crate::text_input::{FieldKind, TextInputPlugin, number_field};

#[derive(Resource, Default)]
pub struct SidebarState {
    pub create_kind: CreateKind,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum CreateKind {
    #[default]
    Block,
    Signal,
}

#[derive(Component)]
pub struct InfoText;

#[derive(Component, Clone, Copy)]
pub enum SidebarButton {
    NewLamp,
    Delete,
    Save,
    ResetZoom,
    ToggleKind,
}

#[derive(Component)]
pub struct ButtonLabel(pub SidebarButton);

pub struct SidebarPlugin;

impl Plugin for SidebarPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SidebarState>()
            .add_plugins(TextInputPlugin)
            .add_systems(Startup, setup_sidebar)
            .add_systems(
                Update,
                (update_info_text, update_button_labels).run_if(in_state(LoadingState::Loaded)),
            );
    }
}

fn setup_sidebar(mut commands: Commands) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Vw(20.0),
                height: Val::Vh(100.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(8.0)),
                row_gap: Val::Px(6.0),
                ..default()
            },
            ZIndex(100),
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.7)),
        ))
        .with_children(|p| {
            p.spawn((
                Text::new("Map Editor"),
                TextFont::from_font_size(16.0),
                TextColor(Color::WHITE),
            ));
            p.spawn((
                InfoText,
                Text::new(""),
                TextFont::from_font_size(12.0),
                TextColor(Color::WHITE),
            ));
            sidebar_button(p, "New lamp", SidebarButton::NewLamp);
            sidebar_button(p, "Kind: Block", SidebarButton::ToggleKind);
            sidebar_button(p, "Delete", SidebarButton::Delete);

            number_field(p, FieldKind::X, 1);
            number_field(p, FieldKind::Y, 1);
            number_field(p, FieldKind::Width, 1);
            number_field(p, FieldKind::Rotation, 5);

            sidebar_button(p, "Reset zoom", SidebarButton::ResetZoom);
            sidebar_button(p, "Save level.toml", SidebarButton::Save);
        });
}

fn sidebar_button(parent: &mut ChildSpawnerCommands, label: &str, action: SidebarButton) {
    parent
        .spawn((
            Button,
            action,
            Node {
                padding: UiRect::all(Val::Px(4.0)),
                min_width: Val::Px(40.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.2, 0.2, 0.3, 1.0)),
        ))
        .with_children(|p| {
            p.spawn((
                ButtonLabel(action),
                Text::new(label.to_string()),
                TextFont::from_font_size(11.0),
                TextColor(Color::WHITE),
            ));
        })
        .observe(on_button_click);
}

fn on_button_click(
    event: On<Pointer<Click>>,
    buttons: Query<&SidebarButton>,
    mut state: ResMut<EditorState>,
    mut sidebar_state: ResMut<SidebarState>,
    mut respawns: MessageWriter<RespawnLamp>,
) {
    let Ok(action) = buttons.get(event.entity) else { return };
    match *action {
        SidebarButton::NewLamp => {
            let id = match sidebar_state.create_kind {
                CreateKind::Block => state.next_block_id(),
                CreateKind::Signal => state.next_signal_id(),
            };
            state.lamps.push(LampData {
                id,
                x: 100,
                y: 100,
                width: 17,
                rotation: 0,
            });
            state.selected = Some(id);
            respawns.write(RespawnLamp(id));
        }
        SidebarButton::Delete => {
            if let Some(id) = state.selected {
                state.lamps.retain(|l| l.id != id);
                state.selected = None;
                respawns.write(RespawnLamp(id));
            }
        }
        SidebarButton::ResetZoom => {
            state.zoom = 1.0;
        }
        SidebarButton::Save => match save_level(&state.lamps) {
            Ok(()) => info!("saved level.toml"),
            Err(e) => warn!("save failed: {e}"),
        },
        SidebarButton::ToggleKind => {
            sidebar_state.create_kind = match sidebar_state.create_kind {
                CreateKind::Block => CreateKind::Signal,
                CreateKind::Signal => CreateKind::Block,
            };
        }
    }
}

fn update_info_text(
    state: Res<EditorState>,
    sidebar_state: Res<SidebarState>,
    mut q: Query<&mut Text, With<InfoText>>,
) {
    let Ok(mut text) = q.single_mut() else { return };
    let new = match state.selected.and_then(|id| state.get(id)) {
        Some(lamp) => {
            let kind = match lamp_kind_for(lamp.id) {
                LampKind::Block => "block",
                LampKind::Signal => "signal",
            };
            format!(
                "id {} ({})\n\nlamps total: {}\nnew kind: {:?}",
                lamp.id,
                kind,
                state.lamps.len(),
                sidebar_state.create_kind
            )
        }
        None => format!(
            "(no lamp selected)\n\nlamps total: {}\nnew kind: {:?}",
            state.lamps.len(),
            sidebar_state.create_kind
        ),
    };
    if text.0 != new {
        text.0 = new;
    }
}

fn update_button_labels(
    sidebar_state: Res<SidebarState>,
    editor_state: Res<EditorState>,
    mut q: Query<(&ButtonLabel, &mut Text)>,
) {
    for (button, mut text) in q.iter_mut() {
        match button.0 {
            SidebarButton::ToggleKind => {
                let label = match sidebar_state.create_kind {
                    CreateKind::Block => "Kind: Block",
                    CreateKind::Signal => "Kind: Signal",
                };
                if text.0 != label {
                    text.0 = label.into();
                }
            }
            SidebarButton::ResetZoom => {
                let label = format!("Reset zoom ({:.1})", editor_state.zoom);
                if text.0 != label {
                    text.0 = label.into();
                }
            }
            _ => {}
        }
    }
}
