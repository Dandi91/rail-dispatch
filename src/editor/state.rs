//! The editable in-memory model and selection state.
//!
//! [`EditorModel`] is the single source of truth and lives in **pixel space** (the `level.toml`
//! save format). Everything rendered on screen is a pure projection of it via [`TrackTransform`].

use bevy::prelude::*;
use rail_dispatch::assets::AssetHandles;
use rail_dispatch::common::BlockId;
use rail_dispatch::level::Level;
use rail_dispatch::panel::TrackTransform;

/// Physical length (metres) given to a freshly created block. Independent of its drawn shape;
/// edit by hand in `level.toml` afterwards if a different length is needed.
pub const DEFAULT_BLOCK_LENGTH: f64 = 100.0;

/// Two points within this pixel distance are treated as coincident (a shared junction) and as
/// lying on the same horizontal row.
pub const COINCIDENT_EPS: f32 = 0.5;

/// A block's physical row: `{ id, length }`. The drawn geometry lives in [`GeomRow`].
#[derive(Clone, Copy)]
pub struct BlockRow {
    pub id: BlockId,
    pub length: f64,
}

/// A block's drawn polyline, in pixel space.
pub struct GeomRow {
    pub id: BlockId,
    pub points: Vec<Vec2>,
}

/// The editable level. Pixel space throughout. Switches/signals/sections/etc. are left untouched
/// in `level.toml` by the save step, so they are not mirrored here (switch editing is a deferred
/// follow-up; junctions are detected geometrically).
#[derive(Resource)]
pub struct EditorModel {
    pub blocks: Vec<BlockRow>,
    pub connections: Vec<(BlockId, BlockId)>,
    pub geometry: Vec<GeomRow>,
    /// Vertical spacing of the row grid (pixel space). Vertices snap to multiples of this.
    pub row_spacing: f32,
    /// Pixel↔world mapping, captured once at init and held fixed for the session.
    pub transform: TrackTransform,
    /// Set whenever the model changes so the renderer redraws.
    pub dirty: bool,
}

impl EditorModel {
    fn from_level(level: &Level) -> Self {
        let blocks = level
            .blocks
            .iter()
            .map(|b| BlockRow {
                id: b.id,
                length: b.length,
            })
            .collect();
        let connections = level.connections.iter().map(|c| (c.start, c.end)).collect();
        let geometry = level
            .geometry
            .iter()
            .map(|g| GeomRow {
                id: g.id,
                points: g.points.clone(),
            })
            .collect::<Vec<_>>();

        Self {
            blocks,
            connections,
            geometry,
            row_spacing: level.row_spacing.max(1.0),
            transform: TrackTransform::from_level(level),
            dirty: true,
        }
    }

    /// The next free block id (max existing + 1, or 1 if empty).
    pub fn next_block_id(&self) -> BlockId {
        self.blocks.iter().map(|b| b.id).max().map_or(1, |m| m + 1)
    }

    /// Snap a y value to the nearest row on the grid (a multiple of [`Self::row_spacing`]).
    pub fn snap_row(&self, y: f32) -> f32 {
        (y / self.row_spacing).round() * self.row_spacing
    }
}

/// State for translating a whole block: which block, the cursor position at press, and the
/// block's vertices at press — so each frame's position is computed as `start + snapped_delta`.
pub struct BlockDrag {
    pub block_idx: usize,
    pub start_cursor: Vec2,
    pub start_points: Vec<Vec2>,
}

/// Reference to one vertex of one polyline.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct VertexRef {
    pub block_idx: usize,
    pub point_idx: usize,
}

/// Current editor interaction mode.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum EditMode {
    #[default]
    Idle,
    /// Placing a new block: `true` once the first endpoint has been clicked.
    PlacingBlock,
}

#[derive(Resource, Default)]
pub struct EditorSelection {
    pub hovered: Option<VertexRef>,
    pub selected: Option<VertexRef>,
    /// Where a vertex would be inserted (pixel space) if `Ctrl`+clicked now — drives the ghost
    /// marker. `Some` only while `Ctrl` is held near a segment and not over a vertex.
    pub insert_preview: Option<Vec2>,
    pub dragging: bool,
    /// Vertices that move together with the dragged one (coincident at drag start), as
    /// `(block_idx, point_idx)`.
    pub drag_group: Vec<VertexRef>,
    /// In-progress whole-block translation (left-drag on a segment, no `Ctrl`).
    pub block_drag: Option<BlockDrag>,
    pub mode: EditMode,
    /// First clicked endpoint (pixel space) while placing a block.
    pub pending_start: Option<Vec2>,
}

/// Build the [`EditorModel`] from the loaded [`Level`]. Runs once on `OnExit(Loading)`.
pub fn init_model(handles: Res<AssetHandles>, levels: Res<Assets<Level>>, mut commands: Commands) {
    let level = levels.get(&handles.level).expect("level had been loaded");
    commands.insert_resource(EditorModel::from_level(level));
    commands.init_resource::<EditorSelection>();
}
