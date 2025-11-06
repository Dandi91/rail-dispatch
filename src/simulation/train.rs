use crate::common::{Direction, TrainId};
use crate::display::train::TrainKind;
use crate::simulation::block::{BlockId, BlockMap, TrackPoint};
use crate::simulation::updates::{BlockUpdateQueue, UpdateQueues};
use bevy::prelude::*;
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
            max_braking_force_n: 10_000.0,
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
            max_braking_force_n: 50_000.0,
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
    pub number: String,
    pub kind: TrainKind,
    pub speed_mps: f64,
    pub direction: Direction,
    pub spawn_point: TrackPoint,
}

pub struct TrainStatusUpdate {
    pub id: TrainId,
    pub speed_mps: f64,
    pub target_speed_mps: f64,
    pub next_block_m: f64,
    pub control_percentage: i32,
    pub braking_distance_m: f64,
    pub signal_distance_m: f64,
}

#[derive(Resource)]
pub struct NextTrainId(TrainId);

impl NextTrainId {
    pub fn new() -> Self {
        NextTrainId(1)
    }

    pub fn next(&mut self) -> TrainId {
        let result = self.0;
        self.0 += 1;
        result
    }
}

#[derive(Component)]
pub struct Train {
    pub id: TrainId,
    pub number: String,
    kind: TrainKind,

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

    braking_distance_m: f64,
    signal_distance_m: f64,
    target_speed_margin_mps: f64,
    position_updated: bool,
}

impl Train {
    pub fn spawn_at(
        id: TrainId,
        state: TrainSpawnState,
        rail_vehicles: Vec<RailVehicle>,
        block_map: &BlockMap,
        block_updates: &mut BlockUpdateQueue,
    ) -> Self {
        let stats = get_train_stats(&rail_vehicles);
        let mut trace: Vec<TrackPoint> = block_map
            .walk(&state.spawn_point, stats.length_m.max(1.0), state.direction.reverse())
            .collect();
        let occupied: VecDeque<_> = trace.iter().map(|x| x.block_id).collect();
        occupied
            .iter()
            .cloned()
            .for_each(|block_id| block_updates.occupied(block_id, id));

        Train {
            id,
            number: state.number,
            kind: state.kind,
            controls: Default::default(),
            speed_mps: state.speed_mps,
            target_speed_mps: 0.0,
            acceleration_mps2: 0.0,
            direction: state.direction,
            vehicles: rail_vehicles,
            stats,
            occupied_blocks: occupied,
            front_position: state.spawn_point,
            back_position: trace.pop().unwrap(),
            signal_distance_m: 0.0,
            braking_distance_m: 0.0,
            target_speed_margin_mps: 0.0,
            position_updated: true,
        }
    }

    pub fn despawn(&self, block_updates: &mut BlockUpdateQueue) {
        self.occupied_blocks
            .iter()
            .for_each(|&block_id| block_updates.freed(block_id, self.id))
    }

    pub fn set_target_speed_kmh(&mut self, speed_kmh: f64) {
        self.target_speed_margin_mps = rand::random::<f64>() * 0.5 + 0.35;
        self.target_speed_mps = speed_kmh / 3.6
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
            let brake_level = speed_diff_mps.abs() / 2.0;
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

    fn get_braking_distance(&self, target_speed_mps: f64) -> f64 {
        if target_speed_mps > self.speed_mps {
            return 0.0;
        }

        let braking_force = self.stats.max_braking_force_n * 0.8;
        let deceleration_mps2 = braking_force / self.stats.mass_kg;

        let speed_diff_mps = self.speed_mps - target_speed_mps;
        let speed_sum = self.speed_mps + target_speed_mps;
        0.0f64.max((speed_diff_mps * speed_sum) / (2.0 * deceleration_mps2))
    }

    pub fn update(&mut self, dt: f64, map: &BlockMap, block_updates: &mut BlockUpdateQueue) {
        if dt <= 0.0 {
            return;
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
        self.speed_mps += self.acceleration_mps2 * dt;

        if self.speed_mps < 0.1 && self.target_speed_mps < 0.25 {
            self.speed_mps = 0.0; // brake to full stop
            self.acceleration_mps2 = 0.0;
        }

        let dx = self.speed_mps * dt + 0.5 * self.acceleration_mps2 * dt.powi(2);
        if dx > 0.0 {
            let (signal, distance_m) = map.lookup_signal(&self.front_position, self.direction);
            let allowed_speed_mps = signal.get_allowed_speed_mps();
            self.braking_distance_m = self.get_braking_distance(allowed_speed_mps);
            self.signal_distance_m = distance_m;
            if distance_m < self.braking_distance_m {
                self.target_speed_mps = allowed_speed_mps;
            }

            if distance_m < dx {
                println!(
                    "Passed signal {} at {:.2} km/h, allowed speed {:.2} km/h",
                    signal.get_name(),
                    self.speed_mps * 3.6,
                    allowed_speed_mps * 3.6
                );
            }

            let new_front = map.step_by(&self.front_position, dx, self.direction);
            if self.front_position.block_id != new_front.block_id {
                block_updates.occupied(new_front.block_id, self.id);
                self.occupied_blocks.push_front(new_front.block_id);
            }
            let new_back = map.step_by(&self.front_position, self.stats.length_m, self.direction.reverse());
            if self.back_position.block_id != new_back.block_id {
                let freed = self.occupied_blocks.pop_back().unwrap();
                block_updates.freed(freed, self.id);
            }
            self.front_position = new_front;
            self.back_position = new_back;
            self.position_updated = true;
        }
    }

    pub fn get_state_update(&mut self, map: &BlockMap) -> Option<TrainStatusUpdate> {
        if self.position_updated {
            self.position_updated = false;
            Some(TrainStatusUpdate {
                id: self.id,
                speed_mps: self.speed_mps,
                target_speed_mps: self.target_speed_mps,
                next_block_m: map.get_available_length(&self.front_position, self.direction),
                control_percentage: self.controls.as_percentage(),
                braking_distance_m: self.braking_distance_m,
                signal_distance_m: self.signal_distance_m,
            })
        } else {
            None
        }
    }
}

pub fn spawn_train(train_id: TrainId, block_map: &BlockMap, updates: &mut UpdateQueues) -> Train {
    let spawn_state = TrainSpawnState {
        number: rand::random_range(1000..=9999).to_string(),
        kind: TrainKind::Cargo,
        direction: Direction::Even,
        speed_mps: 0.0,
        spawn_point: TrackPoint {
            block_id: 2,
            offset_m: 600.0,
        },
    };

    let mut cars: Vec<RailVehicle> = Vec::with_capacity(100);
    cars.extend([RailVehicle::new_locomotive(138_000.0, 18.15, 2250.0, 375.0); 2]);
    cars.extend([RailVehicle::new_car(30_000.0, 15.0, 70_000.0); 60]);

    let mut train = Train::spawn_at(train_id, spawn_state, cars, block_map, &mut updates.block_updates);
    train.set_target_speed_kmh(80.0);
    train
}
