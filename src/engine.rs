use crate::block::{BlockMap, TrackPoint};
use crate::clock::Clock;
use crate::common::Direction;
use crate::event::{Command, SimulationUpdate};
use crate::level::Level;
use crate::train::{RailVehicle, Train, TrainPriority, TrainSpawnState};
use atomic_float::AtomicF64;
use std::iter::zip;
use std::ops::Sub;
use std::sync::atomic::Ordering;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{mpsc, Arc};
use std::thread;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

const MULTIPLIERS: [f64; 7] = [0.1, 0.5, 1.0, 2.0, 5.0, 10.0, 20.0];
const DEFAULT_MULTIPLIER_INDEX: usize = 2;
const UNIT_DT: f64 = 0.01;

struct SimulationState {
    sender: Sender<SimulationUpdate>,
    receiver: Receiver<Command>,
    clock: Clock,
    block_map: BlockMap,
    trains: Vec<Train>,
}

struct ControlState {
    time_scale: AtomicF64,
    sim_duration: AtomicF64,
}

impl SimulationState {
    fn new(init: ThreadInitState) -> Self {
        SimulationState {
            sender: init.sender,
            receiver: init.receiver,
            clock: Clock::new(None),
            block_map: init.block_map,
            trains: Vec::new(),
        }
    }

    fn consume_events(&mut self) -> bool {
        match self.receiver.try_recv() {
            Ok(event) => {
                match event {
                    Command::TrainSpawn(state) => self.spawn_train(*state),
                    Command::TrainDespawn => self.despawn_last_train(),
                    Command::Shutdown => return false,
                }
                true
            }
            Err(err) => match err {
                mpsc::TryRecvError::Empty => true,
                mpsc::TryRecvError::Disconnected => false,
            },
        }
    }

    fn despawn_last_train(&mut self) {
        self.trains.pop();
    }

    fn simulate(&mut self, control: Arc<ControlState>) {
        let mut last_wake = Instant::now();
        while self.consume_events() {
            // compute simulation duration since last wake
            let sim_duration = Instant::now().sub(last_wake);
            control
                .sim_duration
                .store(sim_duration.as_secs_f64(), Ordering::Relaxed);

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
            self.clock.tick(sim_dt);
            let mut updates = Vec::with_capacity(self.trains.len());
            for train in &self.trains {
                updates.push(train.calculate_update(sim_dt, &self.block_map));
            }
            for (train, update) in zip(&mut self.trains, updates) {
                if let Some(update) = update {
                    train.apply_update(update);
                }
            }
        }
        println!("Shutting down simulation");
    }

    pub fn spawn_train(&mut self, spawn_state: TrainSpawnState) {
        let mut cars: Vec<RailVehicle> = Vec::with_capacity(100);
        cars.extend([RailVehicle::new_locomotive(138_000.0, 18.15, 2250.0, 375.0); 2]);
        cars.extend([RailVehicle::new_car(30_000.0, 15.0, 70_000.0); 75]);

        let train = Train::spawn_at(spawn_state, &self.block_map, cars);
        self.trains.push(train);
    }
}

struct ThreadInitState {
    block_map: BlockMap,
    sender: Sender<SimulationUpdate>,
    receiver: Receiver<Command>,
}

pub struct Engine {
    multiplier_index: usize,
    sender: Sender<Command>,
    receiver: Receiver<SimulationUpdate>,
    control: Arc<ControlState>,
    thread_init_state: Option<ThreadInitState>,
    thread: Option<JoinHandle<()>>,
}

impl Engine {
    pub fn new(level: &Level) -> Self {
        let (cmd_tx, cmd_rx) = mpsc::channel();
        let (sim_tx, sim_rx) = mpsc::channel();
        Engine {
            multiplier_index: DEFAULT_MULTIPLIER_INDEX,
            sender: cmd_tx,
            receiver: sim_rx,
            control: Arc::new(ControlState {
                time_scale: AtomicF64::new(MULTIPLIERS[DEFAULT_MULTIPLIER_INDEX]),
                sim_duration: AtomicF64::default(),
            }),
            thread_init_state: Some(ThreadInitState {
                block_map: BlockMap::from_level(&level),
                receiver: cmd_rx,
                sender: sim_tx,
            }),
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
        let spawn_point = TrackPoint {
            block_id: 2,
            offset_m: 600.0,
        };
        let train_number = rand::random_range(1000..=9999).to_string();

        let event = Command::TrainSpawn(Box::new(TrainSpawnState {
            priority: TrainPriority::Cargo,
            number: train_number.clone(),
            direction: Direction::Even,
            speed_mps: 0.0,
            spawn_point,
        }));
        self.sender.send(event).unwrap();
        train_number
    }

    pub fn remove_last_train(&mut self) {
        self.sender.send(Command::TrainDespawn).unwrap()
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
            let control = self.control.clone();
            let init = self.thread_init_state.take().unwrap();
            self.thread = Some(
                thread::Builder::new()
                    .name("SimThread".into())
                    .spawn(move || SimulationState::new(init).simulate(control))
                    .unwrap(),
            );
        }
    }

    pub fn stop(&mut self) {
        if let Some(thread) = self.thread.take() {
            self.sender.send(Command::Shutdown).unwrap();
            thread.join().unwrap();
        }
    }
}
