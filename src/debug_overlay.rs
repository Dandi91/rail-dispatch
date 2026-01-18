use crate::display::Lamp;
use crate::simulation::block::BlockMap;
use crate::simulation::train::Train;
use bevy::prelude::*;
use std::ops::DerefMut;

#[derive(Event)]
pub struct UpdateDebugObservers;

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
                border_radius: BorderRadius::all(px(3.0)),
                ..default()
            },
            BackgroundColor(Color::srgb(0.21, 0.21, 0.21)),
            BorderColor::all(Color::WHITE),
            ZIndex(99),
            Pickable::IGNORE,
            Visibility::Hidden,
        ))
        .with_children(|p| {
            p.spawn((Text::default(), TextFont::from_font_size(10.0))); // lamp id
            p.spawn((Text::default(), TextFont::from_font_size(10.0))); // lamp info
            p.spawn((Text::default(), TextFont::from_font_size(10.0))); // train info
        });

    commands.add_observer(on_setup);
}

fn on_setup(_: On<UpdateDebugObservers>, lamps: Query<Entity, With<Lamp>>, mut commands: Commands) {
    let mut on_over = Observer::new(on_over_lamp);
    let mut on_out = Observer::new(on_out_lamp);

    on_over.watch_entities(lamps);
    on_out.watch_entities(lamps);

    commands.spawn(on_over);
    commands.spawn(on_out);
}

fn on_over_lamp(
    event: On<Pointer<Over>>,
    block_map: If<Res<BlockMap>>,
    trains: Query<&Train>,
    lamps: Query<&Lamp>,
    mut info: Single<(&Children, &mut Visibility, &mut Node), With<DetailsInfo>>,
    mut writer: TextUiWriter,
) {
    let target = event.entity;
    if let Ok(lamp) = lamps.get(target) {
        let (children, vis, node) = info.deref_mut();
        *writer.text(children[0], 0) = format!("Lamp ID: {}", lamp.0);
        let (lamp_str, train_ids) = block_map.get_lamp_info(lamp.0);
        *writer.text(children[1], 0) = lamp_str;
        if let Some(train_ids) = train_ids {
            let first = *train_ids.first().expect("at least one train in block");
            let train = trains.iter().find(|t| t.id == first).expect("invalid train ID");
            *writer.text(children[2], 0) = format!(
                "Train {}, speed {:.0} km/h, target speed {:.0} km/h",
                train.number,
                train.get_speed_kmh(),
                train.get_target_speed_kmh()
            );
        } else {
            writer.text(children[2], 0).clear();
        }
        **vis = Visibility::Visible;
        node.left = px(event.pointer_location.position.x + 10.0);
        node.top = px(event.pointer_location.position.y + 10.0);
    }
}

fn on_out_lamp(_: On<Pointer<Out>>, mut vis_info: Single<&mut Visibility, With<DetailsInfo>>) {
    **vis_info = Visibility::Hidden;
}
