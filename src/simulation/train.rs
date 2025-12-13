use crate::assets::LoadingState;
use crate::common::{Direction, TrainId};
use crate::simulation::block::{BlockMap, TrackPoint};
use crate::simulation::messages::BlockUpdate;
use crate::simulation::signal::SpeedLimit;
use bevy::prelude::*;

#[derive(Default)]
struct TrainControls {
    throttle: f64,
    brake_level: f64,
}

impl TrainControls {
    fn as_percentage(&self) -> i32 {
        if self.throttle != 0.0 {
            (self.throttle * 100.0) as i32
        } else {
            -(self.brake_level * 100.0) as i32
        }
    }
}

#[derive(Copy, Clone)]
enum VehicleType {
    Locomotive,
    RailCar,
}

#[derive(Copy, Clone)]
struct RailVehicle {
    vehicle_type: VehicleType,
    mass_kg: f64,
    length_m: f64,
    max_braking_force_n: f64,
    cargo_mass_kg: f64,
    power_w: f64,
    max_tractive_effort_n: f64,
}

impl RailVehicle {
    fn new_car(mass_kg: f64, length_m: f64, cargo_mass_kg: f64) -> RailVehicle {
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

    fn new_locomotive(mass_kg: f64, length_m: f64, power_kw: f64, max_tractive_effort_kn: f64) -> RailVehicle {
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

    fn get_tractive_effort(&self, speed_mps: f64, throttle: f64) -> f64 {
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

#[derive(Default)]
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

#[derive(Resource, Default)]
struct NextTrainId(TrainId);

impl NextTrainId {
    fn next(&mut self) -> TrainId {
        self.0 = self.0.wrapping_add(1);
        self.0
    }
}

#[derive(Component, Default)]
pub struct Train {
    pub id: TrainId,
    pub number: String,

    controls: TrainControls,
    speed_mps: f64,
    target_speed_mps: f64,
    target_speed_margin_mps: f64,

    direction: Direction,
    vehicles: Vec<RailVehicle>,
    stats: TrainStats,

    front_position: TrackPoint,
    back_position: TrackPoint,
}

impl Train {
    fn set_target_speed_mps(&mut self, speed_mps: f64) {
        self.target_speed_margin_mps = rand::random::<f64>() * 0.5 + 0.35;
        let speed_kmh = speed_mps * 3.6;
        info!("Train {} setting target speed to {:.2} km/h", self.number, speed_kmh);
        self.target_speed_mps = speed_mps;
    }

    pub fn get_speed_kmh(&self) -> f64 {
        self.speed_mps * 3.6
    }

    pub fn get_target_speed_kmh(&self) -> f64 {
        self.target_speed_mps * 3.6
    }

    /// Simple throttle and brake controls based on the difference between current and target speed.
    /// Returns `TrainControls` with values between 0.0 and 1.0.
    fn calculate_controls(&self) -> TrainControls {
        let speed_diff_mps = (self.target_speed_mps - self.target_speed_margin_mps) - self.speed_mps;
        if self.speed_mps < 0.001 && self.target_speed_mps < 0.01 {
            return TrainControls {
                throttle: 0.0,
                brake_level: 1.0, // Full brake when the target is effectively zero
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

    fn get_braking_distance(&self, speed_limit: SpeedLimit) -> Option<f64> {
        let target_speed_mps = match speed_limit {
            SpeedLimit::Unrestricted => return None,
            SpeedLimit::Restricted(speed_limit_kmh) => speed_limit_kmh / 3.6,
        };

        let braking_force = self.stats.max_braking_force_n * 0.8;
        let deceleration_mps2 = braking_force / self.stats.mass_kg;

        let speed_diff_mps = self.speed_mps - target_speed_mps;
        let speed_sum = self.speed_mps + target_speed_mps;
        Some(0.0f64.max((speed_diff_mps * speed_sum) / (2.0 * deceleration_mps2)))
    }

    fn update(&mut self, dt: f64, map: &BlockMap, block_updates: &mut MessageWriter<BlockUpdate>) {
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

        let mut acceleration_mps2 = if self.stats.mass_kg > 0.0 {
            net_force_n / self.stats.mass_kg
        } else {
            0.0
        };
        self.speed_mps += acceleration_mps2 * dt;

        if self.speed_mps < 0.1 && self.target_speed_mps < 0.25 {
            if self.speed_mps >= 0.0 {
                info!("Train {} stopped at {}", self.number, self.front_position);
            }
            self.speed_mps = 0.0; // brake to full stop
            acceleration_mps2 = 0.0;
        }

        let dx = self.speed_mps * dt + 0.5 * acceleration_mps2 * dt.powi(2);
        let (signal, distance_m) = map.lookup_signal_forward(&self.front_position, self.direction);
        let speed_control = &signal.speed_ctrl;
        let braking_distance = self.get_braking_distance(speed_control.passing_kmh);
        let speed_limit = match braking_distance {
            None => speed_control.approaching_kmh,
            Some(braking_distance_m) => {
                let approaching_mps = speed_control.approaching_kmh.to_mps(80.0);
                if distance_m > braking_distance_m && self.target_speed_mps >= approaching_mps {
                    speed_control.approaching_kmh
                } else {
                    speed_control.passing_kmh
                }
            }
        };
        let target_speed_mps = speed_limit.to_mps(80.0);
        if self.target_speed_mps != target_speed_mps {
            self.set_target_speed_mps(target_speed_mps);
        }

        if distance_m < dx {
            info!(
                "Train {} passed signal {} at {:.2} km/h, allowed speed {}",
                self.number,
                signal.name,
                self.get_speed_kmh(),
                speed_control.passing_kmh,
            );
        }

        if dx > 0.0 {
            let new_front = map.step_by(&self.front_position, dx, self.direction);
            if self.front_position.block_id != new_front.block_id {
                block_updates.write(BlockUpdate::occupied(new_front.block_id, self.id));
            }
            let new_back = map.step_by(&self.front_position, self.stats.length_m, self.direction.reverse());
            if self.back_position.block_id != new_back.block_id {
                block_updates.write(BlockUpdate::freed(self.back_position.block_id, self.id));
            }
            self.front_position = new_front;
            self.back_position = new_back;
        }
    }
}

pub struct TrainPlugin;

impl Plugin for TrainPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<NextTrainId>()
            .add_systems(Update, keyboard_handling.run_if(in_state(LoadingState::Loaded)))
            .add_systems(FixedUpdate, update.run_if(in_state(LoadingState::Loaded)));
    }
}

fn keyboard_handling(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut block_map: ResMut<BlockMap>,
    query: Query<(Entity, &mut Train)>,
    mut block_updates: MessageWriter<BlockUpdate>,
    mut train_id: ResMut<NextTrainId>,
    mut commands: Commands,
) {
    if keyboard_input.just_pressed(KeyCode::KeyG) {
        let train = spawn_train(train_id.next(), &block_map, &mut block_updates);
        info!("Train {} spawned with ID {}", train.number, train.id);
        commands.spawn(train);
    }
    if keyboard_input.just_pressed(KeyCode::KeyH) {
        if let Some((entity, train)) = query.iter().min_by_key(|(_, t)| t.id) {
            info!("Train {} despawned with ID {}", train.number, train.id);
            block_map.despawn_train(train.id, &mut block_updates);
            commands.entity(entity).despawn();
        }
    }
}

fn update(
    time: Res<Time>,
    block_map: Res<BlockMap>,
    mut query: Query<&mut Train>,
    mut block_updates: MessageWriter<BlockUpdate>,
) {
    query.iter_mut().for_each(|mut train| {
        train.update(time.delta_secs_f64(), &block_map, &mut block_updates);
    });
}

fn spawn_train(train_id: TrainId, block_map: &BlockMap, block_updates: &mut MessageWriter<BlockUpdate>) -> Train {
    let mut cars: Vec<RailVehicle> = Vec::with_capacity(100);
    cars.extend([RailVehicle::new_locomotive(138_000.0, 18.15, 2250.0, 375.0); 2]);
    cars.extend([RailVehicle::new_car(30_000.0, 15.0, 70_000.0); 60]);

    let spawn_pos = TrackPoint {
        block_id: 2,
        offset_m: 600.0,
    };
    let direction = Direction::Even;
    let stats = get_train_stats(&cars);
    let trace: Vec<TrackPoint> = block_map
        .walk(&spawn_pos, stats.length_m.max(1.0), direction.reverse())
        .collect();

    block_updates.write_batch(
        trace
            .iter()
            .map(|point| BlockUpdate::occupied(point.block_id, train_id)),
    );

    Train {
        id: train_id,
        number: rand::random_range(1000..=9999).to_string(),
        direction,
        stats,
        vehicles: cars,
        front_position: spawn_pos,
        back_position: trace.last().cloned().unwrap(),
        ..default()
    }
}
