use crate::assets::SoundHandles;
use bevy::prelude::*;

pub enum AudioKind {
    Beep,
    Error,
    Message,
    Notification,
}

#[derive(Event)]
pub struct AudioEvent {
    pub kind: AudioKind,
}

impl AudioEvent {
    pub fn beep() -> Self {
        AudioEvent { kind: AudioKind::Beep }
    }

    pub fn error() -> Self {
        AudioEvent { kind: AudioKind::Error }
    }

    pub fn message() -> Self {
        AudioEvent {
            kind: AudioKind::Message,
        }
    }

    pub fn notification() -> Self {
        AudioEvent {
            kind: AudioKind::Notification,
        }
    }
}

#[derive(Component)]
struct AudioRoot;

pub struct AudioPlugin;

impl Plugin for AudioPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, startup);
    }
}

fn startup(mut commands: Commands) {
    commands.spawn(AudioRoot);
    commands.add_observer(on_audio_event);
}

fn on_audio_event(
    event: On<AudioEvent>,
    root: Single<Entity, With<AudioRoot>>,
    handles: Res<SoundHandles>,
    mut commands: Commands,
) {
    let mut audio_root = commands.get_entity(*root).expect("audio root should've been spawned");
    let source = match event.kind {
        AudioKind::Beep => handles.beep.clone(),
        AudioKind::Error => handles.error.clone(),
        AudioKind::Message => handles.message.clone(),
        AudioKind::Notification => handles.notification.clone(),
    };

    audio_root.with_child((AudioPlayer::new(source), PlaybackSettings::DESPAWN));
}
