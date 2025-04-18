use crate::common::{Direction, TrainID};
use crate::simulation::block::{BlockId, BlockMap, TrackPoint};
use std::collections::VecDeque;

#[derive(Default)]
pub struct TrainControls {
    throttle: f64,
    brake_level: f64,
}

impl TrainControls {
    pub fn as_percentage(&self) -> i32 {
        if self.throttle != 0.0 {
            (self.throttle * 100.0) as i32
        } else {
            -(self.brake_level * 100.0) as i32
        }
    }
}

#[derive(Copy, Clone)]
pub enum VehicleType {
    Locomotive,
    RailCar,
}

#[derive(Copy, Clone)]
pub struct RailVehicle {
    vehicle_type: VehicleType,
    mass_kg: f64,
    length_m: f64,
    max_braking_force_n: f64,
    cargo_mass_kg: f64,
    power_w: f64,
    max_tractive_effort_n: f64,
}

impl RailVehicle {
    pub fn new_car(mass_kg: f64, length_m: f64, cargo_mass_kg: f64) -> RailVehicle {
        RailVehicle {
            vehicle_type: VehicleType::RailCar,
            mass_kg,
            length_m,
            cargo_mass_kg,
            max_braking_force_n: 40.0,
            power_w: 0.0,
            max_tractive_effort_n: 0.0,
        }
    }

    pub fn new_locomotive(mass_kg: f64, length_m: f64, power_kw: f64, max_tractive_effort_kn: f64) -> RailVehicle {
        RailVehicle {
            vehicle_type: VehicleType::Locomotive,
            mass_kg,
            length_m,
            power_w: power_kw * 1000.0,
            max_tractive_effort_n: max_tractive_effort_kn * 1000.0,
            max_braking_force_n: 150.0,
            cargo_mass_kg: 0.0,
        }
    }

    pub fn get_tractive_effort(&self, speed_mps: f64, throttle: f64) -> f64 {
        match self.vehicle_type {
            VehicleType::Locomotive => {
                let max_tractive_effort_n = self.max_tractive_effort_n * throttle;
                if speed_mps < 0.01 {
                    max_tractive_effort_n
                } else {
                    let power_w = self.power_w * throttle;
                    let tractive_effort = power_w / speed_mps;
                    f64::min(tractive_effort, max_tractive_effort_n)
                }
            }
            VehicleType::RailCar => 0.0,
        }
    }
}

struct TrainStats {
    length_m: f64,
    mass_kg: f64,
    max_braking_force_n: f64,
}

fn get_train_stats<'a, I: IntoIterator<Item = &'a RailVehicle>>(vehicles: I) -> TrainStats {
    let result = vehicles.into_iter().fold((0.0, 0.0, 0.0), |acc, vehicle| {
        (
            acc.0 + vehicle.length_m,
            acc.1 + vehicle.mass_kg + vehicle.cargo_mass_kg,
            acc.2 + vehicle.max_braking_force_n,
        )
    });
    TrainStats {
        length_m: result.0,
        mass_kg: result.1,
        max_braking_force_n: result.2,
    }
}

pub struct TrainSpawnState {
    pub speed_mps: f64,
    pub direction: Direction,
    pub spawn_point: TrackPoint,
}

pub struct TrainStatusUpdate {
    pub id: TrainID,
    pub speed_mps: f64,
    pub target_speed_mps: f64,
    pub next_block_m: f64,
    pub control_percentage: i32,
}

pub struct Train {
    pub id: TrainID,

    controls: TrainControls,
    speed_mps: f64,
    target_speed_mps: f64,
    acceleration_mps2: f64,

    direction: Direction,
    vehicles: Vec<RailVehicle>,
    stats: TrainStats,

    occupied_blocks: VecDeque<BlockId>,
    front_position: TrackPoint,
    back_position: TrackPoint,
    target_speed_margin_mps: f64,
}

impl Train {
    pub fn spawn_at(
        id: TrainID,
        state: TrainSpawnState,
        block_map: &BlockMap,
        rail_vehicles: Vec<RailVehicle>,
    ) -> Self {
        let stats = get_train_stats(&rail_vehicles);
        let mut trace: Vec<TrackPoint> = state
            .spawn_point
            .walk(stats.length_m.max(1.0), state.direction.reverse(), block_map)
            .collect();

        Train {
            id,
            controls: Default::default(),
            speed_mps: state.speed_mps,
            target_speed_mps: 20.0,
            acceleration_mps2: 0.0,
            direction: state.direction,
            vehicles: rail_vehicles,
            stats,
            occupied_blocks: trace.iter().map(|x| x.block_id).collect(),
            front_position: state.spawn_point,
            back_position: trace.pop().unwrap(),
            target_speed_margin_mps: 0.0,
        }
    }

    /// Simple throttle and brake controls based on difference between current and target speed.
    /// Returns `TrainControls` with values between 0.0 and 1.0.
    fn calculate_controls(&self) -> TrainControls {
        let speed_diff_mps = (self.target_speed_mps - self.target_speed_margin_mps) - self.speed_mps;
        if self.speed_mps < 0.001 && self.target_speed_mps < 0.01 {
            return TrainControls {
                throttle: 0.0,
                brake_level: 1.0, // Full brake when target is effectively zero
            };
        }

        let hysteresis = 0.01;
        if speed_diff_mps < hysteresis {
            // Calculate brake level - more braking for bigger negative difference
            let brake_level = speed_diff_mps.abs() / 5.0;
            return TrainControls {
                throttle: 0.0,
                brake_level: brake_level.clamp(0.0, 1.0),
            };
        }

        if speed_diff_mps > hysteresis {
            return TrainControls {
                throttle: 1.0,
                brake_level: 0.0,
            };
        }
        TrainControls::default()
    }

    pub fn update(&mut self, dt: f64, map: &BlockMap) -> TrainStatusUpdate {
        if dt <= 0.0 {
            return TrainStatusUpdate {
                id: self.id,
                speed_mps: self.speed_mps,
                target_speed_mps: self.target_speed_mps,
                next_block_m: 0.0,
                control_percentage: self.controls.as_percentage(),
            };
        }

        // Calculate tractive effort and braking force
        self.controls = self.calculate_controls();
        let tractive_effort = self
            .vehicles
            .iter()
            .map(|x| x.get_tractive_effort(self.speed_mps, self.controls.throttle))
            .sum::<f64>();
        let braking_force = self.stats.max_braking_force_n * self.controls.brake_level;
        let net_force_n = tractive_effort - braking_force;

        self.acceleration_mps2 = if self.stats.mass_kg > 0.0 {
            net_force_n / self.stats.mass_kg
        } else {
            0.0
        };
        self.speed_mps = self.speed_mps + self.acceleration_mps2 * dt;

        if self.speed_mps < 0.1 && self.target_speed_mps < 0.25 {
            self.speed_mps = 0.0; // brake to full stop
            self.acceleration_mps2 = 0.0;
        }

        let dx = self.speed_mps * dt + 0.5 * self.acceleration_mps2 * dt.powi(2);
        if dx > 0.0 {
            let new_front = self.front_position.step_by(dx, self.direction, map);
            if self.front_position != new_front {
                self.occupied_blocks.push_front(new_front.block_id);
            }
            let new_back = self
                .front_position
                .step_by(self.stats.length_m, self.direction.reverse(), map);
            if self.back_position != new_back {
                self.occupied_blocks.pop_back();
            }
            self.front_position = new_front;
            self.back_position = new_back;
        }

        TrainStatusUpdate {
            id: self.id,
            speed_mps: self.speed_mps,
            target_speed_mps: self.target_speed_mps,
            next_block_m: map.get_available_length(&self.front_position, self.direction),
            control_percentage: self.controls.as_percentage(),
        }
    }
}
