use crate::common::{Direction, TrainId};
use crate::common::BlockId;
use bevy::prelude::*;

#[derive(Message)]
pub struct BlockUpdate {
    pub block_id: BlockId,
    pub train_id: TrainId,
    pub state: bool,
}

impl BlockUpdate {
    pub fn occupied(block_id: BlockId, train_id: TrainId) -> Self {
        BlockUpdate {
            block_id,
            train_id,
            state: true,
        }
    }

    pub fn freed(block_id: BlockId, train_id: TrainId) -> Self {
        BlockUpdate {
            block_id,
            train_id,
            state: false,
        }
    }
}

pub struct MessagingPlugin;

impl Plugin for MessagingPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<BlockUpdate>();
    }
}

pub struct SignalUpdate {
    pub block_id: BlockId,
    pub direction: Direction,
}
