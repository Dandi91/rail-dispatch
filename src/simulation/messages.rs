use crate::common::BlockId;
use crate::common::{Direction, LampId, TrainId};
use bevy::prelude::*;
use std::ops::Not;

pub enum BlockUpdateState {
    Occupied,
    Freed,
}

impl Not for BlockUpdateState {
    type Output = Self;
    fn not(self) -> Self::Output {
        match self {
            BlockUpdateState::Occupied => BlockUpdateState::Freed,
            BlockUpdateState::Freed => BlockUpdateState::Occupied,
        }
    }
}

#[derive(Message)]
pub struct BlockUpdate {
    pub block_id: BlockId,
    pub train_id: TrainId,
    pub state: BlockUpdateState,
}

impl BlockUpdate {
    pub fn occupied(block_id: BlockId, train_id: TrainId) -> Self {
        BlockUpdate {
            block_id,
            train_id,
            state: BlockUpdateState::Occupied,
        }
    }

    pub fn freed(block_id: BlockId, train_id: TrainId) -> Self {
        BlockUpdate {
            block_id,
            train_id,
            state: BlockUpdateState::Freed,
        }
    }
}

pub enum LampUpdateState {
    On,
    Off,
}

impl Not for LampUpdateState {
    type Output = Self;
    fn not(self) -> Self::Output {
        match self {
            LampUpdateState::On => LampUpdateState::Off,
            LampUpdateState::Off => LampUpdateState::On,
        }
    }
}

#[derive(Message)]
pub struct LampUpdate {
    pub lamp_id: LampId,
    pub state: LampUpdateState,
}

impl LampUpdate {
    pub fn on(lamp_id: LampId) -> Self {
        LampUpdate {
            lamp_id,
            state: LampUpdateState::On,
        }
    }

    pub fn off(lamp_id: LampId) -> Self {
        LampUpdate {
            lamp_id,
            state: LampUpdateState::Off,
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
