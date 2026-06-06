//! Schematic dispatcher panel (Dutch PRL-style dark schematic), split into two layers:
//!
//! - [`SchematicPlugin`] is the **simulation-agnostic** draw layer: it owns the camera
//!   and builds the static schematic (track lines and manual-signal glyphs) purely from
//!   the [`Level`] asset, so it can be mounted with no running simulation (e.g. the map
//!   editor renders the same schematic statically).
//! - [`PanelPlugin`] is the **game** layer: it adds the schematic, the camera pan/zoom
//!   controls, and the message-driven systems that colour track/signals and place the
//!   train-describer labels, plus picking/menus/tooltips.
//!
//! - Track is a continuous line per block (a rotated [`Rectangle`] mesh per segment).
//!   A block is yellow when occupied, green while pending under a set route, else gray
//!   (occupied > pending > free). The panel never polls: occupancy follows `BlockUpdate`,
//!   the pending path follows `RoutePending`, and the green path is consumed block-by-block
//!   as occupancy arrives.
//! - Only manual (route-protecting) signals are drawn, as a triangle that is green when open
//!   and subdued red when closed (driven by `SignalAspectChanged`) — closed signals stay
//!   visible so they can be clicked to set a route. No speed plates.
//! - The train describer is a number label anchored near the head block's leading end; it
//!   jumps from block to block on `TrainMove` as the head advances (it never slides).

use crate::assets::{AssetHandles, LoadingState};
use crate::common::{BlockId, Direction, RouteId, SignalId, SignalType, TrainId};
use crate::dropdown_menu::DropDownMenu;
use crate::level::Level;
use crate::simulation::block::{BlockMap, SignalAspectChanged, TrackState, TrackUpdate};
use crate::simulation::signal::SignalAspect;
use crate::simulation::spawner::{SpawnRequest, SpawnTrainType};
use crate::simulation::station::{RouteActivationRequest, RoutePending};
use crate::simulation::train::{Train, TrainDespawnRequest};
use bevy::ecs::system::{SystemParam, SystemParamItem};
use bevy::input::keyboard::Key;
use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll, MouseScrollUnit};
use bevy::prelude::*;
use bevy::text::TextBackgroundColor;
use std::collections::HashMap;
use std::ops::DerefMut;

/// Pixel-space (level) → world-space scale factor.
const SCALE: f32 = 4.0;
const TRACK_THICKNESS: f32 = 3.0;
const SIGNAL_SIZE: f32 = 9.0;
const SPAWNER_SIZE: f32 = 9.0;
/// Describer placement, in world units: inset back from the leading block end (along the
/// track). The plate sits centred on the track line (no perpendicular offset).
const DESCRIBER_INSET: f32 = 28.0;

const TRACK_Z: f32 = 0.0;
const SIGNAL_Z: f32 = 2.0;
const SPAWNER_Z: f32 = 2.0;
const DESCRIBER_Z: f32 = 5.0;

const BG_COLOR: Color = Color::srgb(0.06, 0.07, 0.09);
const TRACK_IDLE: Color = Color::srgb(0.55, 0.57, 0.60);
const TRACK_OCCUPIED: Color = Color::srgb(0.95, 0.82, 0.15);
const TRACK_PENDING: Color = Color::srgb(0.15, 0.80, 0.25);
const SPAWNER_COLOR: Color = Color::srgb(0.85, 0.85, 0.88);
const SIGNAL_GREEN: Color = Color::srgb(0.10, 0.85, 0.22);
const SIGNAL_CLOSED: Color = Color::srgb(0.60, 0.16, 0.16);
const DESCRIBER_TEXT: Color = Color::srgb(0.95, 0.96, 1.0);
const DESCRIBER_BG: Color = Color::srgb(0.10, 0.11, 0.13);

// ----------------------------------------------------------------------------------
// Geometry
// ----------------------------------------------------------------------------------

/// Per-block track polylines, in world space.
#[derive(Resource, Default)]
pub struct TrackGeometry {
    polylines: HashMap<BlockId, Vec<Vec2>>,
}

impl TrackGeometry {
    fn from_level(level: &Level) -> Self {
        let mut min = Vec2::splat(f32::INFINITY);
        let mut max = Vec2::splat(f32::NEG_INFINITY);
        for bg in &level.geometry {
            for p in bg.points.iter().copied() {
                min = min.min(p);
                max = max.max(p);
            }
        }
        let center = if min.x.is_finite() {
            (min + max) / 2.0
        } else {
            Vec2::ZERO
        };

        let polylines = level
            .geometry
            .iter()
            .map(|bg| {
                let pts = bg
                    .points
                    .iter()
                    // flip Y: level pixel space is y-down, world is y-up
                    .map(|p| Vec2::new((p.x - center.x) * SCALE, (center.y - p.y) * SCALE))
                    .collect();
                (bg.id, pts)
            })
            .collect();

        Self { polylines }
    }

    fn endpoints(&self, id: BlockId) -> Option<(Vec2, Vec2)> {
        let pts = self.polylines.get(&id)?;
        Some((*pts.first()?, *pts.last()?))
    }

    /// Unit vector along the even (forward) direction of the block.
    fn forward(&self, id: BlockId) -> Option<Vec2> {
        let (a, b) = self.endpoints(id)?;
        Some((b - a).normalize_or_zero())
    }

    /// Anchor for the train describer: inset back from the block's leading end (the end in
    /// the train's direction of travel), centred on the track line (no perpendicular offset).
    fn describer_anchor(&self, id: BlockId, direction: Direction) -> Option<Vec2> {
        let (first, last) = self.endpoints(id)?;
        let forward = (last - first).normalize_or_zero();
        if forward == Vec2::ZERO {
            return Some((first + last) / 2.0);
        }
        let (leading, interior) = match direction {
            Direction::Even => (last, -forward),
            Direction::Odd => (first, forward),
        };
        Some(leading + interior * DESCRIBER_INSET)
    }
}

// ----------------------------------------------------------------------------------
// Components / resources
// ----------------------------------------------------------------------------------

#[derive(Component)]
pub struct TrackSeg(BlockId);

#[derive(Component)]
pub struct SignalGlyph(SignalId);

#[derive(Component)]
struct SpawnerMarker(BlockId);

#[derive(Component)]
struct DescriberLabel;

#[derive(Component)]
struct PanelTooltip;

/// Live describer label entities keyed by train.
#[derive(Resource, Default)]
struct Describers(HashMap<TrainId, Entity>);

/// One shared track material per block (all of a block's polyline segments reference it),
/// so recolouring a block is a single material write. Built by the schematic layer.
#[derive(Resource, Default)]
struct BlockMaterials(HashMap<BlockId, Handle<ColorMaterial>>);

/// Panel-side display state per block, updated incrementally by messages (never polled).
#[derive(Resource, Default)]
struct BlockVisState(HashMap<BlockId, BlockVis>);

#[derive(Default, Clone, Copy)]
struct BlockVis {
    occupied: bool,
    pending: bool,
}

impl BlockVis {
    /// occupied (yellow) > pending route (green) > free (gray)
    fn color(self) -> Color {
        if self.occupied {
            TRACK_OCCUPIED
        } else if self.pending {
            TRACK_PENDING
        } else {
            TRACK_IDLE
        }
    }
}

fn paint_block(
    block_id: BlockId,
    vis: BlockVis,
    block_materials: &BlockMaterials,
    materials: &mut Assets<ColorMaterial>,
) {
    if let Some(handle) = block_materials.0.get(&block_id)
        && let Some(material) = materials.get_mut(handle)
    {
        material.color = vis.color();
    }
}

// ----------------------------------------------------------------------------------
// Camera control & screen-constant scaling
// ----------------------------------------------------------------------------------

const ZOOM_STEP: f32 = 1.15;
const MIN_SCALE: f32 = 0.15;
const MAX_SCALE: f32 = 8.0;

/// How an entity's `Transform.scale` is driven so it keeps a constant on-screen size as the
/// camera zooms. Base sizes are world units at `scale == 1`. Zoom is the orthographic
/// projection's `scale` (world units per pixel): on-screen size is `world / scale`, so we
/// hold size constant by setting world size to `base * scale`.
#[derive(Component, Clone, Copy)]
enum ScreenScale {
    /// Uniform glyph/label: scaled by the zoom factor on every axis.
    Uniform,
    /// A line whose length follows the world (zooms) but whose thickness stays
    /// screen-constant (e.g. track segments).
    LineThickness { length: f32, thickness: f32 },
}

/// Adds world-space camera pan/zoom input (wheel zoom, middle-drag pan). [`apply_screen_scale`]
/// is owned by [`SchematicPlugin`] so sizing is correct with or without this plugin; the game
/// panel and the map viewer both add it on top of `SchematicPlugin`.
pub struct CameraControlPlugin;

impl Plugin for CameraControlPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, camera_control);
    }
}

fn camera_control(
    scroll: Res<AccumulatedMouseScroll>,
    motion: Res<AccumulatedMouseMotion>,
    buttons: Res<ButtonInput<MouseButton>>,
    camera: Option<Single<(&mut Projection, &mut Transform), With<Camera2d>>>,
) {
    let Some(camera) = camera else { return };
    let (mut projection, mut transform) = camera.into_inner();
    let Projection::Orthographic(ortho) = projection.deref_mut() else {
        return;
    };

    let dy = match scroll.unit {
        MouseScrollUnit::Line => scroll.delta.y,
        MouseScrollUnit::Pixel => scroll.delta.y / 40.0,
    };
    if dy != 0.0 {
        // scroll up (positive) zooms in -> smaller scale
        ortho.scale = (ortho.scale * ZOOM_STEP.powf(-dy)).clamp(MIN_SCALE, MAX_SCALE);
    }

    if buttons.pressed(MouseButton::Middle) && motion.delta != Vec2::ZERO {
        // drag the content with the cursor: world delta is screen delta * scale, Y flipped
        transform.translation.x -= motion.delta.x * ortho.scale;
        transform.translation.y += motion.delta.y * ortho.scale;
    }
}

/// Drives every [`ScreenScale`] entity's `Transform.scale` from the camera's current
/// orthographic scale so the entity keeps a constant on-screen size.
fn apply_screen_scale(
    camera: Option<Single<&Projection, With<Camera2d>>>,
    mut query: Query<(&mut Transform, &ScreenScale)>,
) {
    let Some(projection) = camera else { return };
    let Projection::Orthographic(ortho) = projection.into_inner() else {
        return;
    };
    let s = ortho.scale;

    for (mut transform, screen_scale) in &mut query {
        transform.scale = match *screen_scale {
            ScreenScale::Uniform => Vec3::splat(s),
            ScreenScale::LineThickness { length, thickness } => Vec3::new(length, thickness * s, 1.0),
        };
    }
}

// ----------------------------------------------------------------------------------
// Schematic layer (simulation-agnostic): camera + static track/signal drawing
// ----------------------------------------------------------------------------------

/// Draws the static schematic from the [`Level`] asset and owns the world camera. No
/// dependency on the simulation, so it can be mounted on its own (e.g. the map editor).
pub struct SchematicPlugin;

impl Plugin for SchematicPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_camera)
            .add_systems(OnExit(LoadingState::Loading), setup_schematic)
            .add_systems(Update, apply_screen_scale);
    }
}

fn spawn_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
}

pub fn setup_schematic(
    handles: Res<AssetHandles>,
    levels: Res<Assets<Level>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut clear_color: ResMut<ClearColor>,
    mut commands: Commands,
) {
    let level = levels.get(&handles.level).expect("level had been loaded");
    let geometry = TrackGeometry::from_level(level);
    *clear_color = ClearColor(BG_COLOR);

    let unit_rect = meshes.add(Rectangle::new(1.0, 1.0));
    let tri = meshes.add(Triangle2d::new(
        Vec2::new(SIGNAL_SIZE, 0.0),
        Vec2::new(-SIGNAL_SIZE * 0.6, SIGNAL_SIZE * 0.75),
        Vec2::new(-SIGNAL_SIZE * 0.6, -SIGNAL_SIZE * 0.75),
    ));

    // --- track segments (all segments of a block share one material for cheap recolour) ---
    let mut block_materials: HashMap<BlockId, Handle<ColorMaterial>> = HashMap::new();
    for bg in &level.geometry {
        let Some(pts) = geometry.polylines.get(&bg.id) else {
            continue;
        };
        let material = block_materials
            .entry(bg.id)
            .or_insert_with(|| materials.add(ColorMaterial::from_color(TRACK_IDLE)))
            .clone();
        for seg in pts.windows(2) {
            let (a, b) = (seg[0], seg[1]);
            let mid = (a + b) / 2.0;
            let delta = b - a;
            let len = delta.length();
            if len < f32::EPSILON {
                continue;
            }
            let angle = delta.y.atan2(delta.x);
            commands.spawn((
                TrackSeg(bg.id),
                Mesh2d(unit_rect.clone()),
                MeshMaterial2d(material.clone()),
                Transform {
                    translation: mid.extend(TRACK_Z),
                    rotation: Quat::from_rotation_z(angle),
                    scale: Vec3::new(len, TRACK_THICKNESS, 1.0),
                },
                ScreenScale::LineThickness {
                    length: len,
                    thickness: TRACK_THICKNESS,
                },
                Pickable::default(),
            ));
        }
    }

    // --- signals: only manual (route-protecting) signals are drawn; automatic signals
    // are invisible to the dispatcher. The glyph is a triangle at the guarded block boundary,
    // apex along the governed direction; green when open, subdued red when closed (so it is
    // still visible and clickable for setting a route). Signals start closed. ---
    for s in &level.signals {
        if s.signal_type != SignalType::Manual {
            continue;
        }
        let (Some((first, last)), Some(forward)) = (geometry.endpoints(s.block_id), geometry.forward(s.block_id))
        else {
            continue;
        };
        let length = level
            .blocks
            .iter()
            .find(|b| b.id == s.block_id)
            .map_or(0.0, |b| b.length);
        // offset near the even end -> last point, otherwise the odd end -> first point
        let node = if s.offset_m >= length / 2.0 { last } else { first };
        let apex = if s.direction == Direction::Even {
            forward
        } else {
            -forward
        };
        let angle = apex.y.atan2(apex.x);
        commands.spawn((
            SignalGlyph(s.id),
            Mesh2d(tri.clone()),
            MeshMaterial2d(materials.add(ColorMaterial::from_color(SIGNAL_CLOSED))),
            Transform {
                translation: node.extend(SIGNAL_Z),
                rotation: Quat::from_rotation_z(angle),
                ..default()
            },
            ScreenScale::Uniform,
            Pickable::default(),
        ));
    }

    commands.insert_resource(geometry);
    commands.insert_resource(BlockMaterials(block_materials));
}

// ----------------------------------------------------------------------------------
// Game panel layer: camera control + simulation-driven colouring, labels, picking
// ----------------------------------------------------------------------------------

pub struct PanelPlugin;

impl Plugin for PanelPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((SchematicPlugin, CameraControlPlugin))
            .init_resource::<Describers>()
            .init_resource::<BlockVisState>()
            .add_systems(Startup, startup)
            .add_systems(
                OnExit(LoadingState::Loading),
                attach_panel_interactions.after(setup_schematic),
            )
            .add_systems(OnEnter(LoadingState::Instantiated), setup_spawners)
            .add_systems(
                Update,
                (
                    apply_block_updates,
                    apply_route_pending,
                    apply_signal_aspects,
                    apply_train_describers,
                    despawn_describers,
                )
                    .run_if(in_state(LoadingState::Instantiated)),
            );
    }
}

fn startup(mut commands: Commands) {
    commands.add_observer(on_route_menu_action);
    commands.add_observer(on_spawner_menu_action);

    commands
        .spawn((
            PanelTooltip,
            Node {
                position_type: PositionType::Absolute,
                border: UiRect::all(px(1)),
                padding: UiRect::all(px(5)),
                ..default()
            },
            BackgroundColor(Color::srgb(0.12, 0.13, 0.16)),
            BorderColor::all(Color::srgb(0.4, 0.42, 0.48)),
            GlobalZIndex(99),
            Pickable::IGNORE,
            Visibility::Hidden,
        ))
        .with_children(|p| {
            p.spawn((Text::default(), TextFont::from_font_size(11.0), Pickable::IGNORE));
        });
}

/// Wire up picking on the schematic entities spawned by [`setup_schematic`]: the route
/// menu on signal glyphs, and hover tooltips on both track segments and signal glyphs.
fn attach_panel_interactions(
    tracks: Query<Entity, With<TrackSeg>>,
    signals: Query<Entity, With<SignalGlyph>>,
    mut commands: Commands,
) {
    let signal_entities: Vec<Entity> = signals.iter().collect();
    let info_entities: Vec<Entity> = tracks.iter().chain(signal_entities.iter().copied()).collect();

    PanelRouteMenu::register(&mut commands, signal_entities);
    commands.spawn(Observer::new(on_info_over).with_entities(info_entities.iter().copied()));
    commands.spawn(Observer::new(on_info_out).with_entities(info_entities));
}

fn setup_spawners(
    handles: Res<AssetHandles>,
    levels: Res<Assets<Level>>,
    block_map: Res<BlockMap>,
    geometry: Res<TrackGeometry>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut commands: Commands,
) {
    use crate::level::SpawnerKind;
    let level = levels.get(&handles.level).expect("level had been loaded");
    let marker_mesh = meshes.add(Rectangle::new(SPAWNER_SIZE, SPAWNER_SIZE));
    let mut spawner_entities: Vec<Entity> = Vec::new();

    for data in &level.spawners {
        if data.kind == SpawnerKind::Despawn {
            continue;
        }
        let Some(block) = block_map.get_block(data.block_id) else {
            continue;
        };
        let Some(end) = block.get_end_direction() else {
            continue;
        };
        let Some((first, last)) = geometry.endpoints(data.block_id) else {
            continue;
        };
        // open end: Odd => odd (first) end has no predecessor, Even => even (last) end
        let pos = match end {
            Direction::Odd => first,
            Direction::Even => last,
        };
        let entity = commands
            .spawn((
                SpawnerMarker(data.block_id),
                Mesh2d(marker_mesh.clone()),
                MeshMaterial2d(materials.add(ColorMaterial::from_color(SPAWNER_COLOR))),
                Transform::from_translation(pos.extend(SPAWNER_Z)),
                ScreenScale::Uniform,
                Pickable::default(),
            ))
            .id();
        spawner_entities.push(entity);
    }

    PanelSpawnerMenu::register(&mut commands, spawner_entities);
}

// ----------------------------------------------------------------------------------
// Message-driven updates (no per-frame polling of simulation state)
// ----------------------------------------------------------------------------------

/// Occupancy: a block flips yellow when a train enters and back to its underlying state
/// when it clears. Entering a block also consumes its pending (green) flag for good, so a
/// traversed route block stays gray behind the train.
fn apply_block_updates(
    mut updates: MessageReader<TrackUpdate>,
    mut state: ResMut<BlockVisState>,
    block_materials: Res<BlockMaterials>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    for update in updates.read() {
        for block_id in update.blocks() {
            let vis = state.0.entry(*block_id).or_default();
            match update.state {
                TrackState::Occupied => {
                    vis.occupied = true;
                    vis.pending = false;
                }
                TrackState::Freed => vis.occupied = false,
            }
            paint_block(*block_id, *vis, &block_materials, &mut materials);
        }
    }
}

/// Route path: blocks light green when a route is set and drop back to free when it clears.
/// The green path is then eaten block-by-block by [`apply_block_updates`] as the train runs.
fn apply_route_pending(
    mut updates: MessageReader<RoutePending>,
    mut state: ResMut<BlockVisState>,
    block_materials: Res<BlockMaterials>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    for update in updates.read() {
        for &block_id in &update.blocks {
            let vis = state.0.entry(block_id).or_default();
            vis.pending = update.pending;
            paint_block(block_id, *vis, &block_materials, &mut materials);
        }
    }
}

/// Manual signal glyphs are green when open and subdued red when closed (Forbidding), so a
/// closed signal stays visible and clickable. Glyphs exist only for manual signals; changes
/// for automatic signals match no glyph and are ignored.
fn apply_signal_aspects(
    mut changes: MessageReader<SignalAspectChanged>,
    query: Query<(&SignalGlyph, &MeshMaterial2d<ColorMaterial>)>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    for change in changes.read() {
        let color = if change.aspect == SignalAspect::Forbidding {
            SIGNAL_CLOSED
        } else {
            SIGNAL_GREEN
        };
        for (glyph, material) in &query {
            if glyph.0 == change.signal_id
                && let Some(mat) = materials.get_mut(&material.0)
            {
                mat.color = color;
            }
        }
    }
}

/// Given a section update, return a block for a describer label. The block is always on the
/// end of the section which is opposite from where the train had entered.
/// For regular single block updates, returns the updated block id.
fn get_describer_block(update: &TrackUpdate) -> BlockId {
    let maybe_block = match &update.section_ctx {
        Some(ctx) => {
            let last_index = ctx.get_blocks_len() - 1;
            match ctx.get_block_index(&update.block_id) {
                Some(0) => ctx.blocks.last(),
                Some(i) if i == last_index => ctx.blocks.first(),
                _ => None,
            }
        }
        None => None,
    };
    maybe_block.copied().unwrap_or(update.block_id)
}

/// Moves (or creates) a train's describer to its head block whenever the train steps over a
/// block boundary. The label jumps block-to-block; it never slides with continuous position.
fn apply_train_describers(
    mut updates: MessageReader<TrackUpdate>,
    geometry: Res<TrackGeometry>,
    mut describers: ResMut<Describers>,
    mut labels: Query<(&mut Transform, &mut Text2d)>,
    mut commands: Commands,
) {
    for update in updates.read().filter(|u| u.state == TrackState::Occupied) {
        let block = get_describer_block(update);
        let Some(pos) = geometry.describer_anchor(block, update.train_direction) else {
            continue;
        };
        if let Some(&entity) = describers.0.get(&update.train_id) {
            if let Ok((mut transform, mut text)) = labels.get_mut(entity) {
                transform.translation = pos.extend(DESCRIBER_Z);
                if text.0 != update.train_number {
                    text.0 = update.train_number.clone();
                }
            }
        } else {
            let entity = commands
                .spawn((
                    DescriberLabel,
                    Text2d::new(update.train_number.clone()),
                    TextFont::from_font_size(14.0),
                    TextColor(DESCRIBER_TEXT),
                    TextBackgroundColor(DESCRIBER_BG),
                    Transform::from_translation(pos.extend(DESCRIBER_Z)),
                    ScreenScale::Uniform,
                ))
                .id();
            describers.0.insert(update.train_id, entity);
        }
    }
}

fn despawn_describers(
    mut requests: MessageReader<TrainDespawnRequest>,
    mut describers: ResMut<Describers>,
    mut commands: Commands,
) {
    for request in requests.read() {
        if let Some(entity) = describers.0.remove(&request.id) {
            commands.entity(entity).despawn();
        }
    }
}

// ----------------------------------------------------------------------------------
// Hover tooltip
// ----------------------------------------------------------------------------------

fn on_info_over(
    event: On<Pointer<Over>>,
    block_map: Res<BlockMap>,
    trains: Query<&Train>,
    tracks: Query<&TrackSeg>,
    signals: Query<&SignalGlyph>,
    mut info: Single<(&Children, &mut Visibility, &mut Node), With<PanelTooltip>>,
    mut writer: TextUiWriter,
) {
    let target = event.entity;
    let text = if let Ok(seg) = tracks.get(target) {
        match block_map.block_trains(seg.0).and_then(|t| t.first()).copied() {
            Some(first) => match trains.iter().find(|t| t.id == first) {
                Some(train) => format!(
                    "Block {} — train {} ({:.0} km/h)",
                    seg.0,
                    train.number,
                    train.get_speed_kmh()
                ),
                None => format!("Block {} — occupied", seg.0),
            },
            None => format!("Block {} — free", seg.0),
        }
    } else if let Ok(glyph) = signals.get(target) {
        match block_map.signal(glyph.0) {
            Some(signal) => format!("Signal {} ({})", signal.name, signal.id),
            None => return,
        }
    } else {
        return;
    };

    let (children, vis, node) = info.deref_mut();
    *writer.text(children[0], 0) = text;
    **vis = Visibility::Visible;
    node.left = px(event.pointer_location.position.x + 12.0);
    node.top = px(event.pointer_location.position.y + 12.0);
}

fn on_info_out(_: On<Pointer<Out>>, mut vis: Single<&mut Visibility, With<PanelTooltip>>) {
    **vis = Visibility::Hidden;
}

// ----------------------------------------------------------------------------------
// Context menus (reuse the generic DropDownMenu machinery)
// ----------------------------------------------------------------------------------

#[derive(EntityEvent)]
struct PanelRouteMenuEvent {
    entity: Entity,
    route_id: RouteId,
}

#[derive(Component, Clone, Copy)]
struct PanelRouteMenu(RouteId);

#[derive(SystemParam)]
struct RouteMenuContext<'w, 's> {
    handles: Res<'w, AssetHandles>,
    levels: Res<'w, Assets<Level>>,
    glyphs: Query<'w, 's, &'static SignalGlyph>,
}

impl DropDownMenu for PanelRouteMenu {
    type Event<'a> = PanelRouteMenuEvent;
    type Context = RouteMenuContext<'static, 'static>;

    fn create_event(&self, entity: Entity) -> Self::Event<'_> {
        PanelRouteMenuEvent {
            entity,
            route_id: self.0,
        }
    }

    fn get_label(&self) -> impl Into<String> {
        format!("Open route {}", self.0)
    }

    fn list_available_items(
        target: Entity,
        ctx: &mut SystemParamItem<Self::Context>,
    ) -> impl IntoIterator<Item = Self> {
        let mut items = Vec::new();
        let Ok(glyph) = ctx.glyphs.get(target) else {
            return items;
        };
        let Some(level) = ctx.levels.get(&ctx.handles.level) else {
            return items;
        };
        for route in level.stations.iter().flat_map(|s| s.routes.iter()) {
            if route.signal == glyph.0 {
                items.push(PanelRouteMenu(route.id));
            }
        }
        items
    }

    fn key_filter(keyboard_input: Res<ButtonInput<Key>>) -> bool {
        !keyboard_input.pressed(Key::Control)
    }
}

fn on_route_menu_action(event: On<PanelRouteMenuEvent>, mut requests: MessageWriter<RouteActivationRequest>) {
    requests.write(RouteActivationRequest {
        route_id: event.route_id,
    });
}

#[derive(EntityEvent)]
struct PanelSpawnerMenuEvent {
    entity: Entity,
    action: PanelSpawnerMenu,
}

#[derive(Component, Clone, Copy)]
enum PanelSpawnerMenu {
    Cargo,
    Passenger,
    Locomotive,
}

impl DropDownMenu for PanelSpawnerMenu {
    type Event<'a> = PanelSpawnerMenuEvent;
    type Context = ();

    fn create_event(&self, entity: Entity) -> Self::Event<'_> {
        PanelSpawnerMenuEvent { entity, action: *self }
    }

    fn get_label(&self) -> impl Into<String> {
        match self {
            PanelSpawnerMenu::Cargo => "Spawn Cargo Train",
            PanelSpawnerMenu::Passenger => "Spawn Passenger Train",
            PanelSpawnerMenu::Locomotive => "Spawn Locomotive Only",
        }
    }

    fn list_available_items(_: Entity, _: &mut SystemParamItem<Self::Context>) -> impl IntoIterator<Item = Self> {
        [
            PanelSpawnerMenu::Cargo,
            PanelSpawnerMenu::Passenger,
            PanelSpawnerMenu::Locomotive,
        ]
    }
}

fn on_spawner_menu_action(event: On<PanelSpawnerMenuEvent>, query: Query<&SpawnerMarker>, mut commands: Commands) {
    if let Ok(spawner) = query.get(event.entity) {
        let train_type = match event.action {
            PanelSpawnerMenu::Cargo => SpawnTrainType::Cargo,
            PanelSpawnerMenu::Passenger => SpawnTrainType::Passenger,
            PanelSpawnerMenu::Locomotive => SpawnTrainType::Locomotive,
        };
        commands.trigger(SpawnRequest {
            block_id: spawner.0,
            train_type,
        });
    }
}
