use crate::common::{Direction, TrainId};
use crate::display::lamp::LampId;
use crate::level::{BlockData, ConnectionData, Level, SignalData};
use itertools::Itertools;
use std::collections::{HashMap, VecDeque};

pub type BlockId = usize;
pub type SignalId = usize;

#[derive(Debug)]
struct Chunk {
    start_id: BlockId,
    start_index: usize,
}

impl Default for Chunk {
    fn default() -> Self {
        Chunk {
            start_id: 1,
            start_index: 0,
        }
    }
}

impl PartialEq<(BlockId, usize)> for Chunk {
    fn eq(&self, (start_id, start_index): &(usize, usize)) -> bool {
        self.start_id == *start_id && self.start_index == *start_index
    }
}

#[derive(Default)]
pub struct BlockMap {
    chunks: Vec<Chunk>,
    blocks: Vec<Block>,
    signals: HashMap<(BlockId, Direction), TrackSignal>,
    occupied_blocks: HashMap<BlockId, Vec<TrainId>>,
}

impl BlockMap {
    fn get_block_index(&self, id: &BlockId) -> Option<usize> {
        match self.chunks.binary_search_by(|x| x.start_id.cmp(id)) {
            Ok(x) => Some(self.chunks[x].start_index),
            Err(x) => {
                if x > 0 {
                    let chunk = &self.chunks[x - 1];
                    Some(chunk.start_index + (id - chunk.start_id))
                } else {
                    None
                }
            }
        }
    }

    fn get_block_by_id(&self, id: &BlockId) -> Option<&Block> {
        let index = self.get_block_index(id)?;
        let candidate = self.blocks.get(index)?;
        if candidate.id == *id { Some(candidate) } else { None }
    }

    pub fn get_track_point(&self, block_id: BlockId, offset_m: f64) -> TrackPoint {
        let block = self.get_block_by_id(&block_id).expect("block not found");

        if offset_m > block.length_m {
            panic!("Incorrect track point, offset is greater than the block length");
        }
        if offset_m < 0. {
            panic!("Incorrect track point, offset cannot be negative");
        }

        TrackPoint { block_id, offset_m }
    }

    pub fn get_available_length(&self, point: &TrackPoint, direction: Direction) -> f64 {
        let block = self.get_block_by_id(&point.block_id).expect("block not found");
        if direction == Direction::Even {
            block.length_m - point.offset_m
        } else {
            point.offset_m
        }
    }

    pub fn get_next(&self, block_id: BlockId, direction: Direction) -> Option<&Block> {
        let block = self.get_block_by_id(&block_id).expect("block not found");
        let next = match direction {
            Direction::Even => block.next?,
            Direction::Odd => block.prev?,
        };
        Some(self.get_block_by_id(&next).expect("block not found"))
    }

    pub fn is_block_free(&self, block_id: BlockId) -> bool {
        self.occupied_blocks.get(&block_id).is_none_or(|v| v.is_empty())
    }

    pub fn get_signals(&self) -> impl Iterator<Item = &TrackSignal> {
        self.signals.values()
    }

    pub fn process_updates(&mut self, updates: &mut BlockUpdateQueue) -> impl Iterator<Item = (LampId, bool)> {
        updates
            .0
            .drain(..)
            .map(|u| {
                let vec = self
                    .occupied_blocks
                    .entry(u.block_id)
                    .or_insert_with(|| Vec::with_capacity(1));
                let block_change = if u.state {
                    // block occupation
                    vec.push(u.train_id);
                    if vec.len() == 1 {
                        self.get_block_by_id(&u.block_id)
                    } else {
                        None
                    }
                } else {
                    // block freeing
                    vec.retain(|&x| x != u.train_id);
                    if vec.is_empty() {
                        self.get_block_by_id(&u.block_id)
                    } else {
                        None
                    }
                };

                let mut result = [None; 3];
                if let Some(block) = block_change {
                    result[0].replace((block.lamp_id, u.state));
                    self.find_affected_signals(block, u.state)
                        .map(|signal| (signal.lamp_id, !u.state))
                        .enumerate()
                        .fold(&mut result, |acc, (idx, item)| {
                            acc[idx + 1].replace(item);
                            acc
                        });
                }
                result
            })
            .flatten()
            .flatten()
    }

    fn find_affected_signals(&self, block: &Block, state: bool) -> impl Iterator<Item = &TrackSignal> {
        let point = block.middle();
        [Direction::Even, Direction::Odd]
            .iter()
            .map(move |&direction| {
                self.walk(&point, f64::INFINITY, direction)
                    .skip(1)
                    .find_map(|p| self.signals.get(&(p.block_id, direction.reverse())))
            })
            .flatten()
            .filter(move |signal| state || self.is_signal_free(signal))
    }

    fn is_signal_free(&self, signal: &TrackSignal) -> bool {
        self.walk(&signal.position, f64::INFINITY, signal.direction)
            .skip(1)
            .take_while_inclusive(|p| self.signals.get(&(p.block_id, signal.direction)).is_none())
            .all(|p| self.is_block_free(p.block_id))
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
            if let Some(signal) = self.signals.get(&(point.block_id, direction)) {
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
        let block = self.get_block_by_id(&start.block_id).unwrap();
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

    pub fn from_level(level: &Level) -> Self {
        Self::from_iterable(&level.blocks, &level.signals, &level.connections)
    }

    pub fn from_iterable<'a, I, J, K>(block_data: I, signal_data: J, connection_data: K) -> Self
    where
        I: IntoIterator<Item = &'a BlockData>,
        J: IntoIterator<Item = &'a SignalData>,
        K: IntoIterator<Item = &'a ConnectionData>,
    {
        let signals: HashMap<(BlockId, Direction), TrackSignal> = signal_data
            .into_iter()
            .map(|x| ((x.block_id, x.direction), x.into()))
            .collect();
        let mut blocks: Vec<Block> = block_data.into_iter().map_into().collect();
        blocks.sort_by_key(|block| block.id);

        for conn in connection_data {
            let start = blocks
                .binary_search_by(|block| block.id.cmp(&conn.start))
                .expect("start block not found");
            let end = blocks
                .binary_search_by(|block| block.id.cmp(&conn.end))
                .expect("end block not found");
            blocks[start].next = Some(conn.end);
            blocks[end].prev = Some(conn.start);
        }

        let mut chunks: Vec<Chunk> = vec![Chunk {
            start_id: blocks[0].id,
            start_index: 0,
        }];
        chunks.extend(
            blocks
                .iter()
                .map(|x| x.id)
                .enumerate()
                .tuple_windows()
                .filter(|(a, b)| b.1 - a.1 != 1)
                .map(|(_, b)| Chunk {
                    start_id: b.1,
                    start_index: b.0,
                }),
        );

        BlockMap {
            chunks,
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

impl Block {
    pub fn middle(&self) -> TrackPoint {
        TrackPoint {
            block_id: self.id,
            offset_m: self.length_m / 2.0,
        }
    }
}

struct BlockUpdate {
    block_id: BlockId,
    train_id: TrainId,
    state: bool,
}

pub struct BlockUpdateQueue(VecDeque<BlockUpdate>);
impl BlockUpdateQueue {
    pub fn new() -> Self {
        BlockUpdateQueue(VecDeque::new())
    }

    pub fn with_capacity(capacity: usize) -> Self {
        BlockUpdateQueue(VecDeque::with_capacity(capacity))
    }

    pub fn occupied(&mut self, block_id: BlockId, train_id: TrainId) {
        self.0.push_back(BlockUpdate {
            block_id,
            train_id,
            state: true,
        });
    }

    pub fn freed(&mut self, block_id: BlockId, train_id: TrainId) {
        self.0.push_back(BlockUpdate {
            block_id,
            train_id,
            state: false,
        });
    }

    pub fn get_capacity(&self) -> usize {
        self.0.capacity()
    }
}

#[derive(Default, Debug, Clone)]
pub struct SpeedControl {
    allowed_kmh: f64,
}

#[derive(Default, Debug, Clone)]
pub struct TrackSignal {
    id: SignalId,
    position: TrackPoint,
    lamp_id: LampId,
    direction: Direction,
    name: String,
    speed_ctrl: SpeedControl,
}

impl From<&SignalData> for TrackSignal {
    fn from(value: &SignalData) -> Self {
        TrackSignal {
            id: value.id,
            position: TrackPoint {
                block_id: value.block_id,
                offset_m: value.offset_m,
            },
            lamp_id: value.lamp_id,
            direction: value.direction,
            name: value.name.clone(),
            ..Default::default()
        }
    }
}

impl TrackSignal {
    pub fn get_allowed_speed_mps(&self) -> f64 {
        self.speed_ctrl.allowed_kmh / 3.6
    }

    pub fn get_name(&self) -> &str {
        self.name.as_str()
    }

    pub fn get_lamp_id(&self) -> LampId {
        self.lamp_id
    }
}

#[derive(PartialEq, Clone, Default, Debug)]
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::wrap;

    #[test]
    fn test_sparse_block_map() {
        let block_ids = [1, 2, 3, 50, 51, 52, 65, 70, 100, 101];
        let block_data = block_ids.map(|x| BlockData {
            id: x,
            ..Default::default()
        });
        let block_map = BlockMap::from_iterable(&block_data, [], []);
        assert_eq!(block_map.blocks.len(), 10);
        assert_eq!(block_map.chunks.len(), 5);
        assert_eq!(block_map.chunks[0], (1, 0));
        assert_eq!(block_map.chunks[1], (50, 3));
        assert_eq!(block_map.chunks[2], (65, 6));
        assert_eq!(block_map.chunks[3], (70, 7));
        assert_eq!(block_map.chunks[4], (100, 8));

        let test_ids = [Ok(3), Ok(1), Ok(65), Ok(101), Err(0), Err(5), Err(69), Err(102)];
        for test in test_ids.iter() {
            match test {
                Ok(id) => {
                    let block = block_map.get_block_by_id(id);
                    assert_eq!(block.unwrap().id, *id);
                }
                Err(id) => {
                    let block = block_map.get_block_by_id(id);
                    assert!(block.is_none());
                }
            }
        }
    }

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
            chunks: vec![Chunk::default()],
            blocks: blocks.into_iter().collect(),
            signals: signals.map(|x| ((x.position.block_id, x.direction), x)).into(),
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
            chunks: vec![Chunk::default()],
            blocks: blocks.into_iter().collect(),
            signals: signals
                .flatten()
                .map(|x| ((x.position.block_id, x.direction), x))
                .collect(),
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
        let block = map.get_block_by_id(&2).unwrap();
        let mut result = map.find_affected_signals(block, true).collect_vec();
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
        let block = map.get_block_by_id(&2).unwrap();
        let mut result = map.find_affected_signals(block, false).collect_vec();
        result.sort_by_key(|&signal| signal.position.block_id);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].position.block_id, 1);
        assert_eq!(result[0].direction, Direction::Even);
        assert_eq!(result[1].position.block_id, 3);
        assert_eq!(result[1].direction, Direction::Odd);
    }
}
