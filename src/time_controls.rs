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

    pub fn time_scale_formatted(&self) -> String {
        if self.time_scale >= 1.0 {
            format!("{}x", self.time_scale as u32)
        } else {
            format!("{:.1}x", self.time_scale)
        }
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

pub struct TimeControlsPlugin;

impl Plugin for TimeControlsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TimeControls>()
            .add_systems(Startup, setup)
            .add_systems(Update, time_controls);
    }
}

fn setup(mut commands: Commands, time_controls: Res<TimeControls>) {
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
                Text::new(time_controls.time_scale_formatted()),
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
    mut time_controls: ResMut<TimeControls>,
    mut time: ResMut<Time<Virtual>>,
    mut overlay_config: ResMut<FpsOverlayConfig>,
    query: Single<Entity, With<TimeScaleText>>,
    mut writer: TextUiWriter,
) {
    if keyboard_input.just_pressed(KeyCode::ArrowUp) {
        if let Some(new_time_scale) = time_controls.inc() {
            time.set_relative_speed_f64(new_time_scale);
            overlay_config.refresh_interval = Duration::from_millis((100.0 * new_time_scale) as u64);
            *writer.text(query.entity(), 0) = time_controls.time_scale_formatted();
            println!("Setting timescale to {}", new_time_scale);
        }
    }
    if keyboard_input.just_pressed(KeyCode::ArrowDown) {
        if let Some(new_time_scale) = time_controls.dec() {
            time.set_relative_speed_f64(new_time_scale);
            overlay_config.refresh_interval = Duration::from_millis((100.0 * new_time_scale) as u64);
            *writer.text(query.entity(), 0) = time_controls.time_scale_formatted();
            println!("Setting timescale to {}", new_time_scale);
        }
    }
    if keyboard_input.just_pressed(KeyCode::KeyP) {
        time_controls.paused = !time_controls.paused;
        if time_controls.paused {
            time.pause();
            println!("Paused");
        } else {
            time.unpause();
            println!("Resumed");
        }
    }
}
