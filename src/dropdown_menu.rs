use bevy::prelude::*;
use std::ops::DerefMut;

const MENU_BACKGROUND_DEFAULT: BackgroundColor = BackgroundColor(Color::srgb(0.15, 0.15, 0.15));
const MENU_BACKGROUND_HIGHLIGHT: BackgroundColor = BackgroundColor(Color::srgb(0.31, 0.31, 0.31));

#[derive(Component, Default)]
pub struct ContextMenu {
    target: Option<Entity>,
}

pub trait MenuAction {
    fn create_entity_event<'a>(&self, entity: Entity) -> impl EntityEvent<Trigger<'a>: Default>;

    fn get_label(&self) -> impl Into<String>;
}

pub trait MenuItem: Component + Sized {
    type Action: MenuAction;

    fn get_action(&self) -> &Self::Action;

    fn list_available_items() -> impl IntoIterator<Item = Self>;

    fn on_entity_right_click(
        event: On<Pointer<Click>>,
        mut menu: Single<(Entity, &mut Visibility, &mut Node, &mut ContextMenu)>,
        mut commands: Commands,
    ) {
        if event.button == PointerButton::Secondary {
            let (entity, vis, node, context_menu) = menu.deref_mut();
            commands.entity(*entity).clear_children().with_children(|p| {
                for item in Self::list_available_items() {
                    let label = Text::new(item.get_action().get_label());
                    p.spawn((
                        Node {
                            padding: UiRect::all(px(4.0)),
                            ..default()
                        },
                        item,
                        Pickable::default(),
                    ))
                    .observe(on_menu_hover)
                    .observe(on_menu_out)
                    .with_children(|item| {
                        item.spawn((label, TextFont::from_font_size(12.0)));
                    });
                }
            });

            **vis = Visibility::Visible;
            node.left = px(event.pointer_location.position.x);
            node.top = px(event.pointer_location.position.y);
            context_menu.target = Some(event.entity);
        }
    }

    fn on_left_click(
        event: On<Pointer<Click>>,
        items: Populated<&Self>,
        mut menu: Single<(&mut Visibility, &mut ContextMenu)>,
        mut commands: Commands,
    ) {
        if event.button != PointerButton::Primary {
            return;
        }

        let (vis, context_menu) = menu.deref_mut();
        if let Ok(item) = items.get(event.entity) {
            if let Some(target) = context_menu.target {
                commands.trigger(item.get_action().create_entity_event(target));
                context_menu.target = None;
            }
        }
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
    commands.spawn((
        ContextMenu::default(),
        Node {
            position_type: PositionType::Absolute,
            border: UiRect::all(px(1)),
            flex_direction: FlexDirection::Column,
            ..default()
        },
        MENU_BACKGROUND_DEFAULT,
        BorderColor::all(Color::WHITE),
        BorderRadius::all(px(3.0)),
        GlobalZIndex(100),
        Visibility::Hidden,
    ));
}

fn on_menu_hover(event: On<Pointer<Over>>, mut commands: Commands) {
    commands.entity(event.entity).insert(MENU_BACKGROUND_HIGHLIGHT);
}

fn on_menu_out(event: On<Pointer<Out>>, mut commands: Commands) {
    commands.entity(event.entity).remove::<BackgroundColor>();
}
