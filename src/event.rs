use crate::clock::ClockPayload;
use crate::common::TrainId;
use crate::display::train::TrainDisplayState;
use crate::simulation::block::BlockId;
use crate::simulation::train::{TrainSpawnState, TrainStatusUpdate};

pub enum Command {
    SetTimeScale(f64),
    TrainSpawn(Box<TrainSpawnState>),
    TrainDespawn(TrainId),
    Shutdown,
}

pub enum SimulationUpdate {
    Clock(ClockPayload),
    TrainStates(f64, Vec<TrainStatusUpdate>),
    BlockOccupation(BlockId, bool),
    RegisterTrain(TrainDisplayState),
    UnregisterTrain(TrainId),
}
