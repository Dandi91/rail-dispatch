use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::prelude::*;
use bevy::time::common_conditions::on_real_timer;
use std::time::Duration;

/// How often the counter text is refreshed. Measured in real time, so it is unaffected by the
/// simulation time scale.
const REFRESH_INTERVAL: Duration = Duration::from_millis(100);
const TEXT_COLOR: Color = Color::srgb(0.0, 1.0, 0.0);

#[derive(Component, Default, Clone)]
struct FpsText;

/// Lightweight FPS readout in the top-left corner, fed by [`FrameTimeDiagnosticsPlugin`].
pub struct FpsCounterPlugin;

impl Plugin for FpsCounterPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<FrameTimeDiagnosticsPlugin>() {
            app.add_plugins(FrameTimeDiagnosticsPlugin::default());
        }
        app.add_systems(Startup, fps_counter.spawn())
            .add_systems(Update, update_fps_text.run_if(on_real_timer(REFRESH_INTERVAL)));
    }
}

fn fps_counter() -> impl Scene {
    bsn! {
        FpsText
        Node {
            position_type: {PositionType::Absolute},
            left: {px(5)},
        }
        Text("FPS: -")
        TextFont { font_size: {20.0} }
        TextColor(TEXT_COLOR)
        GlobalZIndex(i32::MAX)
        Pickable::IGNORE
    }
}

fn update_fps_text(diagnostics: Res<DiagnosticsStore>, mut text: Single<&mut Text, With<FpsText>>) {
    if let Some(fps) = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FPS)
        .and_then(|fps| fps.smoothed())
    {
        text.0 = format!("FPS: {fps:.0}");
    }
}
