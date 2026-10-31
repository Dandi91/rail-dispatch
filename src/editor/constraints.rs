//! Geometry invariants for the rows model.
//!
//! Every vertex lies on a horizontal y-row. A segment is **horizontal** when its endpoints share
//! a row and **diagonal** otherwise; diagonals are locked to a single fixed angle. Open block
//! ends keep a horizontal stub long enough to seat a describer label.
//!
//! [`normalize`] is the single entry point, run during and after every edit. The vertices the
//! user is manipulating (the **anchors**, e.g. the dragged vertex and its coincident junction
//! partners) are authoritative — they stay exactly where the cursor put them. The angle lock is
//! propagated *outward* from each anchor: only the neighbouring vertices move to keep every
//! diagonal's run at the fixed slope, so a dragged vertex never snaps back. Open terminal stubs
//! are then repaired to the minimum length, skipping any end the user is currently dragging.

use bevy::prelude::*;

use crate::state::{COINCIDENT_EPS, EditorModel, VertexRef};

/// The single permitted magnitude for diagonal ("up/down") legs.
pub const DIAGONAL_ANGLE_DEG: f32 = 72.0;

/// Minimum length (pixel space) of a horizontal terminal stub at an open end, sized so a train
/// describer label seats comfortably. Derived conservatively from the describer geometry in
/// `panel.rs` (`DESCRIBER_INSET` + padding + a few mono glyphs), divided by `SCALE`.
pub const MIN_END_STUB_PX: f32 = 16.0;

/// Minimum length (pixel space) of any segment, i.e. the closest two adjacent vertices of a block
/// may get. Prevents welding a block's vertices together. Below the smallest segment in the
/// shipped level so it never disturbs existing geometry.
pub const MIN_SEGMENT_PX: f32 = 8.0;

/// Horizontal run of a diagonal for a given vertical span, at the locked angle.
fn diagonal_run(dy: f32) -> f32 {
    dy.abs() / DIAGONAL_ANGLE_DEG.to_radians().tan()
}

/// Enforce all invariants, keeping `anchors` (the vertices the user is manipulating) fixed.
/// Cheap enough to run every drag frame. Pass an empty slice to anchor each polyline at point 0.
pub fn normalize(model: &mut EditorModel, anchors: &[VertexRef]) {
    for (block_idx, g) in model.geometry.iter_mut().enumerate() {
        if g.points.is_empty() {
            continue;
        }
        // Anchor this polyline at the lowest dragged index it contains (else point 0).
        let anchor = anchors
            .iter()
            .filter(|a| a.block_idx == block_idx)
            .map(|a| a.point_idx)
            .min()
            .unwrap_or(0)
            .min(g.points.len() - 1);
        propagate_from(&mut g.points, anchor);
    }
    enforce_open_stubs(model, anchors);
}

/// Lock every diagonal's horizontal run to the fixed slope, holding `anchor` fixed and moving
/// only the vertices on either side of it. Horizontal runs keep their x.
fn propagate_from(points: &mut [Vec2], anchor: usize) {
    for i in (anchor + 1)..points.len() {
        let prev = points[i - 1];
        let dy = points[i].y - prev.y;
        if dy.abs() > COINCIDENT_EPS {
            let sign = if (points[i].x - prev.x) < 0.0 { -1.0 } else { 1.0 };
            points[i].x = prev.x + sign * diagonal_run(dy);
        }
    }
    for i in (0..anchor).rev() {
        let next = points[i + 1];
        let dy = points[i].y - next.y;
        if dy.abs() > COINCIDENT_EPS {
            let sign = if (points[i].x - next.x) < 0.0 { -1.0 } else { 1.0 };
            points[i].x = next.x + sign * diagonal_run(dy);
        }
    }
}

/// A terminal vertex is **open** when no other polyline has a coincident vertex (junctions —
/// connections and switch legs — always share a point with another block).
fn is_open_end(model: &EditorModel, block_idx: usize, point: Vec2) -> bool {
    for (other_idx, g) in model.geometry.iter().enumerate() {
        if other_idx == block_idx {
            continue;
        }
        if g.points.iter().any(|p| p.distance(point) < COINCIDENT_EPS) {
            return false;
        }
    }
    true
}

/// Ensure each open block end terminates in a horizontal stub of at least [`MIN_END_STUB_PX`].
/// The terminal vertex is dropped onto the interior vertex's row (keeping the stub horizontal)
/// and extended outward to the minimum length. Ends the user is currently dragging (anchored)
/// are left untouched so they follow the cursor freely.
fn enforce_open_stubs(model: &mut EditorModel, anchors: &[VertexRef]) {
    let count = model.geometry.len();
    for idx in 0..count {
        let n = model.geometry[idx].points.len();
        if n < 2 {
            continue;
        }
        // odd end (first point), interior neighbour is points[1]
        let first = model.geometry[idx].points[0];
        if !is_anchored(anchors, idx, 0) && is_open_end(model, idx, first) {
            let inner = model.geometry[idx].points[1];
            let dir = if first.x <= inner.x { -1.0 } else { 1.0 }; // outward along x
            let target_x = inner.x + dir * MIN_END_STUB_PX;
            let pts = &mut model.geometry[idx].points;
            pts[0] = Vec2::new(stub_x(first.x, target_x, dir), inner.y);
        }
        // even end (last point), interior neighbour is points[n-2]
        let last = model.geometry[idx].points[n - 1];
        if !is_anchored(anchors, idx, n - 1) && is_open_end(model, idx, last) {
            let inner = model.geometry[idx].points[n - 2];
            let dir = if last.x >= inner.x { 1.0 } else { -1.0 };
            let target_x = inner.x + dir * MIN_END_STUB_PX;
            let pts = &mut model.geometry[idx].points;
            pts[n - 1] = Vec2::new(stub_x(last.x, target_x, dir), inner.y);
        }
    }
}

fn is_anchored(anchors: &[VertexRef], block_idx: usize, point_idx: usize) -> bool {
    anchors
        .iter()
        .any(|a| a.block_idx == block_idx && a.point_idx == point_idx)
}

/// Keep the user's stub length if it already meets the minimum (`target` is the minimum-length
/// position); otherwise extend out to `target`. `dir` is the outward sign along x.
fn stub_x(current_x: f32, target_x: f32, dir: f32) -> f32 {
    if dir > 0.0 {
        current_x.max(target_x)
    } else {
        current_x.min(target_x)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn angle_deg(a: Vec2, b: Vec2) -> f32 {
        ((b.y - a.y) / (b.x - a.x)).abs().atan().to_degrees()
    }

    #[test]
    fn propagate_locks_diagonals_around_the_anchor() {
        // Diagonal before the anchor, horizontal run, then a diagonal after it.
        let mut points = [
            Vec2::new(0.0, 0.0),
            Vec2::new(20.0, 12.0),
            Vec2::new(60.0, 12.0),
            Vec2::new(65.0, 24.0),
        ];
        propagate_from(&mut points, 1);
        assert_eq!(points[1], Vec2::new(20.0, 12.0), "anchor stays put");
        assert!((angle_deg(points[0], points[1]) - DIAGONAL_ANGLE_DEG).abs() < 0.01);
        assert_eq!(points[2], Vec2::new(60.0, 12.0), "horizontal runs keep their x");
        assert!((angle_deg(points[2], points[3]) - DIAGONAL_ANGLE_DEG).abs() < 0.01);
    }

    #[test]
    fn stub_extends_only_when_too_short() {
        assert_eq!(stub_x(100.0, 50.0, 1.0), 100.0);
        assert_eq!(stub_x(40.0, 50.0, 1.0), 50.0);
        assert_eq!(stub_x(-100.0, -50.0, -1.0), -100.0);
    }
}
