use crate::common::{Direction, TrainID};

pub enum TrainKind {
    Extra = 0,
    Passenger = 1,
    Cargo = 2,
    Shunting = 3,
}

pub struct TrainDisplayState {
    pub id: TrainID,
    pub number: String,
    pub kind: TrainKind,
    pub direction: Direction,
}
