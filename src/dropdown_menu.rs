use bevy::ecs::system::{StaticSystemParam, SystemParam, SystemParamItem};
use bevy::input::keyboard::Key;
use bevy::prelude::*;
use std::ops::DerefMut;

const MENU_BACKGROUND_DEFAULT: Color = Color::srgb(0.15, 0.15, 0.15);
const MENU_BACKGROUND_HIGHLIGHT: Color = Color::srgb(0.31, 0.31, 0.31);

#[derive(Component, Default, Clone)]
struct ContextMenuItem;

#[derive(Component, Default, Clone)]
pub struct ContextMenu {
    target: Option<Entity>,
}

pub trait DropDownMenu: Component + Sized {
    type Event<'a>: EntityEvent<Trigger<'a>: Default>;
    type Context: SystemParam + 'static;

    fn create_event(&self, entity: Entity) -> Self::Event<'_>;

    fn get_label(&self) -> impl Into<String>;

    fn list_available_items(target: Entity, ctx: &mut SystemParamItem<Self::Context>)
    -> impl IntoIterator<Item = Self>;

    fn key_filter(_: Res<ButtonInput<Key>>) -> bool {
        true
    }

    fn on_entity_click(
        event: On<PointerClick>,
        keyboard_input: Res<ButtonInput<Key>>,
        mut ctx: StaticSystemParam<Self::Context>,
        mut menu: Single<(Entity, &mut Visibility, &mut Node, &mut ContextMenu)>,
        mut commands: Commands,
    ) {
        if !Self::key_filter(keyboard_input) {
            return;
        }

        let items: Vec<Self> = Self::list_available_items(event.entity, &mut *ctx)
            .into_iter()
            .collect();
        let items_len = items.len();

        let (entity, vis, node, context_menu) = menu.deref_mut();
        let menu_entity = *entity;
        commands.entity(menu_entity).despawn_children();
        for item in items {
            commands
                .spawn_scene(bsn! {
                    ContextMenuItem
                    Node { padding: {UiRect::all(px(4.0))} }
                    Pickable
                    Children [
                        Text({item.get_label()})
                        TextFont { font_size: {12.0} }
                        Pickable::IGNORE
                    ]
                })
                .insert((item, ChildOf(menu_entity)));
        }

        if items_len > 0 {
            **vis = Visibility::Visible;
            node.left = px(event.pointer.position.x);
            node.top = px(event.pointer.position.y);
            context_menu.target = Some(event.entity);
        }
    }

    fn on_menu_click(
        mut event: On<PointerClick>,
        items: Populated<&Self>,
        mut menu: Single<(Entity, &mut Visibility, &mut ContextMenu)>,
        mut commands: Commands,
    ) {
        if event.button != PointerButton::Primary {
            return;
        }

        let (entity, vis, context_menu) = menu.deref_mut();
        if let Ok(item) = items.get(event.entity)
            && let Some(target) = context_menu.target
        {
            commands.trigger(item.create_event(target));
            event.propagate(false);
            context_menu.target = None;
        }

        commands.entity(*entity).despawn_children();
        **vis = Visibility::Hidden;
    }

    fn register<E: IntoIterator<Item = Entity>>(commands: &mut Commands, entities: E) {
        commands.spawn(Observer::new(Self::on_entity_click).with_entities(entities));
        commands.add_observer(Self::on_menu_click);
    }
}

pub struct DropdownPlugin;

impl Plugin for DropdownPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup);
    }
}

fn setup(mut commands: Commands) {
    commands.spawn_scene(bsn! {
        ContextMenu
        Node {
            position_type: {PositionType::Absolute},
            border: {UiRect::all(px(1))},
            flex_direction: {FlexDirection::Column},
            border_radius: {BorderRadius::all(px(3.0))},
        }
        BackgroundColor(MENU_BACKGROUND_DEFAULT)
        BorderColor::all(Color::WHITE)
        GlobalZIndex(100)
        Visibility::Hidden
        on(|event: On<PointerOver>, menu_items: Query<&ContextMenuItem>, mut commands: Commands| {
            let target = event.original_event_target();
            if menu_items.get(target).is_ok() {
                commands.entity(target).insert(BackgroundColor(MENU_BACKGROUND_HIGHLIGHT));
            }
        })
        on(|event: On<PointerOut>, menu_items: Query<&ContextMenuItem>, mut commands: Commands| {
            let target = event.original_event_target();
            if menu_items.get(target).is_ok() {
                commands.entity(target).remove::<BackgroundColor>();
            }
        })
    });
}
