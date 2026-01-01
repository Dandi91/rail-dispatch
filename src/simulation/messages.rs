use crate::common::{BlockId, LampId, SignalId, SwitchId, TrainId};
use crate::simulation::signal::SignalAspect;
use bevy::prelude::*;
use std::ops::Not;

#[derive(Copy, Clone)]
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

#[derive(Copy, Clone)]
pub enum LampUpdateState {
    On,
    Off,
    Pending,
}

#[derive(Message)]
pub struct LampUpdate {
    pub lamp_id: LampId,
    pub state: LampUpdateState,
}

impl LampUpdate {
    pub fn from_block_state(update_state: BlockUpdateState, lamp_id: LampId) -> Self {
        match update_state {
            BlockUpdateState::Occupied => Self::on(lamp_id),
            BlockUpdateState::Freed => Self::off(lamp_id),
        }
    }

    pub fn from_signal_aspect(aspect: SignalAspect, lamp_id: LampId) -> Self {
        Self {
            lamp_id,
            state: match aspect {
                SignalAspect::Unrestricting => LampUpdateState::On,
                SignalAspect::Restricting => LampUpdateState::Pending,
                SignalAspect::Forbidding => LampUpdateState::Off,
            },
        }
    }

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

#[derive(Copy, Clone)]
pub enum SignalUpdateState {
    /// Update caused by the change of the guarded block state
    BlockChange(BlockUpdateState),
    /// Update caused by the change of the next signal state
    SignalPropagation(SignalAspect),
}

#[derive(Message, Clone)]
pub struct SignalUpdate {
    pub signal_id: SignalId,
    pub state: SignalUpdateState,
}

impl SignalUpdate {
    pub fn new(signal_id: SignalId, state: SignalUpdateState) -> Self {
        Self { signal_id, state }
    }

    pub fn from_block_change(signal_id: SignalId, state: BlockUpdateState) -> Self {
        Self::new(signal_id, SignalUpdateState::BlockChange(state))
    }
}

#[derive(Message)]
pub struct SwitchUpdate {
    pub switch_id: SwitchId,
}

impl SwitchUpdate {
    pub fn new(switch_id: SwitchId) -> Self {
        Self { switch_id }
    }
}

pub struct MessagingPlugin;

impl Plugin for MessagingPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<BlockUpdate>()
            .add_message::<LampUpdate>()
            .add_message::<SignalUpdate>()
            .add_message::<SwitchUpdate>();
    }
}
