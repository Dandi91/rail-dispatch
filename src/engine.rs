use crate::common::SimObject;
use chrono::{Local, NaiveDateTime, TimeDelta};
use std::ops::Add;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{spawn, JoinHandle};

pub struct Clock {
    start_point: NaiveDateTime,
    elapsed_seconds: f64,
}

impl SimObject for Clock {
    fn tick(&mut self, dt: f64) {
        self.elapsed_seconds += dt;
    }
}

impl Clock {
    pub fn new(start_point: Option<NaiveDateTime>) -> Self {
        Clock {
            start_point: start_point.unwrap_or(Local::now().naive_local()),
            elapsed_seconds: 0.0,
        }
    }

    pub fn current(&self) -> NaiveDateTime {
        let delta = TimeDelta::microseconds((self.elapsed_seconds * 1_000_000.0) as i64);
        self.start_point.add(delta)
    }

    pub fn datetime_to_elapsed_seconds(&self, dt: NaiveDateTime) -> f64 {
        (dt - self.start_point).num_microseconds().unwrap() as f64 / 1_000_000.0
    }
}

const MULTIPLIERS: [f64; 7] = [0.1, 0.5, 1.0, 2.0, 5.0, 10.0, 20.0];

struct EngineState {
    sim_objects: Vec<Arc<dyn SimObject>>,
    unit_dt: f64,
    time_scale: f64,
    sim_duration: f64,
}

impl EngineState {
    fn dt(&self) -> f64 {
        self.unit_dt / self.time_scale
    }

    pub fn simulate(state: Arc<Mutex<EngineState>>) {}
}

pub struct Engine {
    clock: Arc<Clock>,
    multiplier: usize,
    state: Arc<Mutex<EngineState>>,
    done: AtomicBool,
    thread: Option<JoinHandle<()>>,
}

impl Engine {
    pub fn new() -> Self {
        let default_multiplier = 2; // 1.0
        let clock = Arc::new(Clock::new(None));
        Engine {
            clock: clock.clone(),
            multiplier: default_multiplier,
            state: Arc::new(Mutex::new(EngineState {
                sim_objects: vec![clock.clone()],
                unit_dt: 0.01,
                time_scale: MULTIPLIERS[default_multiplier],
                sim_duration: 0.0,
            })),
            done: AtomicBool::new(false),
            thread: None,
        }
    }

    pub fn increase_simulation_speed(&mut self) {
        if self.multiplier < MULTIPLIERS.len() - 1 {
            self.multiplier += 1;
            self.state.lock().unwrap().time_scale = MULTIPLIERS[self.multiplier];
        }
    }

    pub fn decrease_simulation_speed(&mut self) {
        if self.multiplier > 0 {
            self.multiplier -= 1;
            self.state.lock().unwrap().time_scale = MULTIPLIERS[self.multiplier];
        }
    }

    pub fn add_sim_object(&mut self, sim_object: Arc<dyn SimObject>) {
        self.state.lock().unwrap().sim_objects.push(sim_object);
    }

    pub fn remove_sim_object(&mut self, sim_object: Arc<dyn SimObject>) {
        let sim_objects = &mut self.state.lock().unwrap().sim_objects;
        if let Some(index) = sim_objects.iter().position(|x| Arc::ptr_eq(x, &sim_object)) {
            sim_objects.swap_remove(index);
        }
    }

    pub fn time_scale_formatted(&self) -> String {
        let time_scale = self.state.lock().unwrap().time_scale;
        if time_scale >= 1.0 {
            format!("{}x", time_scale as u32)
        } else {
            format!("{:.1}x", time_scale)
        }
    }

    pub fn sim_duration_formatted(&self) -> String {
        let sim_duration = self.state.lock().unwrap().sim_duration;
        format!("{} ms", (sim_duration * 1000.0) as u32)
    }

    pub fn start(&mut self) {
        if self.thread.is_none() {
            let state = self.state.clone();
            self.thread = Some(spawn(move || { EngineState::simulate(state) } ));
        }
    }

    pub fn stop(&mut self) {
        if let Some(thread) = self.thread.take() {
            self.done.store(true, Ordering::Relaxed);
            thread.join().unwrap();
        }
    }
}
