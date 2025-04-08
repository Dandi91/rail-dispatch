use crate::common::SimObject;
use crate::train::Train;
use std::ops::Sub;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::thread::{sleep, spawn, JoinHandle};
use std::time::{Duration, Instant};
use crate::clock::Clock;

const MULTIPLIERS: [f64; 7] = [0.1, 0.5, 1.0, 2.0, 5.0, 10.0, 20.0];

struct EngineState {
    clock: Clock,
    trains: Vec<Train>,
    unit_dt: f64,
    time_scale: f64,
    sim_duration: f64,
}

impl EngineState {
    fn dt(&self) -> Duration {
        Duration::from_secs_f64(self.unit_dt / self.time_scale)
    }

    pub fn simulate(state: Arc<RwLock<EngineState>>, done: Arc<AtomicBool>) {
        let mut last_wake = Instant::now();
        while !done.load(Ordering::Relaxed) {
            let sim_duration = Instant::now().sub(last_wake);
            let dt = state.read().unwrap().dt();
            sleep(dt.saturating_sub(sim_duration));
            let this_wake = Instant::now();
            let actual_dt = this_wake - last_wake;
            last_wake = this_wake;
            {
                let mut state = state.write().unwrap();
                let sim_dt = actual_dt.as_secs_f64() * state.time_scale;
                state.sim_duration = sim_duration.as_secs_f64();

                state.clock.tick(sim_dt);
                for train in &mut state.trains {
                    train.tick(sim_dt);
                }
            }
        }
    }
}

pub struct Engine {
    multiplier: usize,
    state: Arc<RwLock<EngineState>>,
    done: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Engine {
    pub fn new() -> Self {
        let default_multiplier = 2; // 1.0
        Engine {
            multiplier: default_multiplier,
            state: Arc::new(RwLock::new(EngineState {
                clock: Clock::new(None),
                trains: Vec::new(),
                unit_dt: 0.01,
                time_scale: MULTIPLIERS[default_multiplier],
                sim_duration: 0.0,
            })),
            done: Arc::new(AtomicBool::new(false)),
            thread: None,
        }
    }

    pub fn increase_simulation_speed(&mut self) {
        if self.multiplier < MULTIPLIERS.len() - 1 {
            self.multiplier += 1;
            self.state.write().unwrap().time_scale = MULTIPLIERS[self.multiplier];
        }
    }

    pub fn decrease_simulation_speed(&mut self) {
        if self.multiplier > 0 {
            self.multiplier -= 1;
            self.state.write().unwrap().time_scale = MULTIPLIERS[self.multiplier];
        }
    }

    pub fn add_train(&mut self, train: Train) {
        self.state.write().unwrap().trains.push(train);
    }

    pub fn remove_last_train(&mut self) -> Option<Train> {
        self.state.write().unwrap().trains.pop()
    }

    pub fn time_scale_formatted(&self) -> String {
        let time_scale = self.state.read().unwrap().time_scale;
        if time_scale >= 1.0 {
            format!("{}x", time_scale as u32)
        } else {
            format!("{:.1}x", time_scale)
        }
    }

    pub fn sim_duration_formatted(&self) -> String {
        let sim_duration = self.state.read().unwrap().sim_duration;
        format!("{} ms", (sim_duration * 1000.0) as u32)
    }

    pub fn start(&mut self) {
        if self.thread.is_none() {
            let state = self.state.clone();
            let done = self.done.clone();
            self.thread = Some(spawn(move || EngineState::simulate(state, done)));
        }
    }

    pub fn stop(&mut self) {
        if let Some(thread) = self.thread.take() {
            self.done.store(true, Ordering::Relaxed);
            thread.join().unwrap();
        }
    }
}
