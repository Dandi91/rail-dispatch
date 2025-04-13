use crate::block::{BlockId, BlockMap, TrackPoint};
use crate::common::{Direction, SimObject};
use std::collections::VecDeque;

pub enum TrainPriority {
    Extra = 0,
    Passenger = 1,
    Cargo = 2,
    Shunting = 3,
}

#[derive(Default)]
pub struct TrainControls {
    throttle: f64,
    brake_level: f64,
}

pub fn train_controls_percentage(controls: &TrainControls) -> isize {
    let value = if controls.throttle != 0.0 {
        controls.throttle
    } else {
        -controls.brake_level
    };
    value as isize
}

struct RailVehicle {
    mass_kg: f64,
    length_m: f64,
    max_braking_force_n: f64,
}

pub struct TrainCar {
    vehicle: RailVehicle,
    cargo_mass_kg: f64,
}

pub struct Locomotive {
    vehicle: RailVehicle,
    power_w: f64,
    max_tractive_effort_n: f64,
}

fn get_loco_tractive_effort(loco: &Locomotive, speed_mps: f64, throttle: f64) -> f64 {
    let max_tractive_effort_n = loco.max_tractive_effort_n * throttle;
    if speed_mps < 0.01 {
        max_tractive_effort_n
    } else {
        let power_w = loco.power_w * throttle;
        let tractive_effort = power_w / speed_mps;
        f64::min(tractive_effort, max_tractive_effort_n)
    }
}

pub struct Train {
    pub priority: TrainPriority,
    pub number: String,

    pub controls: TrainControls,
    pub speed_mps: f64,
    pub target_speed_mps: f64,
    pub acceleration_mps2: f64,

    pub direction: Direction,
    pub locomotives: Vec<Locomotive>,
    pub cars: Vec<TrainCar>,

    occupied_blocks: VecDeque<BlockId>,
    front_position: TrackPoint,
    back_position: TrackPoint,
    target_speed_margin_mps: f64,
}

impl Train {
    pub fn spawn_at(
        priority: TrainPriority,
        number: String,
        speed_mps: f64,
        direction: Direction,
        spawn_point: TrackPoint,
        block_map: &BlockMap,
        locomotives: Vec<Locomotive>,
        cars: Vec<TrainCar>,
    ) -> Self {
        let length_m = locomotives.iter().map(|x| x.vehicle.length_m).sum::<f64>()
            + cars.iter().map(|x| x.vehicle.length_m).sum::<f64>();
        let mut trace: Vec<TrackPoint> = spawn_point
            .walk(length_m.max(1.0), Direction::Even, block_map)
            .collect();

        Train {
            priority,
            number,
            controls: Default::default(),
            speed_mps,
            target_speed_mps: speed_mps,
            acceleration_mps2: 0.0,
            direction,
            locomotives,
            cars,
            occupied_blocks: trace.iter().map(|x| x.block_id).collect(),
            front_position: spawn_point,
            back_position: trace.pop().unwrap(),
            target_speed_margin_mps: 0.0,
        }
    }
}

impl SimObject for Train {
    fn tick(&mut self, dt: f64) {
        // todo!()
    }
}
