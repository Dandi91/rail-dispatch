use crate::display::Lamp;
use crate::simulation::block::BlockMap;
use bevy::prelude::*;
use std::ops::DerefMut;

#[derive(Component)]
struct DebugObserverOver;

#[derive(Component)]
struct DebugObserverOut;

#[derive(Component)]
struct DetailsInfo;

pub struct DebugOverlayPlugin;

impl Plugin for DebugOverlayPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup);
    }
}

fn setup(mut commands: Commands) {
    commands.spawn((
        Text::new(
            "G to spawn a new train\n\
                  H to despawn the oldest train\n\
                  Up or Down to change the speed\n\
                  Hover over lamps to see info",
        ),
        TextFont::from_font_size(16.0),
        Node {
            position_type: PositionType::Absolute,
            bottom: px(12),
            left: px(12),
            ..default()
        },
    ));

    commands
        .spawn((
            DetailsInfo,
            Node {
                position_type: PositionType::Absolute,
                border: UiRect::all(px(1)),
                padding: UiRect::all(px(5)),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            BackgroundColor(Color::srgb(0.21, 0.21, 0.21)),
            BorderColor::all(Color::WHITE),
            BorderRadius::all(px(3.0)),
            Pickable::IGNORE,
            Visibility::Hidden,
        ))
        .with_children(|p| {
            p.spawn((Text::default(), TextFont::from_font_size(10.0)));
            p.spawn((Text::default(), TextFont::from_font_size(10.0)));
        });

    commands.spawn((DebugObserverOver, Observer::new(on_over_lamp)));
    commands.spawn((DebugObserverOut, Observer::new(on_out_lamp)));
    commands.add_observer(on_add_lamp);
}

fn on_add_lamp(
    event: On<Add, Lamp>,
    mut over: Single<&mut Observer, (With<DebugObserverOver>, Without<DebugObserverOut>)>,
    mut out: Single<&mut Observer, (With<DebugObserverOut>, Without<DebugObserverOver>)>,
) {
    over.watch_entity(event.entity);
    out.watch_entity(event.entity);
}

fn on_over_lamp(
    event: On<Pointer<Over>>,
    block_map: If<Res<BlockMap>>,
    mut query: Query<&Lamp>,
    mut info: Single<(&Children, &mut Visibility, &mut Node), With<DetailsInfo>>,
    mut writer: TextUiWriter,
) {
    let target = event.entity;
    if let Ok(lamp) = query.get_mut(target) {
        let (children, vis, node) = info.deref_mut();
        *writer.text(children[0], 0) = format!("Lamp ID: {}", lamp.id);
        *writer.text(children[1], 0) = block_map.get_lamp_info(lamp.id).unwrap();
        **vis = Visibility::Visible;
        node.left = px(event.pointer_location.position.x + 10.0);
        node.top = px(event.pointer_location.position.y + 10.0);
    }
}

fn on_out_lamp(_event: On<Pointer<Out>>, mut vis_info: Single<&mut Visibility, With<DetailsInfo>>) {
    **vis_info = Visibility::Hidden;
}
