use crate::common::{Direction, TrainId};

pub enum TrainKind {
    Extra = 0,
    Passenger = 1,
    Cargo = 2,
    Shunting = 3,
}

pub struct TrainDisplayState {
    pub id: TrainId,
    pub number: String,
    pub kind: TrainKind,
    pub direction: Direction,
}
