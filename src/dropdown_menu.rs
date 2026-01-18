use bevy::prelude::*;
use std::ops::DerefMut;

const MENU_BACKGROUND_DEFAULT: BackgroundColor = BackgroundColor(Color::srgb(0.15, 0.15, 0.15));
const MENU_BACKGROUND_HIGHLIGHT: BackgroundColor = BackgroundColor(Color::srgb(0.31, 0.31, 0.31));

#[derive(Component)]
struct ContextMenuItem;

#[derive(Component, Default)]
pub struct ContextMenu {
    target: Option<Entity>,
}

pub trait DropDownMenu: Component + Sized {
    type Event<'a>: EntityEvent<Trigger<'a>: Default>;

    fn create_event(&self, entity: Entity) -> Self::Event<'_>;

    fn get_label(&self) -> impl Into<String>;

    fn list_available_items() -> impl IntoIterator<Item = Self>;

    fn on_entity_right_click(
        event: On<Pointer<Click>>,
        mut menu: Single<(Entity, &mut Visibility, &mut Node, &mut ContextMenu)>,
        mut commands: Commands,
    ) {
        if event.button != PointerButton::Secondary {
            return;
        }

        let (entity, vis, node, context_menu) = menu.deref_mut();
        commands.entity(*entity).despawn_children().with_children(|p| {
            for item in Self::list_available_items() {
                let label = Text::new(item.get_label());
                p.spawn((
                    ContextMenuItem,
                    Node {
                        padding: UiRect::all(px(4.0)),
                        ..default()
                    },
                    item,
                    Pickable::default(),
                ))
                .with_children(|item| {
                    item.spawn((label, TextFont::from_font_size(12.0), Pickable::IGNORE));
                });
            }
        });

        **vis = Visibility::Visible;
        node.left = px(event.pointer_location.position.x);
        node.top = px(event.pointer_location.position.y);
        context_menu.target = Some(event.entity);
    }

    fn on_left_click(
        mut event: On<Pointer<Click>>,
        items: Populated<&Self>,
        mut menu: Single<(Entity, &mut Visibility, &mut ContextMenu)>,
        mut commands: Commands,
    ) {
        if event.button != PointerButton::Primary {
            return;
        }

        let (entity, vis, context_menu) = menu.deref_mut();
        if let Ok(item) = items.get(event.entity) {
            if let Some(target) = context_menu.target {
                commands.trigger(item.create_event(target));
                event.propagate(false);
                context_menu.target = None;
            }
        }

        commands.entity(*entity).despawn_children();
        **vis = Visibility::Hidden;
    }

    fn register<E: IntoIterator<Item = Entity>>(commands: &mut Commands, entities: E) {
        commands.spawn(Observer::new(Self::on_entity_right_click).with_entities(entities));
        commands.add_observer(Self::on_left_click);
    }
}

pub struct DropdownPlugin;

impl Plugin for DropdownPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup);
    }
}

fn setup(mut commands: Commands) {
    commands
        .spawn((
            ContextMenu::default(),
            Node {
                position_type: PositionType::Absolute,
                border: UiRect::all(px(1)),
                flex_direction: FlexDirection::Column,
                border_radius: BorderRadius::all(px(3.0)),
                ..default()
            },
            MENU_BACKGROUND_DEFAULT,
            BorderColor::all(Color::WHITE),
            GlobalZIndex(100),
            Visibility::Hidden,
        ))
        .observe(
            |event: On<Pointer<Over>>, menu_items: Query<&ContextMenuItem>, mut commands: Commands| {
                let target = event.original_event_target();
                if menu_items.get(target).is_ok() {
                    commands.entity(target).insert(MENU_BACKGROUND_HIGHLIGHT);
                }
            },
        )
        .observe(
            |event: On<Pointer<Out>>, menu_items: Query<&ContextMenuItem>, mut commands: Commands| {
                let target = event.original_event_target();
                if menu_items.get(target).is_ok() {
                    commands.entity(target).remove::<BackgroundColor>();
                }
            },
        );
}
