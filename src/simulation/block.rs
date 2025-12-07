use crate::assets::{AssetHandles, LoadingState};
use crate::common::LampId;
use crate::common::{BlockId, Direction, TrainId};
use crate::level::{BlockData, ConnectionData, Level, SignalData};
use crate::simulation::messages::{BlockUpdate, BlockUpdateState, LampUpdate, SignalUpdate, SignalUpdateState};
use crate::simulation::signal::{SignalMap, TrackSignal};
use crate::simulation::sparse_vec::{Chunkable, SparseVec};
use bevy::prelude::*;
use itertools::Itertools;
use std::collections::{HashMap, HashSet, VecDeque};

#[derive(Default)]
struct BlockTracker {
    blocks: HashMap<BlockId, Vec<TrainId>>,
    trains: HashMap<TrainId, HashSet<BlockId>>,
}

impl BlockTracker {
    fn is_block_free(&self, block_id: BlockId) -> bool {
        self.blocks.get(&block_id).is_none_or(|v| v.is_empty())
    }

    /// Records block as occupied by the train id, returns true if the block was previously free
    fn set_occupied(&mut self, block_id: BlockId, train_id: TrainId) -> bool {
        // we rarely need to track more than 1 train per block (more in case of shunting),
        // but since TrainId is u32, we can afford to preallocate 2 of them just in case.
        const OCCUPIED_CAPACITY: usize = 2;
        let entry = self
            .blocks
            .entry(block_id)
            .or_insert_with(|| Vec::with_capacity(OCCUPIED_CAPACITY));
        entry.push(train_id);

        // a single train can span multiple blocks, especially at stations.
        // again, considering that BlockId is u32, we can afford to preallocate 8 of them.
        const TRAIN_BLOCKS_CAPACITY: usize = 8;
        self.trains
            .entry(train_id)
            .or_insert_with(|| HashSet::with_capacity(TRAIN_BLOCKS_CAPACITY))
            .insert(block_id);

        entry.len() == 1
    }

    /// Records block as freed by the train id, returns true if the block is now free
    fn set_freed(&mut self, block_id: BlockId, train_id: TrainId) -> bool {
        self.trains.get_mut(&train_id).map(|v| v.remove(&block_id));
        self.blocks.get_mut(&block_id).map_or(false, |v| {
            v.retain(|&x| x != train_id);
            v.is_empty()
        })
    }

    /// Despawns the train and removes it from all blocks occupied by it
    fn despawn_train(&mut self, train_id: TrainId) -> Option<HashSet<BlockId>> {
        self.trains.remove(&train_id).inspect(|blocks| {
            blocks.iter().for_each(|&block_id| {
                self.set_freed(block_id, train_id);
            })
        })
    }
}

#[derive(Default, Resource)]
pub struct BlockMap {
    blocks: SparseVec<Block>,
    signals: SignalMap,
    tracker: BlockTracker,
}

impl BlockMap {
    pub fn get_track_point(&self, block_id: BlockId, offset_m: f64) -> TrackPoint {
        let block = self.blocks.get(block_id).expect("block not found");

        if offset_m > block.length_m {
            panic!("Incorrect track point, offset is greater than the block length");
        }
        if offset_m < 0. {
            panic!("Incorrect track point, offset cannot be negative");
        }

        TrackPoint { block_id, offset_m }
    }

    pub fn get_available_length(&self, point: &TrackPoint, direction: Direction) -> f64 {
        let block = self.blocks.get(point.block_id).expect("block not found");
        if direction == Direction::Even {
            block.length_m - point.offset_m
        } else {
            point.offset_m
        }
    }

    pub fn get_next(&self, block_id: BlockId, direction: Direction) -> Option<&Block> {
        let block = self.blocks.get(block_id).expect("block not found");
        let next = match direction {
            Direction::Even => block.next?,
            Direction::Odd => block.prev?,
        };
        Some(self.blocks.get(next).expect("block not found"))
    }

    pub fn despawn_train(&mut self, train_id: TrainId, block_updates: &mut MessageWriter<BlockUpdate>) {
        if let Some(blocks) = self.tracker.despawn_train(train_id) {
            block_updates.write_batch(blocks.iter().map(|b| BlockUpdate::freed(*b, train_id)));
        }
    }

    pub fn process_block_updates(
        &mut self,
        block_updates: &mut MessageReader<BlockUpdate>,
        lamp_updates: &mut MessageWriter<LampUpdate>,
        signal_updates: &mut MessageWriter<SignalUpdate>,
    ) {
        for update in block_updates.read() {
            let changed = match update.state {
                BlockUpdateState::Occupied => self.tracker.set_occupied(update.block_id, update.train_id),
                BlockUpdateState::Freed => self.tracker.set_freed(update.block_id, update.train_id),
            };

            if !changed {
                return;
            }
            let block = self.blocks.get(update.block_id).unwrap();
            lamp_updates.write(LampUpdate::from_block_state(update.state, block.lamp_id));
            signal_updates.write_batch(
                self.find_affected_signals(block, update.state)
                    .map(|signal| SignalUpdate::from_block_change(signal.id, update.state)),
            );
        }
    }

    pub fn process_signal_updates(
        &mut self,
        signal_updates: &mut MessageReader<SignalUpdate>,
        lamp_updates: &mut MessageWriter<LampUpdate>,
    ) {
        let mut queue = VecDeque::from_iter(signal_updates.read().cloned());
        while let Some(update) = queue.pop_front() {
            let signal = self.signals.get(update.signal_id).expect("invalid signal ID");
            let (prev, _) = self.lookup_signal(&signal.position, signal.direction.reverse());
            match update.state {
                SignalUpdateState::BlockChange(block_update) => {
                    lamp_updates.write(LampUpdate::from_block_state(!block_update, signal.lamp_id));
                    queue.push_back(SignalUpdate::new(prev.id, SignalUpdateState::SignalPropagation));
                }
                SignalUpdateState::SignalPropagation => {}
            }
        }
    }

    /// Given a block state update, returns an iterator of all signals that it affects
    /// (at most 2 signals per block, one in each direction).
    fn find_affected_signals(&self, block: &Block, state: BlockUpdateState) -> impl Iterator<Item = &TrackSignal> {
        let point = block.middle();
        [Direction::Even, Direction::Odd]
            .iter()
            .map(move |&direction| {
                self.walk(&point, f64::INFINITY, direction)
                    .skip(1)
                    .find_map(|p| self.signals.find_signal(p.block_id, direction.reverse()))
            })
            .flatten()
            .filter(move |signal| matches!(state, BlockUpdateState::Occupied) || self.is_signal_free(signal))
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

    /// Tries to find a signal in the `direction` along the track, returning tuple of signal and distance to it
    pub fn lookup_signal(&self, start: &TrackPoint, direction: Direction) -> (&TrackSignal, f64) {
        let reversed = direction.reverse();
        let mut length = -self.get_available_length(start, reversed);
        for (idx, point) in self.walk(start, f64::INFINITY, direction).enumerate() {
            if let Some(signal) = self.signals.find_signal(point.block_id, direction) {
                let diff = direction.apply_sign(signal.position.offset_m - start.offset_m);
                if idx > 0 || diff > 0.0 {
                    length += self.get_available_length(&signal.position, reversed);
                    return (signal, length);
                }
            }
            length += self.get_available_length(&point, reversed);
        }
        unreachable!("The loop should always return")
    }

    pub fn walk(&self, start: &TrackPoint, length_m: f64, direction: Direction) -> TrackWalker<'_> {
        let block = self.blocks.get(start.block_id).unwrap();
        TrackWalker {
            block_map: self,
            current_block_id: start.block_id,
            offset_m: start.offset_m,
            block_available_m: self.get_available_length(start, direction),
            current_block_length_m: block.length_m,
            length_m,
            direction,
        }
    }

    pub fn get_lamp_info(&self, id: LampId) -> Option<String> {
        if let Some(block) = self.blocks.iter().find(|&b| b.lamp_id == id) {
            if self.tracker.is_block_free(block.id) {
                return Some(format!("Block ID: {}\nFree", block.id));
            }
            let trains = self.tracker.blocks.get(&block.id).unwrap().into_iter().join(", ");
            return Some(format!("Block ID: {}\nTrains: {}", block.id, trains));
        }
        if let Some(signal) = self.signals.iter().find(|&s| s.lamp_id == id) {
            return Some(format!(
                "Signal '{}' (ID {})\nAllowed speed: {:.0} km/h\nBlock ID: {}",
                signal.name, signal.id, signal.speed_ctrl.allowed_kmh, signal.position.block_id
            ));
        }
        None
    }

    pub fn from_level(level: &Level) -> Self {
        Self::from_iterable(&level.blocks, &level.signals, &level.connections)
    }

    pub fn from_iterable<'a, I, J, K>(block_data: I, signal_data: J, connection_data: K) -> Self
    where
        I: IntoIterator<Item = &'a BlockData>,
        J: IntoIterator<Item = &'a SignalData>,
        K: IntoIterator<Item = &'a ConnectionData>,
    {
        let mut blocks: SparseVec<Block> = block_data.into_iter().map_into().collect();
        let signals: SignalMap = signal_data.into_iter().map_into().collect();

        for conn in connection_data {
            let start = blocks.get_mut(conn.start).expect("start block not found");
            start.next = Some(conn.end);
            let end = blocks.get_mut(conn.end).expect("end block not found");
            end.prev = Some(conn.start);
        }

        BlockMap {
            blocks,
            signals,
            ..Default::default()
        }
    }
}

#[derive(Default)]
pub struct Block {
    id: BlockId,
    length_m: f64,
    lamp_id: LampId,
    prev: Option<BlockId>,
    next: Option<BlockId>,
}

impl From<&BlockData> for Block {
    fn from(value: &BlockData) -> Self {
        Block {
            id: value.id,
            length_m: value.length,
            lamp_id: value.lamp_id,
            ..Default::default()
        }
    }
}

impl Chunkable for Block {
    #[inline]
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
}

#[derive(Default, Debug, Clone)]
pub struct TrackPoint {
    pub block_id: BlockId,
    pub offset_m: f64,
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
        if self.length_m <= 0.0 {
            return None;
        }

        if self.offset_m.is_nan() {
            panic!("No further block length available. Still need {} m", self.length_m);
        }

        if self.length_m < self.block_available_m {
            let new_offset = self.offset_m + self.direction.apply_sign(self.length_m);
            self.length_m = 0.0;
            Some(TrackPoint {
                block_id: self.current_block_id,
                offset_m: new_offset,
            })
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
            Some(TrackPoint {
                block_id: result_block_id,
                offset_m: match self.direction {
                    Direction::Even => result_block_length,
                    Direction::Odd => 0.0,
                },
            })
        }
    }
}

pub struct MapPlugin;

impl Plugin for MapPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnExit(LoadingState::Loading), setup).add_systems(
            Update,
            (block_updates, signal_updates).run_if(in_state(LoadingState::Loaded)),
        );
    }
}

fn setup(handles: Res<AssetHandles>, levels: Res<Assets<Level>>, mut commands: Commands) {
    let level = levels.get(&handles.level).unwrap();
    commands.insert_resource(BlockMap::from_level(level));
}

fn block_updates(
    mut block_map: ResMut<BlockMap>,
    mut block_updates: MessageReader<BlockUpdate>,
    mut lamp_updates: MessageWriter<LampUpdate>,
    mut signal_updates: MessageWriter<SignalUpdate>,
) {
    block_map.process_block_updates(&mut block_updates, &mut lamp_updates, &mut signal_updates);
}

fn signal_updates(
    mut block_map: ResMut<BlockMap>,
    mut signal_updates: MessageReader<SignalUpdate>,
    mut lamp_updates: MessageWriter<LampUpdate>,
) {
    block_map.process_signal_updates(&mut signal_updates, &mut lamp_updates);
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
                position: TrackPoint {
                    block_id: 3,
                    offset_m: 1400.0,
                },
                direction: Direction::Even,
                ..Default::default()
            },
            TrackSignal {
                id: 2,
                position: TrackPoint {
                    block_id: 1,
                    offset_m: 250.0,
                },
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
                    position: TrackPoint {
                        block_id: idx,
                        offset_m: 490.0,
                    },
                    direction: Direction::Even,
                    ..Default::default()
                },
                TrackSignal {
                    id: idx * 2,
                    position: TrackPoint {
                        block_id: idx,
                        offset_m: 10.0,
                    },
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
        let point = TrackPoint {
            block_id: 1,
            offset_m: 250.0,
        };
        let visited: Vec<TrackPoint> = map.walk(&point, 450.0, Direction::Even).collect();
        assert_eq!(visited.len(), 1);
        assert_eq!(visited[0].block_id, 1);
        assert_eq!(visited[0].offset_m, 700.0);
    }

    #[test]
    fn walk_same_block_odd() {
        let map = build_track();
        let point = TrackPoint {
            block_id: 1,
            offset_m: 750.0,
        };
        let visited: Vec<TrackPoint> = map.walk(&point, 650.0, Direction::Odd).collect();
        assert_eq!(visited.len(), 1);
        assert_eq!(visited[0].block_id, 1);
        assert_eq!(visited[0].offset_m, 100.0);
    }

    #[test]
    fn walk_track_even() {
        let map = build_track();
        let point = TrackPoint {
            block_id: 1,
            offset_m: 250.0,
        };
        let visited: Vec<TrackPoint> = map.walk(&point, 2500.0, Direction::Even).collect();
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
        let point = TrackPoint {
            block_id: 3,
            offset_m: 1050.0,
        };
        let visited: Vec<TrackPoint> = map.walk(&point, 2500.0, Direction::Odd).collect();
        assert_eq!(visited.len(), 3);
        assert_eq!(visited[0].block_id, 3);
        assert_eq!(visited[1].block_id, 2);
        assert_eq!(visited[2].block_id, 1);
        assert_eq!(visited[0].offset_m, 0.0);
        assert_eq!(visited[1].offset_m, 0.0);
        assert_eq!(visited[2].offset_m, 50.0);
    }

    #[test]
    #[should_panic(expected = "No further block length available. Still need 1850 m")]
    fn walk_track_even_panic() {
        let map = build_track();
        let point = TrackPoint {
            block_id: 3,
            offset_m: 850.0,
        };
        map.walk(&point, 2500.0, Direction::Even).collect_vec();
    }

    #[test]
    #[should_panic(expected = "No further block length available. Still need 2350 m")]
    fn walk_track_odd_panic() {
        let map = build_track();
        let point = TrackPoint {
            block_id: 1,
            offset_m: 150.0,
        };
        map.walk(&point, 2500.0, Direction::Odd).collect_vec();
    }

    #[test]
    fn find_signal_even() {
        let map = build_track();
        let point = TrackPoint {
            block_id: 1,
            offset_m: 200.0,
        };
        let (signal, distance) = map.lookup_signal(&point, Direction::Even);
        assert_eq!(signal.id, 1);
        assert_eq!(signal.position.block_id, 3);
        assert_eq!(distance, 2700.0);
    }

    #[test]
    fn find_signal_odd() {
        let map = build_track();
        let point = TrackPoint {
            block_id: 3,
            offset_m: 1100.0,
        };
        let (signal, distance) = map.lookup_signal(&point, Direction::Odd);
        assert_eq!(signal.id, 2);
        assert_eq!(signal.position.block_id, 1);
        assert_eq!(distance, 2350.0);
    }

    #[test]
    #[should_panic(expected = "No further block length available. Still need inf m")]
    fn find_signal_even_same_block_behind() {
        let map = build_track();
        let point = TrackPoint {
            block_id: 3,
            offset_m: 1450.0,
        };
        map.lookup_signal(&point, Direction::Even);
    }

    #[test]
    #[should_panic(expected = "No further block length available. Still need inf m")]
    fn find_signal_odd_same_block_behind() {
        let map = build_track();
        let point = TrackPoint {
            block_id: 1,
            offset_m: 200.0,
        };
        map.lookup_signal(&point, Direction::Odd);
    }

    #[test]
    fn affected_signals_busy() {
        let map = build_track_extended();
        let block = map.blocks.get(2).unwrap();
        let mut result = map
            .find_affected_signals(block, BlockUpdateState::Occupied)
            .collect_vec();
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
        let mut result = map.find_affected_signals(block, BlockUpdateState::Freed).collect_vec();
        result.sort_by_key(|&signal| signal.position.block_id);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].position.block_id, 1);
        assert_eq!(result[0].direction, Direction::Even);
        assert_eq!(result[1].position.block_id, 3);
        assert_eq!(result[1].direction, Direction::Odd);
    }
}
