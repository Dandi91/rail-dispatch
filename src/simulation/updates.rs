use crate::common::{Direction, TrainId};
use crate::simulation::block::BlockId;
use bevy::prelude::Resource;
use std::collections::VecDeque;

#[derive(Resource)]
pub struct UpdateQueues {
    pub block_updates: BlockUpdateQueue,
    pub signal_updates: VecDeque<SignalUpdate>,
}

impl UpdateQueues {
    pub fn new() -> Self {
        UpdateQueues {
            block_updates: BlockUpdateQueue::with_capacity(8),
            signal_updates: VecDeque::with_capacity(8),
        }
    }

    pub fn report(&self) {
        println!("Update queues capacity:");
        println!("  block:\t{}", self.block_updates.capacity());
        println!("  signal:\t{}", self.signal_updates.capacity());
    }
}

pub struct BlockUpdate {
    pub block_id: BlockId,
    pub train_id: TrainId,
    pub state: bool,
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

    pub fn drain(&mut self) -> impl Iterator<Item = BlockUpdate> {
        self.0.drain(..)
    }

    pub fn capacity(&self) -> usize {
        self.0.capacity()
    }
}

pub struct SignalUpdate {
    pub block_id: BlockId,
    pub direction: Direction,
}
