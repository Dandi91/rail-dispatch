use bevy::dev_tools::fps_overlay::FpsOverlayConfig;
use bevy::prelude::*;
use std::time::Duration;

const MULTIPLIERS: [f64; 7] = [0.1, 0.5, 1.0, 2.0, 5.0, 10.0, 20.0];
const DEFAULT_MULTIPLIER_INDEX: usize = 2;

#[derive(Resource)]
pub struct TimeControls {
    pub time_scale: f64,
    pub multiplier_index: usize,
    pub paused: bool,
}

impl TimeControls {
    fn inc(&mut self) -> Option<f64> {
        if self.multiplier_index < MULTIPLIERS.len() - 1 {
            self.multiplier_index += 1;
            self.time_scale = MULTIPLIERS[self.multiplier_index];
            return Some(self.time_scale);
        }
        None
    }

    fn dec(&mut self) -> Option<f64> {
        if self.multiplier_index > 0 {
            self.multiplier_index -= 1;
            self.time_scale = MULTIPLIERS[self.multiplier_index];
            return Some(self.time_scale);
        }
        None
    }
}

pub fn time_scale_formatted(time_scale: f64) -> String {
    if time_scale >= 1.0 {
        format!("{}x", time_scale as u32)
    } else {
        format!("{:.1}x", time_scale)
    }
}

impl Default for TimeControls {
    fn default() -> Self {
        TimeControls {
            time_scale: MULTIPLIERS[DEFAULT_MULTIPLIER_INDEX],
            multiplier_index: DEFAULT_MULTIPLIER_INDEX,
            paused: false,
        }
    }
}

#[derive(Component)]
struct TimeScaleText;

#[derive(Event)]
pub struct TimeScaleChanged {
    pub time_scale: f64,
}

#[derive(Event)]
pub struct PauseToggled {
    pub paused: bool,
}

pub struct TimeControlsPlugin;

impl Plugin for TimeControlsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TimeControls>()
            .add_systems(Startup, setup)
            .add_systems(Update, time_controls)
            .add_observer(on_time_scale_change)
            .add_observer(on_pause_toggle);
    }
}

fn setup(mut commands: Commands) {
    commands
        .spawn((
            Node {
                right: Val::Px(100.0),
                position_type: PositionType::Absolute,
                flex_direction: FlexDirection::Column,
                ..Default::default()
            },
            GlobalZIndex(i32::MAX),
            Pickable::IGNORE,
        ))
        .with_children(|p| {
            p.spawn((
                Text::new(time_scale_formatted(MULTIPLIERS[DEFAULT_MULTIPLIER_INDEX])),
                TextFont::from_font_size(20.0),
                TextColor(Color::WHITE),
                TimeScaleText,
                Pickable::IGNORE,
            ))
            .with_child((TextSpan::default(), TextFont::from_font_size(20.0)));
        });
}

fn time_controls(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    mut time_controls: ResMut<TimeControls>,
) {
    if keyboard_input.just_pressed(KeyCode::ArrowUp) {
        if let Some(time_scale) = time_controls.inc() {
            commands.trigger(TimeScaleChanged { time_scale });
        }
    }
    if keyboard_input.just_pressed(KeyCode::ArrowDown) {
        if let Some(time_scale) = time_controls.dec() {
            commands.trigger(TimeScaleChanged { time_scale });
        }
    }
    if keyboard_input.just_pressed(KeyCode::KeyP) {
        time_controls.paused = !time_controls.paused;
        commands.trigger(PauseToggled {
            paused: time_controls.paused,
        });
    }
}

fn on_time_scale_change(
    change: On<TimeScaleChanged>,
    mut time: ResMut<Time<Virtual>>,
    mut overlay_config: ResMut<FpsOverlayConfig>,
    query: Single<Entity, With<TimeScaleText>>,
    mut writer: TextUiWriter,
) {
    time.set_relative_speed_f64(change.time_scale);
    overlay_config.refresh_interval = Duration::from_millis((100.0 * change.time_scale) as u64);
    *writer.text(query.entity(), 0) = time_scale_formatted(change.time_scale);
    println!("Setting timescale to {}", change.time_scale);
}

fn on_pause_toggle(toggle: On<PauseToggled>, mut time: ResMut<Time<Virtual>>) {
    if toggle.paused {
        time.pause();
        println!("Paused");
    } else {
        time.unpause();
        println!("Resumed");
    }
}
