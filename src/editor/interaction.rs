//! Mouse/keyboard editing: hover + drag vertices, create blocks, connect by snapping.
//!
//! Left-click a vertex to drag it; `Ctrl`+left-click on a segment inserts a vertex there; a plain
//! left-drag on a block's track moves the whole block (shape preserved). Middle drag pans and the
//! wheel zooms (the shared camera controls). The cursor is mapped to pixel space and matched
//! against model vertices; junction vertices coincident at drag start move together so connected
//! blocks stay joined.

use bevy::prelude::*;

use crate::constraints::{MIN_SEGMENT_PX, normalize};
use crate::state::{
    BlockDrag, BlockRow, COINCIDENT_EPS, DEFAULT_BLOCK_LENGTH, EditMode, EditorModel, EditorSelection, GeomRow,
    VertexRef,
};
use rail_dispatch::common::BlockId;
use rail_dispatch::panel::SCALE;

/// Screen-pixel radius for picking a vertex under the cursor.
const PICK_RADIUS_PX: f32 = 10.0;
/// Pixel-space radius for snapping a dragged endpoint onto another block's endpoint.
const CONNECT_SNAP_PX: f32 = 8.0;
/// Screen-pixel radius for clicking a track segment to insert a vertex.
const INSERT_RADIUS_PX: f32 = 8.0;

/// Camera state needed to map the cursor into pixel space, gathered once per system.
struct CursorCtx {
    pixel: Vec2,
    /// Pixel-space distance covered by one screen pixel (`ortho.scale / SCALE`).
    pixel_per_screen: f32,
}

fn cursor_ctx(
    window: &Window,
    camera: &Camera,
    cam_transform: &GlobalTransform,
    projection: &Projection,
    transform: rail_dispatch::panel::TrackTransform,
) -> Option<CursorCtx> {
    let cursor = window.cursor_position()?;
    let world = camera.viewport_to_world_2d(cam_transform, cursor).ok()?;
    let Projection::Orthographic(ortho) = projection else {
        return None;
    };
    Some(CursorCtx {
        pixel: transform.world_to_pixel(world),
        pixel_per_screen: ortho.scale / SCALE,
    })
}

/// Nearest model vertex to `pixel` within `radius` (pixel space), as a `(ref, distance)`.
fn nearest_vertex(model: &EditorModel, pixel: Vec2, radius: f32) -> Option<VertexRef> {
    let mut best: Option<(VertexRef, f32)> = None;
    for (block_idx, g) in model.geometry.iter().enumerate() {
        for (point_idx, p) in g.points.iter().enumerate() {
            let d = p.distance(pixel);
            if d <= radius && best.is_none_or(|(_, bd)| d < bd) {
                best = Some((VertexRef { block_idx, point_idx }, d));
            }
        }
    }
    best.map(|(r, _)| r)
}

/// Nearest track segment to `pixel` within `radius` (pixel space), as `(block_idx, seg_idx,
/// projected_point)` where `seg_idx` is the index of the segment's first vertex.
fn nearest_segment(model: &EditorModel, pixel: Vec2, radius: f32) -> Option<(usize, usize, Vec2)> {
    let mut best: Option<(usize, usize, Vec2, f32)> = None;
    for (block_idx, g) in model.geometry.iter().enumerate() {
        for seg_idx in 0..g.points.len().saturating_sub(1) {
            let a = g.points[seg_idx];
            let ab = g.points[seg_idx + 1] - a;
            let len_sq = ab.length_squared();
            let t = if len_sq > f32::EPSILON {
                ((pixel - a).dot(ab) / len_sq).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let proj = a + ab * t;
            let d = proj.distance(pixel);
            if d <= radius && best.is_none_or(|(_, _, _, bd)| d < bd) {
                best = Some((block_idx, seg_idx, proj, d));
            }
        }
    }
    best.map(|(bi, si, p, _)| (bi, si, p))
}

type CameraQuery<'w, 's> =
    Single<'w, 's, (&'static Camera, &'static GlobalTransform, &'static Projection), With<Camera2d>>;

/// Hover highlight when idle (not dragging, not placing a block), plus the `Ctrl`-insert preview.
pub fn update_hover(
    keys: Res<ButtonInput<KeyCode>>,
    window: Option<Single<&Window>>,
    camera: Option<CameraQuery>,
    model: Res<EditorModel>,
    mut selection: ResMut<EditorSelection>,
) {
    if selection.dragging || selection.mode != EditMode::Idle {
        selection.insert_preview = None;
        return;
    }
    let (Some(window), Some(camera)) = (window, camera) else {
        return;
    };
    let (cam, cam_t, proj) = *camera;
    let Some(ctx) = cursor_ctx(&window, cam, cam_t, proj, model.transform) else {
        selection.hovered = None;
        selection.insert_preview = None;
        return;
    };
    selection.hovered = nearest_vertex(&model, ctx.pixel, PICK_RADIUS_PX * ctx.pixel_per_screen);

    // Ghost insert marker: only when Ctrl is held, near a segment, and not over an existing vertex.
    let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    selection.insert_preview = (ctrl && selection.hovered.is_none())
        .then(|| nearest_segment(&model, ctx.pixel, INSERT_RADIUS_PX * ctx.pixel_per_screen))
        .flatten()
        .map(|(_, _, proj)| proj);
}

/// Press → grab a hovered vertex, or (with `Ctrl`) insert a vertex on a clicked segment, else
/// pick up the whole block under the cursor. Drag → move the vertex group / translate the block.
/// Release → snap-connect endpoints & finalise.
pub fn handle_drag(
    buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    window: Option<Single<&Window>>,
    camera: Option<CameraQuery>,
    mut model: ResMut<EditorModel>,
    mut selection: ResMut<EditorSelection>,
) {
    if selection.mode != EditMode::Idle {
        return;
    }
    let (Some(window), Some(camera)) = (window, camera) else {
        return;
    };
    let (cam, cam_t, proj) = *camera;
    let Some(ctx) = cursor_ctx(&window, cam, cam_t, proj, model.transform) else {
        return;
    };
    let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);

    if buttons.just_pressed(MouseButton::Left) {
        if let Some(v) = selection.hovered {
            // Grab an existing vertex (and its coincident junction partners).
            selection.selected = Some(v);
            selection.dragging = true;
            selection.drag_group = coincident_group(&model, v);
        } else if let Some((block_idx, seg_idx, proj)) = ctrl
            .then(|| nearest_segment(&model, ctx.pixel, INSERT_RADIUS_PX * ctx.pixel_per_screen))
            .flatten()
        {
            // Ctrl+click on empty track: insert a new interior vertex splitting that segment, then
            // drag it. This is how diagonals are made — pull the new point onto another row.
            model.geometry[block_idx].points.insert(seg_idx + 1, proj);
            let v = VertexRef {
                block_idx,
                point_idx: seg_idx + 1,
            };
            selection.selected = Some(v);
            selection.dragging = true;
            selection.drag_group = vec![v];
            model.dirty = true;
        } else if let Some((block_idx, ..)) =
            nearest_segment(&model, ctx.pixel, INSERT_RADIUS_PX * ctx.pixel_per_screen)
        {
            // Plain click on a block's track: translate the whole block (shape preserved).
            selection.block_drag = Some(BlockDrag {
                block_idx,
                start_cursor: ctx.pixel,
                start_points: model.geometry[block_idx].points.clone(),
            });
        }
    }

    if selection.dragging && buttons.pressed(MouseButton::Left) {
        // Snap Y to the row grid; X follows the cursor freely. The dragged group is
        // authoritative; constraints adjust the neighbours to keep diagonals at the locked angle.
        let group = &selection.drag_group;
        let raw = Vec2::new(ctx.pixel.x, model.snap_row(ctx.pixel.y));
        // Keep the dragged vertex at least a minimum length from its own block's neighbours so two
        // vertices of a block can't be welded together.
        let target = clamp_min_separation(&model, group, raw);
        for v in group {
            model.geometry[v.block_idx].points[v.point_idx] = target;
        }
        normalize(&mut model, group);
        model.dirty = true;
    } else if buttons.pressed(MouseButton::Left)
        && let Some(drag) = &selection.block_drag
    {
        // Translate every vertex by the cursor delta. Snap the vertical shift to whole grid rows
        // so the block stays grid-aligned; horizontal motion is free. Shape (and thus the locked
        // angles / horizontal stubs) is preserved by construction.
        let delta = Vec2::new(
            ctx.pixel.x - drag.start_cursor.x,
            model.snap_row(ctx.pixel.y - drag.start_cursor.y),
        );
        let points = &mut model.geometry[drag.block_idx].points;
        for (p, start) in points.iter_mut().zip(&drag.start_points) {
            *p = *start + delta;
        }
        model.dirty = true;
    }

    if buttons.just_released(MouseButton::Left) {
        if selection.dragging
            && let Some(v) = selection.selected
        {
            snap_connect(&mut model, v);
            normalize(&mut model, &[v]);
            prune_connections(&mut model);
            model.dirty = true;
        } else if let Some(drag) = selection.block_drag.take() {
            // Reconnect either end if it landed on another block's endpoint.
            let last = model.geometry[drag.block_idx].points.len() - 1;
            for point_idx in [0, last] {
                snap_connect(
                    &mut model,
                    VertexRef {
                        block_idx: drag.block_idx,
                        point_idx,
                    },
                );
            }
            normalize(&mut model, &[]);
            prune_connections(&mut model);
            model.dirty = true;
        }
        selection.dragging = false;
        selection.block_drag = None;
        selection.drag_group.clear();
    }
}

/// `Delete`/`Backspace` removes the selected vertex (merging its two segments), as long as the
/// block keeps at least two points.
pub fn handle_delete(
    keys: Res<ButtonInput<KeyCode>>,
    mut model: ResMut<EditorModel>,
    mut selection: ResMut<EditorSelection>,
) {
    if selection.dragging || !(keys.just_pressed(KeyCode::Delete) || keys.just_pressed(KeyCode::Backspace)) {
        return;
    }
    let Some(v) = selection.selected else { return };
    if model.geometry[v.block_idx].points.len() <= 2 {
        return;
    }
    model.geometry[v.block_idx].points.remove(v.point_idx);
    selection.selected = None;
    selection.hovered = None;
    normalize(&mut model, &[]);
    prune_connections(&mut model);
    model.dirty = true;
}

/// All vertices coincident with `v`'s point (within epsilon), across every polyline.
fn coincident_group(model: &EditorModel, v: VertexRef) -> Vec<VertexRef> {
    let anchor = model.geometry[v.block_idx].points[v.point_idx];
    let mut group = Vec::new();
    for (block_idx, g) in model.geometry.iter().enumerate() {
        for (point_idx, p) in g.points.iter().enumerate() {
            if p.distance(anchor) < COINCIDENT_EPS {
                group.push(VertexRef { block_idx, point_idx });
            }
        }
    }
    group
}

/// Clamp `target` against every other vertex of each dragged group vertex's own block so a block
/// can neither weld nor flip. For a sibling on the **same row** the dragged vertex is kept on its
/// correct side by index order (it cannot cross it), preserving the block's left→right ordering.
/// For a sibling on a **different row** it is only kept from coinciding (diagonals are positioned
/// by the angle lock, so they never need an ordering clamp).
fn clamp_min_separation(model: &EditorModel, group: &[VertexRef], mut target: Vec2) -> Vec2 {
    for v in group {
        for (idx, &p) in model.geometry[v.block_idx].points.iter().enumerate() {
            // Skip the dragged vertex itself and any coincident group partner in this block.
            if group.iter().any(|g| g.block_idx == v.block_idx && g.point_idx == idx) {
                continue;
            }
            if (target.y - p.y).abs() < COINCIDENT_EPS {
                // Same row: hold X order with a minimum gap — earlier index stays left.
                if v.point_idx < idx {
                    target.x = target.x.min(p.x - MIN_SEGMENT_PX);
                } else {
                    target.x = target.x.max(p.x + MIN_SEGMENT_PX);
                }
            } else if target.distance(p) < MIN_SEGMENT_PX {
                let dir = (target - p).normalize_or_zero();
                let dir = if dir == Vec2::ZERO { Vec2::X } else { dir };
                target = p + dir * MIN_SEGMENT_PX;
            }
        }
    }
    target
}

/// If `v` is a terminal vertex sitting near another block's terminal vertex, snap exactly onto
/// it and record a connection (ordered smaller-x → larger-x, the file's even direction).
fn snap_connect(model: &mut EditorModel, v: VertexRef) {
    let n = model.geometry[v.block_idx].points.len();
    if v.point_idx != 0 && v.point_idx != n - 1 {
        return; // only endpoints imply connections
    }
    let me = model.geometry[v.block_idx].points[v.point_idx];
    let my_id = model.geometry[v.block_idx].id;

    let mut hit: Option<(usize, usize)> = None;
    for (block_idx, g) in model.geometry.iter().enumerate() {
        if block_idx == v.block_idx {
            continue;
        }
        let last = g.points.len() - 1;
        for &pi in &[0usize, last] {
            if g.points[pi].distance(me) <= CONNECT_SNAP_PX {
                hit = Some((block_idx, pi));
                break;
            }
        }
        if hit.is_some() {
            break;
        }
    }

    let Some((other_block, other_pi)) = hit else {
        return;
    };
    let other_point = model.geometry[other_block].points[other_pi];
    let other_id = model.geometry[other_block].id;

    // Refuse a connection that would land the dragged endpoint on top of one of its own block's
    // other vertices (which would collapse the block). This happens when the neighbour we'd snap
    // to is itself a junction sitting at the dragged block's other end.
    if model.geometry[v.block_idx]
        .points
        .iter()
        .enumerate()
        .any(|(i, p)| i != v.point_idx && p.distance(other_point) < MIN_SEGMENT_PX)
    {
        return;
    }

    // Snap exactly onto the partner, moving the whole coincident group with it.
    for g in coincident_group(model, v) {
        model.geometry[g.block_idx].points[g.point_idx] = other_point;
    }

    // Order by pixel-x: the left (smaller-x) block precedes in the even (+x) direction.
    let (start, end) = if me.x <= other_point.x {
        (my_id, other_id)
    } else {
        (other_id, my_id)
    };
    add_connection(model, start, end);
}

/// Drop every connection whose two blocks no longer share an endpoint, e.g. after a block was
/// dragged away from a junction or its terminal vertex was deleted. Keeps `connections` (which is
/// saved as the level topology) consistent with the drawn geometry.
fn prune_connections(model: &mut EditorModel) {
    let geometry = &model.geometry;
    let ends = |id: BlockId| {
        geometry
            .iter()
            .find(|g| g.id == id)
            .and_then(|g| Some([*g.points.first()?, *g.points.last()?]))
    };
    model.connections.retain(|&(a, b)| match (ends(a), ends(b)) {
        (Some(ea), Some(eb)) => ea.iter().any(|p| eb.iter().any(|q| p.distance(*q) < COINCIDENT_EPS)),
        _ => false,
    });
}

fn add_connection(model: &mut EditorModel, start: BlockId, end: BlockId) {
    if start != end && !model.connections.contains(&(start, end)) {
        model.connections.push((start, end));
    }
}

/// `B` toggles block-placement mode; clicks place the two endpoints of a new horizontal block.
pub fn handle_new_block(
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    window: Option<Single<&Window>>,
    camera: Option<CameraQuery>,
    mut model: ResMut<EditorModel>,
    mut selection: ResMut<EditorSelection>,
) {
    if keys.just_pressed(KeyCode::KeyB) {
        selection.mode = match selection.mode {
            EditMode::PlacingBlock => EditMode::Idle,
            EditMode::Idle => EditMode::PlacingBlock,
        };
        selection.pending_start = None;
    }
    if keys.just_pressed(KeyCode::Escape) {
        selection.mode = EditMode::Idle;
        selection.pending_start = None;
    }
    if selection.mode != EditMode::PlacingBlock {
        return;
    }

    let (Some(window), Some(camera)) = (window, camera) else {
        return;
    };
    let (cam, cam_t, proj) = *camera;
    let Some(ctx) = cursor_ctx(&window, cam, cam_t, proj, model.transform) else {
        return;
    };

    if buttons.just_pressed(MouseButton::Left) {
        let point = Vec2::new(ctx.pixel.x, model.snap_row(ctx.pixel.y));
        match selection.pending_start.take() {
            None => selection.pending_start = Some(point),
            Some(start) => {
                // a horizontal stub on the start row (constraints enforce the rest), at least
                // MIN_SEGMENT_PX long so it is never created welded to a point
                let id = model.next_block_id();
                let dx = point.x - start.x;
                let len = dx.abs().max(MIN_SEGMENT_PX);
                let end = Vec2::new(start.x + len * if dx < 0.0 { -1.0 } else { 1.0 }, start.y);
                model.blocks.push(BlockRow {
                    id,
                    length: DEFAULT_BLOCK_LENGTH,
                });
                model.geometry.push(GeomRow {
                    id,
                    points: vec![start, end],
                });
                normalize(&mut model, &[]);
                model.dirty = true;
                selection.mode = EditMode::Idle;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rail_dispatch::level::Level;

    /// Two horizontal blocks on row 0: block 1 from x=0..40, block 2 from x=40..80, connected.
    fn two_block_model() -> EditorModel {
        let level: Level = toml::from_str(
            r##"
            blocks = [[1, 100], [2, 100]]
            connections = [[1, 2]]
            switches = []
            spawners = []
            signals = []
            geometry = [[1, [[0, 0], [40, 0]]], [2, [[40, 0], [80, 0]]]]
            background = "#000000"
            "##,
        )
        .expect("test level parses");
        EditorModel::from_level(&level)
    }

    #[test]
    fn prune_keeps_connections_that_still_touch() {
        let mut model = two_block_model();
        prune_connections(&mut model);
        assert_eq!(model.connections, vec![(1, 2)]);
    }

    #[test]
    fn prune_drops_connection_after_block_moves_away() {
        let mut model = two_block_model();
        for p in &mut model.geometry[1].points {
            p.y += 24.0;
        }
        prune_connections(&mut model);
        assert_eq!(model.connections, Vec::<(BlockId, BlockId)>::new());
    }

    #[test]
    fn snap_connect_joins_nearby_endpoints_left_to_right() {
        let mut model = two_block_model();
        model.connections.clear();
        // Pull block 2's start slightly off the junction, then release it there.
        model.geometry[1].points[0] = Vec2::new(43.0, 0.0);
        snap_connect(
            &mut model,
            VertexRef {
                block_idx: 1,
                point_idx: 0,
            },
        );
        assert_eq!(model.geometry[1].points[0], Vec2::new(40.0, 0.0));
        assert_eq!(model.connections, vec![(1, 2)]);
    }
}
