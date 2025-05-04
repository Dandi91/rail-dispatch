use crate::clock::ClockPayload;
use crate::common::TrainId;
use crate::display::lamp::LampId;
use crate::display::train::TrainDisplayState;
use crate::simulation::train::{TrainSpawnState, TrainStatusUpdate};

pub enum Command {
    SetTimeScale(f64),
    TrainSpawn(Box<TrainSpawnState>),
    TrainDespawn(TrainId),
    Shutdown,
}

pub enum SimulationUpdate {
    Clock(ClockPayload),
    SimDuration(f64),
    TrainStates(f64, Vec<TrainStatusUpdate>),
    LampState(LampId, bool),
    RegisterTrain(TrainDisplayState),
    UnregisterTrain(TrainId),
}
