use crate::assets::{AssetHandles, LoadingState};
use crate::common::{BlockId, Direction, SectionId, SignalId, SwitchPosition, TrainId};
use crate::level::{BlockData, Level, SectionData};
use crate::simulation::signal::{SignalAspect, SignalMap, TrackSignal};
use crate::simulation::sparse_vec::{Chunkable, SparseVec};
use crate::simulation::station::{Switch, SwitchUpdate};
use crate::simulation::train::{TrainMove, TrainMoveKind};
use arrayvec::ArrayVec;
use bevy::prelude::*;
use itertools::Itertools;
use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt::Formatter;
use std::ops::Not;

#[derive(Default, Copy, Clone, PartialEq)]
pub enum TrackState {
    #[default]
    Freed,
    Occupied,
}

impl Not for TrackState {
    type Output = Self;
    fn not(self) -> Self::Output {
        match self {
            TrackState::Occupied => TrackState::Freed,
            TrackState::Freed => TrackState::Occupied,
        }
    }
}

impl From<TrainMoveKind> for TrackState {
    fn from(value: TrainMoveKind) -> Self {
        match value {
            TrainMoveKind::Entered => TrackState::Occupied,
            TrainMoveKind::Exited => TrackState::Freed,
        }
    }
}

/// Messages for track occupation update.
/// For single-block sections (not defined in `level.toml`), updates are produced for each block.
/// For multi-block sections, `TrackState::Freed` updates are produced only when all blocks in the section are cleared.
#[derive(Message, Default)]
pub struct TrackUpdate {
    pub block_id: BlockId,
    pub train_id: TrainId,
    pub state: TrackState,
    pub train_direction: Direction,
    pub train_number: String,
    pub section_ctx: Option<SectionContext>,
}

pub struct SectionContext {
    pub section_id: SectionId,
    pub blocks: Vec<BlockId>,
}

impl From<&Section> for SectionContext {
    fn from(section: &Section) -> Self {
        SectionContext {
            section_id: section.id,
            blocks: section.blocks.clone(),
        }
    }
}

impl SectionContext {
    pub fn get_block_index(&self, block_id: &BlockId) -> Option<usize> {
        self.blocks.iter().position(|id| id == block_id)
    }

    pub fn get_blocks_len(&self) -> usize {
        self.blocks.len()
    }
}

impl TrackUpdate {
    fn from_train_move(train_move: &TrainMove, section: Option<&Section>) -> Self {
        TrackUpdate {
            block_id: train_move.block_id,
            train_id: train_move.train_id,
            state: train_move.kind.into(),
            train_direction: train_move.direction,
            train_number: train_move.number.clone(),
            section_ctx: section.map(Into::into),
        }
    }

    /// Anonymous update (not coming from a train) that is used to reset internal game state at the startup
    fn block_reset(block_id: BlockId) -> Self {
        TrackUpdate {
            block_id,
            ..Default::default()
        }
    }

    /// Returns a slice of all updated blocks.
    /// In case of a section update, all section blocks are updated at the same time
    pub fn blocks(&self) -> &[BlockId] {
        match &self.section_ctx {
            Some(section_ctx) => section_ctx.blocks.as_slice(),
            None => std::slice::from_ref(&self.block_id),
        }
    }
}

#[derive(Copy, Clone)]
pub enum SignalUpdateSource {
    /// Update caused by the change of the guarded track state
    BlockChange(TrackState),
    /// Update caused by the change of the next signal state
    SignalPropagation(SignalAspect),
    /// Manual override, e.g. from route activation
    Manual(SignalAspect),
}

/// A request to update the signal. Contains a target signal ID and a source of the request.
#[derive(Message, Clone)]
pub struct SignalUpdate {
    pub signal_id: SignalId,
    pub source: SignalUpdateSource,
}

impl SignalUpdate {
    pub fn new(signal_id: SignalId, source: SignalUpdateSource) -> Self {
        Self { signal_id, source }
    }

    pub fn from_track_change(signal_id: SignalId, state: TrackState) -> Self {
        Self::new(signal_id, SignalUpdateSource::BlockChange(state))
    }
}

/// Notifies consumers (the panel) that a signal's resolved aspect actually changed.
/// Unlike [`SignalUpdate`] (a request), this carries the settled aspect after propagation.
#[derive(Message)]
pub struct SignalAspectChanged {
    pub signal_id: SignalId,
    pub aspect: SignalAspect,
}

#[derive(Default)]
struct BlockTracker {
    blocks: HashMap<BlockId, Vec<TrainId>>,
    trains: HashMap<TrainId, HashSet<BlockId>>,
    sections: HashMap<SectionId, usize>,
}

impl BlockTracker {
    fn is_block_free(&self, block_id: BlockId) -> bool {
        self.blocks.get(&block_id).is_none_or(|v| v.is_empty())
    }

    fn handle_update(&mut self, update: &TrackUpdate) -> bool {
        match update.state {
            TrackState::Freed => self.set_freed(update),
            TrackState::Occupied => self.set_occupied(update),
        }
    }

    /// Records block as occupied by the train id, returns true if the block and/or section was previously free
    fn set_occupied(&mut self, update: &TrackUpdate) -> bool {
        // we rarely need to track more than 1 train per block (more in case of shunting),
        // but since TrainId is u32, we can afford to preallocate 2 of them just in case.
        const OCCUPIED_CAPACITY: usize = 2;
        let entry = self
            .blocks
            .entry(update.block_id)
            .or_insert_with(|| Vec::with_capacity(OCCUPIED_CAPACITY));
        entry.push(update.train_id);
        let mut result = entry.len() == 1;

        // a single train can span multiple blocks, especially at stations.
        // again, considering that BlockId is u32, we can afford to preallocate 8 of them.
        const TRAIN_BLOCKS_CAPACITY: usize = 8;
        self.trains
            .entry(update.train_id)
            .or_insert_with(|| HashSet::with_capacity(TRAIN_BLOCKS_CAPACITY))
            .insert(update.block_id);

        if let Some(section_ctx) = &update.section_ctx {
            let e = self.sections.entry(section_ctx.section_id).or_default();
            result = *e == 0;
            *e += 1;
        }

        result
    }

    /// Records block as freed by the train id, returns true if the block and/or section is now free
    fn set_freed(&mut self, update: &TrackUpdate) -> bool {
        if let Some(v) = self.trains.get_mut(&update.train_id) {
            v.remove(&update.block_id);
            if v.is_empty() {
                self.trains.remove(&update.train_id);
            }
        }

        let mut result = self.blocks.get_mut(&update.block_id).is_none_or(|v| {
            v.retain(|&x| x != update.train_id);
            v.is_empty()
        });

        if let Some(section_ctx) = &update.section_ctx {
            let e = self
                .sections
                .entry(section_ctx.section_id)
                .and_modify(|e| *e -= 1)
                .or_default();
            result = *e == 0;
        }

        result
    }
}

#[derive(Default, Resource)]
pub struct BlockMap {
    blocks: SparseVec<Block>,
    tracker: BlockTracker,
    signals: SignalMap,
    switches: SparseVec<Switch>,
    sections: SparseVec<Section>,
    sectioned_blocks: HashMap<BlockId, SectionId>,
}

impl BlockMap {
    pub fn get_available_length(&self, point: &TrackPoint, direction: Direction) -> f64 {
        if direction == Direction::Even {
            self.blocks[point.block_id].length_m - point.offset_m
        } else {
            point.offset_m
        }
    }

    pub fn get_next(&self, block_id: BlockId, direction: Direction) -> Option<&Block> {
        let block = &self.blocks[block_id];
        let next = match direction {
            Direction::Even => block.next?,
            Direction::Odd => block.prev?,
        };
        Some(&self.blocks[next])
    }

    pub fn get_block(&self, block_id: BlockId) -> Option<&Block> {
        self.blocks.get(block_id)
    }

    pub fn get_train_blocks(&self, train_id: TrainId) -> Option<&HashSet<BlockId>> {
        self.tracker.trains.get(&train_id)
    }

    fn get_section_by_block(&self, block_id: BlockId) -> Option<&Section> {
        let section_id = self.sectioned_blocks.get(&block_id)?;
        self.sections.get(*section_id)
    }

    fn process_switch_updates(&mut self, switch_updates: &mut MessageReader<SwitchUpdate>) {
        for update in switch_updates.read() {
            let switch = &mut self.switches[update.switch_id];
            switch.position = update.position;
            let (base, straight, side, direction) = (switch.base, switch.straight, switch.side, switch.direction);
            let (active_leg, inactive_leg) = if update.position == SwitchPosition::Straight {
                (straight, side)
            } else {
                (side, straight)
            };
            match direction {
                Direction::Even => {
                    self.blocks[base].next = Some(active_leg);
                    self.blocks[active_leg].prev = Some(base);
                    self.blocks[inactive_leg].prev = None;
                }
                Direction::Odd => {
                    self.blocks[base].prev = Some(active_leg);
                    self.blocks[active_leg].next = Some(base);
                    self.blocks[inactive_leg].next = None;
                }
            };
        }
    }

    fn process_train_moves(
        &mut self,
        train_moves: &mut MessageReader<TrainMove>,
        track_updates: &mut MessageWriter<TrackUpdate>,
    ) {
        for mv in train_moves.read() {
            let section = self.get_section_by_block(mv.block_id);
            let track_update = TrackUpdate::from_train_move(mv, section);
            let changed = self.tracker.handle_update(&track_update);
            if changed {
                track_updates.write(track_update);
            }
        }
    }

    fn process_block_updates(
        &self,
        track_updates: &mut MessageReader<TrackUpdate>,
        signal_updates: &mut MessageWriter<SignalUpdate>,
    ) {
        for update in track_updates.read() {
            let block = &self.blocks[update.block_id];
            signal_updates.write_batch(
                self.find_affected_signals(block, update.state)
                    .iter()
                    .map(|signal| SignalUpdate::from_track_change(signal.id, update.state)),
            );
        }
    }

    fn process_signal_updates(
        &mut self,
        signal_updates: &mut MessageReader<SignalUpdate>,
        aspect_changes: &mut MessageWriter<SignalAspectChanged>,
    ) {
        let mut queue = VecDeque::from_iter(signal_updates.read().cloned());
        while let Some(update) = queue.pop_front() {
            let signal = &self.signals[update.signal_id];
            let is_closed_manual = signal.is_closed_manual();
            let aspect = match update.source {
                SignalUpdateSource::BlockChange(block_update) => match block_update {
                    TrackState::Occupied => SignalAspect::Forbidding,
                    TrackState::Freed if is_closed_manual => SignalAspect::Forbidding,
                    TrackState::Freed => {
                        if let Some((next, _)) = self.lookup_signal_forward(&signal.position, signal.direction) {
                            next.speed_ctrl.aspect.chain()
                        } else {
                            SignalAspect::Forbidding
                        }
                    }
                },
                SignalUpdateSource::SignalPropagation(_) if is_closed_manual => SignalAspect::Forbidding,
                SignalUpdateSource::SignalPropagation(next_signal_aspect) => {
                    if self.is_signal_free(signal) {
                        next_signal_aspect.chain()
                    } else {
                        SignalAspect::Forbidding
                    }
                }
                SignalUpdateSource::Manual(aspect) => aspect,
            };

            if aspect != signal.speed_ctrl.aspect {
                let prev = self.lookup_signal(&signal.position, signal.direction.reverse(), signal.direction);
                if let Some((prev, _)) = prev {
                    queue.push_back(SignalUpdate::new(
                        prev.id,
                        SignalUpdateSource::SignalPropagation(aspect),
                    ));
                }

                self.signals[update.signal_id].change_aspect(aspect);
                aspect_changes.write(SignalAspectChanged {
                    signal_id: update.signal_id,
                    aspect,
                });
            }
        }
    }

    /// Trains currently occupying the block, if any (used by the panel's hover tooltip).
    pub fn block_trains(&self, block_id: BlockId) -> Option<&Vec<TrainId>> {
        self.tracker.blocks.get(&block_id).filter(|v| !v.is_empty())
    }

    /// Looks up a signal by id (used by the panel's hover tooltip).
    pub fn signal(&self, signal_id: SignalId) -> Option<&TrackSignal> {
        self.signals.get(signal_id)
    }

    fn init(&self, track_updates: &mut MessageWriter<TrackUpdate>, switch_updates: &mut MessageWriter<SwitchUpdate>) {
        switch_updates.write_batch(
            self.switches
                .iter()
                .map(|switch| SwitchUpdate::new(switch.id, switch.position)),
        );
        track_updates.write_batch(self.blocks.iter().map(|block| TrackUpdate::block_reset(block.id)));
    }

    /// Given a track state update, returns a collection of all signals that it affects
    /// (at most 2 signals per block, one in each direction).
    fn find_affected_signals(&self, block: &Block, state: TrackState) -> ArrayVec<&TrackSignal, 2> {
        let point = block.middle();
        [Direction::Even, Direction::Odd]
            .iter()
            .filter_map(|&direction| {
                self.walk(&point, f64::INFINITY, direction)
                    .skip(1)
                    .find_map(|p| self.signals.find_signal(p.block_id, direction.reverse()))
            })
            .filter(|signal| matches!(state, TrackState::Occupied) || self.is_signal_free(signal))
            .collect()
    }

    /// Checks if the blocks after the `signal` are free up until the next signal in the same direction
    fn is_signal_free(&self, signal: &TrackSignal) -> bool {
        self.walk(&signal.position, f64::INFINITY, signal.direction)
            .skip(1)
            .take_while_inclusive(|p| self.signals.find_signal(p.block_id, signal.direction).is_none())
            .all(|p| self.tracker.is_block_free(p.block_id))
    }

    /// Step `length_m` meters in the `direction` along the track
    pub fn step_by(&self, start: &TrackPoint, length_m: f64, direction: Direction) -> TrackPoint {
        self.walk(start, length_m, direction)
            .last()
            .expect("expected non-zero length")
    }

    /// Tries to find a forward facing signal placed in the `direction` along the track,
    /// returning tuple of signal and distance to it
    pub fn lookup_signal_forward(&self, start: &TrackPoint, direction: Direction) -> Option<(&TrackSignal, f64)> {
        self.lookup_signal(start, direction, direction)
    }

    /// Tries to find a signal placed in the `direction` along the track with a given `signal_direction`,
    /// returning tuple of signal and distance to it
    fn lookup_signal(
        &self,
        start: &TrackPoint,
        direction: Direction,
        signal_direction: Direction,
    ) -> Option<(&TrackSignal, f64)> {
        let reversed = direction.reverse();
        let mut length = -self.get_available_length(start, reversed);
        for (idx, point) in self.walk(start, f64::INFINITY, direction).enumerate() {
            if let Some(signal) = self.signals.find_signal(point.block_id, signal_direction) {
                let diff = direction.apply_sign(signal.position.offset_m - start.offset_m);
                if idx > 0 || diff > 0.0 {
                    length += self.get_available_length(&signal.position, reversed);
                    return Some((signal, length));
                }
            }
            length += self.get_available_length(&point, reversed);
        }
        None
    }

    pub fn find_signal(&self, block_id: BlockId, direction: Direction) -> Option<&TrackSignal> {
        self.signals.find_signal(block_id, direction)
    }

    pub fn walk(&self, start: &TrackPoint, length_m: f64, direction: Direction) -> TrackWalker<'_> {
        TrackWalker {
            block_map: self,
            current_block_id: start.block_id,
            offset_m: start.offset_m,
            block_available_m: self.get_available_length(start, direction),
            current_block_length_m: self.blocks[start.block_id].length_m,
            length_m,
            direction,
        }
    }

    pub fn from_level(level: &Level) -> Self {
        let mut blocks: SparseVec<Block> = level.blocks.iter().map_into().collect();
        let signals: SignalMap = level.signals.iter().map_into().collect();
        let switches: SparseVec<Switch> = level.switches.iter().map_into().collect();
        let sections: SparseVec<Section> = level.sections.iter().map_into().collect();

        for conn in &level.connections {
            blocks[conn.start].next = Some(conn.end);
            blocks[conn.end].prev = Some(conn.start);
        }

        let sectioned_blocks: HashMap<BlockId, SectionId> = level
            .sections
            .iter()
            .flat_map(|sd| sd.blocks.iter().copied().map(|block_id| (block_id, sd.id)))
            .collect();

        BlockMap {
            blocks,
            signals,
            switches,
            sections,
            sectioned_blocks,
            ..Default::default()
        }
    }
}

#[derive(Default)]
pub struct Block {
    pub id: BlockId,
    pub length_m: f64,
    prev: Option<BlockId>,
    next: Option<BlockId>,
}

impl From<&BlockData> for Block {
    fn from(value: &BlockData) -> Self {
        Block {
            id: value.id,
            length_m: value.length,
            ..Default::default()
        }
    }
}

impl Chunkable for Block {
    fn get_id(&self) -> u32 {
        self.id
    }
}

impl Block {
    pub fn middle(&self) -> TrackPoint {
        TrackPoint {
            block_id: self.id,
            offset_m: self.length_m / 2.0,
        }
    }

    /// Returns a direction in which the block has an open end, or `None` if both ends are connected.
    pub fn get_end_direction(&self) -> Option<Direction> {
        if self.prev.is_none() {
            Some(Direction::Odd)
        } else if self.next.is_none() {
            Some(Direction::Even)
        } else {
            None
        }
    }
}

#[derive(Default, Debug, Clone)]
pub struct TrackPoint {
    pub block_id: BlockId,
    pub offset_m: f64,
}

impl TrackPoint {
    pub fn new(block_id: BlockId, offset_m: f64) -> Self {
        Self { block_id, offset_m }
    }
}

impl std::fmt::Display for TrackPoint {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "block {} at {:.0} m", self.block_id, self.offset_m)
    }
}

#[derive(Default)]
struct Section {
    id: SectionId,
    blocks: Vec<BlockId>,
}

impl Chunkable for Section {
    fn get_id(&self) -> u32 {
        self.id
    }
}

impl From<&SectionData> for Section {
    fn from(value: &SectionData) -> Self {
        Section {
            id: value.id,
            blocks: value.blocks.clone(),
        }
    }
}

pub struct TrackWalker<'a> {
    block_map: &'a BlockMap,
    current_block_id: BlockId,
    length_m: f64,
    offset_m: f64,
    direction: Direction,
    block_available_m: f64,
    current_block_length_m: f64,
}

impl Iterator for TrackWalker<'_> {
    type Item = TrackPoint;

    fn next(&mut self) -> Option<Self::Item> {
        if self.length_m <= 0.0 || self.offset_m.is_nan() {
            return None;
        }

        if self.length_m < self.block_available_m {
            let new_offset = self.offset_m + self.direction.apply_sign(self.length_m);
            self.length_m = 0.0;
            Some(TrackPoint::new(self.current_block_id, new_offset))
        } else {
            self.length_m -= self.block_available_m;
            let result_block_id = self.current_block_id;
            let result_block_length = self.current_block_length_m;
            if let Some(next_block) = self.block_map.get_next(self.current_block_id, self.direction) {
                self.current_block_id = next_block.id;
                self.block_available_m = next_block.length_m;
                self.current_block_length_m = next_block.length_m;
                self.offset_m = match self.direction {
                    Direction::Even => 0.0,
                    Direction::Odd => next_block.length_m,
                };
            } else {
                self.offset_m = f64::NAN;
            }
            Some(TrackPoint::new(
                result_block_id,
                match self.direction {
                    Direction::Even => result_block_length,
                    Direction::Odd => 0.0,
                },
            ))
        }
    }
}

pub struct MapPlugin;

impl Plugin for MapPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<TrackUpdate>()
            .add_message::<SignalUpdate>()
            .add_message::<SignalAspectChanged>()
            .add_systems(OnExit(LoadingState::Loading), (setup, init).chain())
            .add_systems(
                Update,
                (switch_updates, train_moves, track_updates, signal_updates)
                    .chain()
                    .run_if(in_state(LoadingState::Instantiated)),
            );
    }
}

fn setup(handles: Res<AssetHandles>, levels: Res<Assets<Level>>, mut commands: Commands) {
    let level = levels.get(&handles.level).expect("level had been loaded");
    commands.insert_resource(BlockMap::from_level(level));
}

fn init(
    block_map: Res<BlockMap>,
    mut track_updates: MessageWriter<TrackUpdate>,
    mut switch_updates: MessageWriter<SwitchUpdate>,
    mut next_loading_state: ResMut<NextState<LoadingState>>,
) {
    block_map.init(&mut track_updates, &mut switch_updates);
    next_loading_state.set(LoadingState::Instantiated);
}

fn train_moves(
    mut block_map: ResMut<BlockMap>,
    mut train_moves: MessageReader<TrainMove>,
    mut track_updates: MessageWriter<TrackUpdate>,
) {
    block_map.process_train_moves(&mut train_moves, &mut track_updates);
}

fn track_updates(
    block_map: Res<BlockMap>,
    mut track_updates: MessageReader<TrackUpdate>,
    mut signal_updates: MessageWriter<SignalUpdate>,
) {
    block_map.process_block_updates(&mut track_updates, &mut signal_updates);
}

fn signal_updates(
    mut block_map: ResMut<BlockMap>,
    mut signal_updates: MessageReader<SignalUpdate>,
    mut aspect_changes: MessageWriter<SignalAspectChanged>,
) {
    block_map.process_signal_updates(&mut signal_updates, &mut aspect_changes);
}

fn switch_updates(mut block_map: ResMut<BlockMap>, mut switch_updates: MessageReader<SwitchUpdate>) {
    block_map.process_switch_updates(&mut switch_updates);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::wrap;

    fn build_track() -> BlockMap {
        let blocks = [
            Block {
                id: 1,
                length_m: 1000.0,
                next: Some(2),
                ..Default::default()
            },
            Block {
                id: 2,
                length_m: 500.0,
                prev: Some(1),
                next: Some(3),
                ..Default::default()
            },
            Block {
                id: 3,
                length_m: 1500.0,
                prev: Some(2),
                ..Default::default()
            },
        ];
        let signals = [
            TrackSignal {
                id: 1,
                position: TrackPoint::new(3, 1400.0),
                direction: Direction::Even,
                ..Default::default()
            },
            TrackSignal {
                id: 2,
                position: TrackPoint::new(1, 250.0),
                direction: Direction::Odd,
                ..Default::default()
            },
        ];
        BlockMap {
            blocks: blocks.into_iter().collect(),
            signals: signals.into_iter().collect(),
            ..Default::default()
        }
    }

    fn build_track_extended() -> BlockMap {
        let blocks = (1..=4).map(|idx| Block {
            id: idx,
            length_m: 500.0,
            next: Some(wrap(idx + 1, 1, 4)),
            prev: Some(wrap(idx - 1, 1, 4)),
            ..Default::default()
        });
        let signals = (1..=4).map(|idx| {
            [
                TrackSignal {
                    id: idx * 2 - 1,
                    position: TrackPoint::new(idx, 490.0),
                    direction: Direction::Even,
                    ..Default::default()
                },
                TrackSignal {
                    id: idx * 2,
                    position: TrackPoint::new(idx, 10.0),
                    direction: Direction::Odd,
                    ..Default::default()
                },
            ]
        });
        BlockMap {
            blocks: blocks.into_iter().collect(),
            signals: signals.flatten().into_iter().collect(),
            ..Default::default()
        }
    }

    #[test]
    fn walk_same_block_even() {
        let map = build_track();
        let point = TrackPoint::new(1, 250.0);
        let visited: Vec<_> = map.walk(&point, 450.0, Direction::Even).collect();
        assert_eq!(visited.len(), 1);
        assert_eq!(visited[0].block_id, 1);
        assert_eq!(visited[0].offset_m, 700.0);
    }

    #[test]
    fn walk_same_block_odd() {
        let map = build_track();
        let point = TrackPoint::new(1, 750.0);
        let visited: Vec<_> = map.walk(&point, 650.0, Direction::Odd).collect();
        assert_eq!(visited.len(), 1);
        assert_eq!(visited[0].block_id, 1);
        assert_eq!(visited[0].offset_m, 100.0);
    }

    #[test]
    fn walk_track_even() {
        let map = build_track();
        let point = TrackPoint::new(1, 250.0);
        let visited: Vec<_> = map.walk(&point, 2500.0, Direction::Even).collect();
        assert_eq!(visited.len(), 3);
        assert_eq!(visited[0].block_id, 1);
        assert_eq!(visited[1].block_id, 2);
        assert_eq!(visited[2].block_id, 3);
        assert_eq!(visited[0].offset_m, 1000.0);
        assert_eq!(visited[1].offset_m, 500.0);
        assert_eq!(visited[2].offset_m, 1250.0);
    }

    #[test]
    fn walk_track_odd() {
        let map = build_track();
        let point = TrackPoint::new(3, 1050.0);
        let visited: Vec<_> = map.walk(&point, 2500.0, Direction::Odd).collect();
        assert_eq!(visited.len(), 3);
        assert_eq!(visited[0].block_id, 3);
        assert_eq!(visited[1].block_id, 2);
        assert_eq!(visited[2].block_id, 1);
        assert_eq!(visited[0].offset_m, 0.0);
        assert_eq!(visited[1].offset_m, 0.0);
        assert_eq!(visited[2].offset_m, 50.0);
    }

    #[test]
    fn walk_track_over_end_even() {
        let map = build_track();
        let point = TrackPoint::new(3, 850.0);
        let visited: Vec<_> = map.walk(&point, 2500.0, Direction::Even).collect_vec();
        assert_eq!(visited.len(), 1);
        assert_eq!(visited[0].block_id, 3);
        assert_eq!(visited[0].offset_m, 1500.0);
    }

    #[test]
    fn walk_track_over_end_odd() {
        let map = build_track();
        let point = TrackPoint::new(1, 150.0);
        let visited: Vec<_> = map.walk(&point, 2500.0, Direction::Odd).collect_vec();
        assert_eq!(visited.len(), 1);
        assert_eq!(visited[0].block_id, 1);
        assert_eq!(visited[0].offset_m, 0.0);
    }

    #[test]
    fn find_signal_even() {
        let map = build_track();
        let point = TrackPoint::new(1, 200.0);
        let (signal, distance) = map.lookup_signal_forward(&point, Direction::Even).unwrap();
        assert_eq!(signal.id, 1);
        assert_eq!(signal.position.block_id, 3);
        assert_eq!(distance, 2700.0);
    }

    #[test]
    fn find_signal_odd() {
        let map = build_track();
        let point = TrackPoint::new(3, 1100.0);
        let (signal, distance) = map.lookup_signal_forward(&point, Direction::Odd).unwrap();
        assert_eq!(signal.id, 2);
        assert_eq!(signal.position.block_id, 1);
        assert_eq!(distance, 2350.0);
    }

    #[test]
    fn find_signal_even_same_block_behind() {
        let map = build_track();
        let point = TrackPoint::new(3, 1450.0);
        assert!(map.lookup_signal_forward(&point, Direction::Even).is_none());
    }

    #[test]
    fn find_signal_odd_same_block_behind() {
        let map = build_track();
        let point = TrackPoint::new(1, 200.0);
        assert!(map.lookup_signal_forward(&point, Direction::Odd).is_none());
    }

    #[test]
    fn affected_signals_busy() {
        let map = build_track_extended();
        let block = map.blocks.get(2).unwrap();
        let mut result = map.find_affected_signals(block, TrackState::Occupied);
        result.sort_by_key(|&signal| signal.position.block_id);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].position.block_id, 1);
        assert_eq!(result[0].direction, Direction::Even);
        assert_eq!(result[1].position.block_id, 3);
        assert_eq!(result[1].direction, Direction::Odd);
    }

    #[test]
    fn affected_signals_free() {
        let map = build_track_extended();
        let block = map.blocks.get(2).unwrap();
        let mut result = map.find_affected_signals(block, TrackState::Freed);
        result.sort_by_key(|&signal| signal.position.block_id);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].position.block_id, 1);
        assert_eq!(result[0].direction, Direction::Even);
        assert_eq!(result[1].position.block_id, 3);
        assert_eq!(result[1].direction, Direction::Odd);
    }
}
