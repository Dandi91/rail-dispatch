use crate::common::TrainID;
use crate::display::train::TrainDisplayState;
use crate::simulation::train::{TrainSpawnState, TrainStatusUpdate};

pub enum Command {
    SetTimeScale(f64),
    TrainSpawn(Box<TrainSpawnState>),
    TrainDespawn(TrainID),
    Shutdown,
}

pub enum SimulationUpdate {
    Clock(f64),
    TrainState(TrainStatusUpdate),
    BlockOccupation,
    RegisterTrain(TrainDisplayState),
    UnregisterTrain(TrainID),
}

#[derive(Default)]
pub struct Event<T> {
    callbacks: Vec<fn(&T)>,
}

impl<T> Event<T> {
    pub fn new(callback: fn(&T)) -> Self {
        Self {
            callbacks: vec![callback],
        }
    }

    pub fn subscribe(&mut self, callback: fn(&T)) {
        self.callbacks.push(callback);
    }

    pub fn notify(&self, data: &T) {
        for callback in &self.callbacks {
            callback(data);
        }
    }
}
