//! Live redraw of the editable geometry. The [`EditorModel`] is projected to world space and
//! drawn as one rotated rectangle per segment plus a draggable handle per vertex. Track meshes
//! are rebuilt only when the model is `dirty`; vertex highlight follows selection every frame.

use bevy::prelude::*;
use rail_dispatch::panel::{ScreenScale, TRACK_THICKNESS};

use crate::state::{COINCIDENT_EPS, EditorModel, EditorSelection, VertexRef};

const TRACK_Z: f32 = 0.0;
const VERTEX_Z: f32 = 3.0;
const GUIDE_Z: f32 = -1.0;
const VERTEX_SIZE: f32 = 6.0;
const GUIDE_THICKNESS: f32 = 1.0;

/// Extra grid rows drawn above and below the geometry's extent, and the pixel-space margin added
/// to each side of the guide lines, so the grid reads as "infinite" around the content.
const GUIDE_MARGIN_ROWS: i32 = 8;
const GUIDE_X_MARGIN: f32 = 40.0;

const TRACK_COLOR: Color = Color::srgb(0.55, 0.57, 0.60);
const VERTEX_COLOR: Color = Color::srgb(0.45, 0.62, 0.95);
const VERTEX_HOVERED: Color = Color::srgb(0.95, 0.82, 0.15);
const VERTEX_SELECTED: Color = Color::srgb(0.15, 0.85, 0.30);
const GHOST_COLOR: Color = Color::srgba(0.55, 0.95, 0.70, 0.55);
const GUIDE_COLOR: Color = Color::srgb(0.16, 0.18, 0.22);
const BG_COLOR: Color = Color::srgb(0.06, 0.07, 0.09);

/// Marks every entity spawned by [`redraw`] so the whole editable layer can be cleared at once.
#[derive(Component)]
pub(crate) struct EditorEntity;

/// A vertex handle, tagged with the model vertex it represents.
#[derive(Component)]
pub(crate) struct VertexHandle(VertexRef);

/// The translucent marker showing where a `Ctrl`+click would insert a vertex.
#[derive(Component)]
pub(crate) struct GhostVertex;

/// Shared meshes/materials for the editable layer.
#[derive(Resource)]
pub(crate) struct EditorAssets {
    unit_rect: Handle<Mesh>,
    vertex_mesh: Handle<Mesh>,
    track: Handle<ColorMaterial>,
    vertex: Handle<ColorMaterial>,
    vertex_hovered: Handle<ColorMaterial>,
    vertex_selected: Handle<ColorMaterial>,
}

pub fn setup_render_assets(
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut clear_color: ResMut<ClearColor>,
    mut commands: Commands,
) {
    *clear_color = ClearColor(BG_COLOR);
    let vertex_mesh = meshes.add(Rectangle::new(VERTEX_SIZE, VERTEX_SIZE));
    commands.insert_resource(EditorAssets {
        unit_rect: meshes.add(Rectangle::new(1.0, 1.0)),
        vertex_mesh: vertex_mesh.clone(),
        track: materials.add(ColorMaterial::from_color(TRACK_COLOR)),
        vertex: materials.add(ColorMaterial::from_color(VERTEX_COLOR)),
        vertex_hovered: materials.add(ColorMaterial::from_color(VERTEX_HOVERED)),
        vertex_selected: materials.add(ColorMaterial::from_color(VERTEX_SELECTED)),
    });

    // The insert-preview ghost: one persistent entity, repositioned and shown/hidden each frame.
    commands.spawn((
        GhostVertex,
        Mesh2d(vertex_mesh),
        MeshMaterial2d(materials.add(ColorMaterial::from_color(GHOST_COLOR))),
        Transform::from_translation(Vec3::new(0.0, 0.0, VERTEX_Z)),
        ScreenScale::Uniform,
        Visibility::Hidden,
    ));
}

/// Follow [`EditorSelection::insert_preview`]: show the ghost vertex at the would-be insert point
/// (projected to world space) when `Ctrl` is held near a segment, hide it otherwise.
pub fn update_ghost(
    selection: Res<EditorSelection>,
    model: Res<EditorModel>,
    ghost: Single<(&mut Transform, &mut Visibility), With<GhostVertex>>,
) {
    let (mut transform, mut visibility) = ghost.into_inner();
    match selection.insert_preview {
        Some(pixel) => {
            let world = model.transform.pixel_to_world(pixel);
            transform.translation.x = world.x;
            transform.translation.y = world.y;
            *visibility = Visibility::Visible;
        }
        None => *visibility = Visibility::Hidden,
    }
}

/// Faint horizontal guide line on a grid row.
#[derive(Component)]
struct GuideLine;

/// Draw the row grid as faint horizontal lines spanning the geometry's extent (plus margin),
/// once after the model is built. Static — not tagged [`EditorEntity`], so it survives redraws.
pub fn setup_guides(
    model: Res<EditorModel>,
    assets: Res<EditorAssets>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut commands: Commands,
) {
    let mut min = Vec2::splat(f32::INFINITY);
    let mut max = Vec2::splat(f32::NEG_INFINITY);
    for g in &model.geometry {
        for p in &g.points {
            min = min.min(*p);
            max = max.max(*p);
        }
    }
    if !min.x.is_finite() {
        return; // no geometry to anchor the grid around
    }

    let spacing = model.row_spacing;
    let first = (min.y / spacing).floor() as i32 - GUIDE_MARGIN_ROWS;
    let last = (max.y / spacing).ceil() as i32 + GUIDE_MARGIN_ROWS;
    let material = materials.add(ColorMaterial::from_color(GUIDE_COLOR));

    for row in first..=last {
        let y = row as f32 * spacing;
        let a = model.transform.pixel_to_world(Vec2::new(min.x - GUIDE_X_MARGIN, y));
        let b = model.transform.pixel_to_world(Vec2::new(max.x + GUIDE_X_MARGIN, y));
        let len = (b - a).length();
        commands.spawn((
            GuideLine,
            Mesh2d(assets.unit_rect.clone()),
            MeshMaterial2d(material.clone()),
            Transform::from_translation(((a + b) / 2.0).extend(GUIDE_Z)),
            ScreenScale::LineThickness {
                length: len,
                thickness: GUIDE_THICKNESS,
            },
        ));
    }
}

/// Rebuild the track segments and vertex handles whenever the model is `dirty`.
pub fn redraw(
    mut model: ResMut<EditorModel>,
    assets: Res<EditorAssets>,
    existing: Query<Entity, With<EditorEntity>>,
    mut commands: Commands,
) {
    if !model.dirty {
        return;
    }

    for e in &existing {
        commands.entity(e).despawn();
    }

    let transform = model.transform;
    for (block_idx, g) in model.geometry.iter().enumerate() {
        // segments
        for seg in g.points.windows(2) {
            let a = transform.pixel_to_world(seg[0]);
            let b = transform.pixel_to_world(seg[1]);
            let delta = b - a;
            let len = delta.length();
            if len < f32::EPSILON {
                continue;
            }
            commands.spawn((
                EditorEntity,
                Mesh2d(assets.unit_rect.clone()),
                MeshMaterial2d(assets.track.clone()),
                Transform {
                    translation: ((a + b) / 2.0).extend(TRACK_Z),
                    rotation: Quat::from_rotation_z(delta.y.atan2(delta.x)),
                    scale: Vec3::new(len, TRACK_THICKNESS, 1.0),
                },
                ScreenScale::LineThickness {
                    length: len,
                    thickness: TRACK_THICKNESS,
                },
            ));
        }
        // vertices
        for (point_idx, p) in g.points.iter().enumerate() {
            commands.spawn((
                EditorEntity,
                VertexHandle(VertexRef { block_idx, point_idx }),
                Mesh2d(assets.vertex_mesh.clone()),
                MeshMaterial2d(assets.vertex.clone()),
                Transform::from_translation(transform.pixel_to_world(*p).extend(VERTEX_Z)),
                ScreenScale::Uniform,
            ));
        }
    }

    model.dirty = false;
}

/// Recolour vertex handles from the current selection/hover (cheap, every frame). Matching is by
/// *position*, not by `VertexRef`, so the two coincident markers at a block junction both light up
/// (otherwise only one would, and the other — drawn on top — would hide it).
pub fn highlight_vertices(
    selection: Res<EditorSelection>,
    model: Res<EditorModel>,
    assets: Res<EditorAssets>,
    mut query: Query<(&VertexHandle, &mut MeshMaterial2d<ColorMaterial>)>,
) {
    let point_of = |v: VertexRef| {
        model
            .geometry
            .get(v.block_idx)
            .and_then(|g| g.points.get(v.point_idx))
            .copied()
    };
    let hovered = selection.hovered.and_then(point_of);
    let selected = selection.selected.and_then(point_of);

    for (handle, mut material) in &mut query {
        let Some(p) = point_of(handle.0) else { continue };
        let coincides = |q: Option<Vec2>| q.is_some_and(|q| q.distance(p) < COINCIDENT_EPS);
        let wanted = if coincides(selected) {
            &assets.vertex_selected
        } else if coincides(hovered) {
            &assets.vertex_hovered
        } else {
            &assets.vertex
        };
        if material.0.id() != wanted.id() {
            material.0 = wanted.clone();
        }
    }
}
