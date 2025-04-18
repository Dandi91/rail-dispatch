use crate::common::Direction;
use crate::level::{BlockData, Level};
use itertools::Itertools;

pub type BlockId = usize;

#[derive(Default)]
pub struct BlockMap {
    chunks: Vec<(BlockId, usize)>,
    blocks: Vec<Block>,
}

impl BlockMap {
    fn get_block_index(&self, id: &BlockId) -> Option<usize> {
        match self.chunks.binary_search_by(|x| x.0.cmp(id)) {
            Ok(x) => Some(self.chunks[x].1),
            Err(x) => {
                if x > 0 {
                    let chunk_start = self.chunks[x - 1];
                    Some(chunk_start.1 + (id - chunk_start.0))
                } else {
                    None
                }
            }
        }
    }

    fn get_block_by_id(&self, id: &BlockId) -> Option<&Block> {
        let index = self.get_block_index(id)?;
        let candidate = self.blocks.get(index)?;
        if candidate.id == *id {
            Some(candidate)
        } else {
            None
        }
    }

    fn get_block_by_id_mut(&mut self, id: &BlockId) -> Option<&mut Block> {
        let index = self.get_block_index(id)?;
        let candidate = self.blocks.get_mut(index)?;
        if candidate.id == *id {
            Some(candidate)
        } else {
            None
        }
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
        let block = self
            .get_block_by_id(&point.block_id)
            .expect("block not found");
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

    pub fn from_level(level: &Level) -> Self {
        let mut map = Self::from_iterable(&level.blocks);
        for conn in &level.connections {
            let start = map
                .get_block_by_id_mut(&conn.start)
                .expect("block not found");
            start.next = Some(conn.end);
            let end = map.get_block_by_id_mut(&conn.end).expect("block not found");
            end.prev = Some(conn.start);
        }
        map
    }

    pub fn from_iterable<'a, I: IntoIterator<Item = &'a BlockData>>(block_data: I) -> Self {
        let mut blocks: Vec<Block> = block_data.into_iter().map_into().collect();
        blocks.sort_by(|a, b| a.id.cmp(&b.id));

        let mut chunks: Vec<(BlockId, usize)> = vec![(blocks[0].id, 0)];
        chunks.extend(
            blocks
                .iter()
                .map(|x| x.id)
                .enumerate()
                .tuple_windows()
                .filter(|(a, b)| b.1 - a.1 != 1)
                .map(|(_, b)| (b.1, b.0)),
        );

        BlockMap { chunks, blocks }
    }
}

#[derive(Default)]
pub struct Block {
    id: BlockId,
    length_m: f64,
    lamp_id: usize,
    prev: Option<BlockId>,
    next: Option<BlockId>,
    side: Option<BlockId>,
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

#[derive(PartialEq, Clone)]
pub struct TrackPoint {
    pub block_id: BlockId,
    pub offset_m: f64,
}

impl TrackPoint {
    /// Step `length_m` meters in the `direction` along the track
    pub fn step_by(&self, length_m: f64, direction: Direction, map: &BlockMap) -> TrackPoint {
        self.walk(length_m, direction, map)
            .last()
            .expect("expected non-zero length")
    }

    pub fn walk<'a>(
        &self,
        length_m: f64,
        direction: Direction,
        map: &'a BlockMap,
    ) -> TrackWalker<'a> {
        TrackWalker {
            block_map: map,
            current_block_id: self.block_id,
            offset_m: self.offset_m,
            block_available_m: map.get_available_length(self, direction),
            length_m,
            direction,
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
}

impl Iterator for TrackWalker<'_> {
    type Item = TrackPoint;

    fn next(&mut self) -> Option<Self::Item> {
        if self.length_m <= 0.0 {
            return None;
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
            if let Some(next_block) = self
                .block_map
                .get_next(self.current_block_id, self.direction)
            {
                self.current_block_id = next_block.id;
                self.block_available_m = next_block.length_m;
                self.offset_m = match self.direction {
                    Direction::Even => 0.0,
                    Direction::Odd => next_block.length_m,
                }
            } else {
                panic!(
                    "No further block length available. Still need {} m",
                    self.length_m
                );
            }
            Some(TrackPoint {
                block_id: result_block_id,
                offset_m: 0.0,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sparse_block_map() {
        let block_ids = [1, 2, 3, 50, 51, 52, 65, 70, 100, 101];
        let block_data = block_ids.map(|x| BlockData {
            id: x,
            ..Default::default()
        });
        let block_map = BlockMap::from_iterable(&block_data);
        assert_eq!(block_map.blocks.len(), 10);
        assert_eq!(block_map.chunks.len(), 5);
        assert_eq!(block_map.chunks[0], (1, 0));
        assert_eq!(block_map.chunks[1], (50, 3));
        assert_eq!(block_map.chunks[2], (65, 6));
        assert_eq!(block_map.chunks[3], (70, 7));
        assert_eq!(block_map.chunks[4], (100, 8));

        let test_ids = [
            Ok(3),
            Ok(1),
            Ok(65),
            Ok(101),
            Err(0),
            Err(5),
            Err(69),
            Err(102),
        ];
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
                length_m: 1000.0,
                prev: Some(1),
                next: Some(3),
                ..Default::default()
            },
            Block {
                id: 3,
                length_m: 1000.0,
                prev: Some(2),
                ..Default::default()
            },
        ];
        BlockMap {
            chunks: vec![(1, 0)],
            blocks: blocks.into_iter().collect(),
        }
    }

    #[test]
    fn walk_same_block_even() {
        let map = build_track();
        let point = TrackPoint {
            block_id: 1,
            offset_m: 250.0,
        };
        let visited: Vec<TrackPoint> = point.walk(450.0, Direction::Even, &map).collect();
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
        let visited: Vec<TrackPoint> = point.walk(650.0, Direction::Odd, &map).collect();
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
        let visited: Vec<TrackPoint> = point.walk(2500.0, Direction::Even, &map).collect();
        assert_eq!(visited.len(), 3);
        assert_eq!(visited[0].block_id, 1);
        assert_eq!(visited[1].block_id, 2);
        assert_eq!(visited[2].block_id, 3);
        assert_eq!(visited[0].offset_m, 0.0);
        assert_eq!(visited[1].offset_m, 0.0);
        assert_eq!(visited[2].offset_m, 750.0);
    }

    #[test]
    fn walk_track_odd() {
        let map = build_track();
        let point = TrackPoint {
            block_id: 3,
            offset_m: 850.0,
        };
        let visited: Vec<TrackPoint> = point.walk(2500.0, Direction::Odd, &map).collect();
        assert_eq!(visited.len(), 3);
        assert_eq!(visited[0].block_id, 3);
        assert_eq!(visited[1].block_id, 2);
        assert_eq!(visited[2].block_id, 1);
        assert_eq!(visited[0].offset_m, 0.0);
        assert_eq!(visited[1].offset_m, 0.0);
        assert_eq!(visited[2].offset_m, 350.0);
    }
}
