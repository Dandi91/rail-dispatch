use crate::block::{BlockMap, TrackPoint};
use crate::clock::Clock;
use crate::common::Direction;
use crate::level::Level;
use crate::train::{RailVehicle, Train, TrainPriority};
use atomic_float::AtomicF64;
use std::iter::zip;
use std::ops::Sub;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::thread;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

const MULTIPLIERS: [f64; 7] = [0.1, 0.5, 1.0, 2.0, 5.0, 10.0, 20.0];
const UNIT_DT: f64 = 0.01;

struct SimulationState {
    clock: Clock,
    block_map: BlockMap,
    trains: Vec<Train>,
}

struct ControlState {
    done: AtomicBool,
    time_scale: AtomicF64,
    sim_duration: AtomicF64,
}

impl SimulationState {
    pub fn simulate(state: Arc<RwLock<SimulationState>>, control: Arc<ControlState>) {
        let mut last_wake = Instant::now();
        while !control.done.load(Ordering::Relaxed) {
            // compute simulation duration since last wake
            let sim_duration = Instant::now().sub(last_wake);
            let sim_duration_f64 = sim_duration.as_secs_f64();
            control
                .sim_duration
                .store(sim_duration_f64, Ordering::Relaxed);

            // compute necessary dt to sleep
            let time_scale = control.time_scale.load(Ordering::Relaxed);
            let dt = Duration::from_secs_f64(UNIT_DT / time_scale);
            thread::sleep(dt.saturating_sub(sim_duration));

            // compute actual dt that passed
            let this_wake = Instant::now();
            let actual_dt = this_wake - last_wake;
            let sim_dt = actual_dt.as_secs_f64() * time_scale;
            last_wake = this_wake;

            // run simulation based on the actual dt
            {
                let mut state = state.write().unwrap();
                state.clock.tick(sim_dt);

                let mut updates = Vec::with_capacity(state.trains.len());
                for train in &state.trains {
                    updates.push(train.calculate_update(sim_dt, &state.block_map));
                }
                for (train, update) in zip(&mut state.trains, updates) {
                    if let Some(update) = update {
                        train.apply_update(update);
                    }
                }
            }
        }
    }

    pub fn spawn_train(
        &mut self,
        priority: TrainPriority,
        number: String,
        direction: Direction,
        speed_mps: f64,
        spawn_point: TrackPoint,
    ) -> &Train {
        let mut cars: Vec<RailVehicle> = Vec::with_capacity(100);
        cars.extend([RailVehicle::new_locomotive(138_000.0, 18.15, 2250.0, 375.0); 2]);
        cars.extend([RailVehicle::new_car(30_000.0, 15.0, 70_000.0); 75]);

        self.trains.push(Train::spawn_at(
            priority,
            number,
            speed_mps,
            direction,
            spawn_point,
            &self.block_map,
            cars,
        ));
        self.trains.last().expect("we just put train in there")
    }
}

pub struct Engine {
    multiplier_index: usize,
    control: Arc<ControlState>,
    state: Arc<RwLock<SimulationState>>,
    thread: Option<JoinHandle<()>>,
}

impl Engine {
    pub fn new(level: &Level) -> Self {
        let default_multiplier = 2; // 1.0
        Engine {
            multiplier_index: default_multiplier,
            control: Arc::new(ControlState {
                done: AtomicBool::new(false),
                time_scale: AtomicF64::new(MULTIPLIERS[default_multiplier]),
                sim_duration: AtomicF64::default(),
            }),
            state: Arc::new(RwLock::new(SimulationState {
                clock: Clock::new(None),
                block_map: BlockMap::from_level(&level),
                trains: Vec::new(),
            })),
            thread: None,
        }
    }

    pub fn increase_simulation_speed(&mut self) {
        if self.multiplier_index < MULTIPLIERS.len() - 1 {
            self.multiplier_index += 1;
            let multiplier = MULTIPLIERS[self.multiplier_index];
            self.control.time_scale.store(multiplier, Ordering::Relaxed);
        }
    }

    pub fn decrease_simulation_speed(&mut self) {
        if self.multiplier_index > 0 {
            self.multiplier_index -= 1;
            let multiplier = MULTIPLIERS[self.multiplier_index];
            self.control.time_scale.store(multiplier, Ordering::Relaxed);
        }
    }

    pub fn add_train(&mut self) -> String {
        let mut state = self.state.write().unwrap();
        let train_number = rand::random_range(1000..=9999).to_string();
        let spawn_point = state.block_map.get_track_point(2, 600.0);
        state.spawn_train(
            TrainPriority::Cargo,
            train_number.clone(),
            Direction::Even,
            0.0,
            spawn_point,
        );
        train_number
    }

    pub fn remove_last_train(&mut self) -> Option<Train> {
        self.state.write().unwrap().trains.pop()
    }

    pub fn time_scale_formatted(&self) -> String {
        let time_scale = self.control.time_scale.load(Ordering::Relaxed);
        if time_scale >= 1.0 {
            format!("{}x", time_scale as u32)
        } else {
            format!("{:.1}x", time_scale)
        }
    }

    pub fn sim_duration_formatted(&self) -> String {
        let sim_duration = self.control.sim_duration.load(Ordering::Relaxed);
        format!("{:5} us", (sim_duration * 1_000_000.0) as u32)
    }

    pub fn start(&mut self) {
        if self.thread.is_none() {
            let state = self.state.clone();
            let control = self.control.clone();
            self.thread = Some(
                thread::Builder::new()
                    .name("SimThread".into())
                    .spawn(move || SimulationState::simulate(state, control))
                    .unwrap(),
            );
        }
    }

    pub fn stop(&mut self) {
        if let Some(thread) = self.thread.take() {
            self.control.done.store(true, Ordering::Relaxed);
            thread.join().unwrap();
        }
    }
}
