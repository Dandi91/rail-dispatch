use crate::clock::{Clock, ClockEvent};
use crate::common::{Direction, TrainId};
use crate::display::speed_table::KEEP_TAIL_S;
use crate::display::train::{TrainDisplayState, TrainKind};
use crate::event::{Command, SimulationUpdate};
use crate::level::Level;
use crate::simulation::block::{BlockMap, BlockUpdateQueue, TrackPoint};
use crate::simulation::train::{RailVehicle, Train, TrainSpawnState, TrainStatusUpdate};
use atomic_float::AtomicF64;
use chrono::{TimeDelta, Timelike};
use itertools::Itertools;
use std::sync::atomic::Ordering;
use std::sync::mpsc::{Receiver, Sender, TryRecvError};
use std::sync::{mpsc, Arc};
use std::thread;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

const MULTIPLIERS: [f64; 7] = [0.1, 0.5, 1.0, 2.0, 5.0, 10.0, 20.0];
const DEFAULT_MULTIPLIER_INDEX: usize = 2;
const UNIT_DT: f64 = 0.01;
const KEEP_SPEED_TABLE_TAIL: TimeDelta = TimeDelta::seconds(KEEP_TAIL_S as i64);

struct SimulationState {
    next_id: TrainId,
    time_scale: f64,
    sender: Sender<SimulationUpdate>,
    receiver: Receiver<Command>,
    clock: Clock,
    block_map: BlockMap,
    trains: Vec<Train>,
    block_updates: BlockUpdateQueue,
}

impl SimulationState {
    fn setup_events(clock: &mut Clock) {
        let now = clock.current();
        clock.subscribe_periodic_event(ClockEvent::TrainInfoUpdate, 0.1, None);
        clock.subscribe_periodic_event(ClockEvent::ClockUpdate, 1.0, Some(now));

        let quarter_hour_start = now.with_minute(now.minute() / 15 * 15).unwrap();
        let tail_clean = quarter_hour_start + KEEP_SPEED_TABLE_TAIL;
        clock.subscribe_periodic_event(ClockEvent::EveryQuarterHour, 15.0 * 60.0, Some(quarter_hour_start));
        clock.subscribe_periodic_event(ClockEvent::SpeedTableTailClean, 15.0 * 60.0, Some(tail_clean));
    }

    fn new(init: ThreadInitState) -> Self {
        let mut clock = Clock::new(None);
        println!("Clock is set to {}", clock.current());
        Self::setup_events(&mut clock);
        SimulationState {
            next_id: 0,
            time_scale: MULTIPLIERS[DEFAULT_MULTIPLIER_INDEX],
            sender: init.sender,
            receiver: init.receiver,
            clock,
            block_map: init.block_map,
            trains: Vec::new(),
            block_updates: BlockUpdateQueue::with_capacity(8),
        }
    }

    fn send_update(&self, update: SimulationUpdate) {
        self.sender.send(update).unwrap();
    }

    fn consume_events(&mut self) -> bool {
        loop {
            match self.receiver.try_recv() {
                Ok(cmd) => match cmd {
                    Command::SetTimeScale(value) => {
                        println!("Setting time scale to {}", value);
                        self.time_scale = value;
                    }
                    Command::TrainSpawn(state) => self.spawn_train(*state),
                    Command::TrainDespawn(id) => self.despawn_train_by_id(id),
                    Command::Shutdown => return false,
                },
                Err(err) => {
                    return match err {
                        TryRecvError::Empty => true,
                        TryRecvError::Disconnected => false,
                    };
                }
            }
        }
    }

    fn simulate(&mut self, sim_duration: Arc<AtomicF64>) {
        let mut last_wake = Instant::now();
        while self.consume_events() {
            // compute simulation duration since last wake
            let duration = Instant::now().duration_since(last_wake);
            sim_duration.store(duration.as_secs_f64(), Ordering::Relaxed);

            // compute necessary dt to sleep
            let dt = Duration::from_secs_f64(UNIT_DT / self.time_scale);
            thread::sleep(dt.saturating_sub(duration));

            // compute actual dt that passed
            let this_wake = Instant::now();
            let actual_dt = this_wake - last_wake;
            let sim_dt = actual_dt.as_secs_f64() * self.time_scale;
            last_wake = this_wake;

            // run simulation based on the actual dt
            self.trains
                .iter_mut()
                .for_each(|train| train.update(sim_dt, &self.block_map, &mut self.block_updates));

            self.block_map
                .process_updates(&mut self.block_updates)
                .into_iter()
                .for_each(|(block_id, state)| {
                    self.send_update(SimulationUpdate::BlockOccupation(block_id, state));
                });

            self.clock
                .tick(sim_dt)
                .into_iter()
                .for_each(|payload| match payload.event {
                    ClockEvent::TrainInfoUpdate => {
                        let train_updates = self.collect_train_updates();
                        self.send_update(SimulationUpdate::TrainStates(payload.elapsed_time, train_updates));
                    }
                    _ => self.send_update(SimulationUpdate::Clock(payload)),
                });
        }
        println!("Shutting down simulation");
    }

    fn collect_train_updates(&mut self) -> Vec<TrainStatusUpdate> {
        self.trains
            .iter_mut()
            .map(|train| train.get_state_update(&self.block_map))
            .flatten()
            .collect()
    }

    fn spawn_train(&mut self, spawn_state: TrainSpawnState) {
        self.next_id += 1;
        let mut cars: Vec<RailVehicle> = Vec::with_capacity(100);
        cars.extend([RailVehicle::new_locomotive(138_000.0, 18.15, 2250.0, 375.0); 2]);
        cars.extend([RailVehicle::new_car(30_000.0, 15.0, 70_000.0); 75]);

        let direction = spawn_state.direction;
        let mut train = Train::spawn_at(
            self.next_id,
            spawn_state,
            cars,
            &self.block_map,
            &mut self.block_updates,
        );
        train.set_target_speed_kmh(80.0);
        self.trains.push(train);

        let number = rand::random_range(1000..=9999).to_string();
        let update = SimulationUpdate::RegisterTrain(TrainDisplayState {
            id: self.next_id,
            kind: TrainKind::Cargo,
            number,
            direction,
        });
        self.send_update(update);
    }

    fn despawn_train_by_id(&mut self, id: TrainId) {
        if let Some((pos, ..)) = self.trains.iter().find_position(|x| x.id == id) {
            let train = self.trains.swap_remove(pos);
            train.despawn(&mut self.block_updates);
            self.send_update(SimulationUpdate::UnregisterTrain(id));
        }
    }
}

struct ThreadInitState {
    block_map: BlockMap,
    sender: Sender<SimulationUpdate>,
    receiver: Receiver<Command>,
}

pub struct Engine {
    multiplier_index: usize,
    time_scale: f64,
    sender: Sender<Command>,
    receiver: Receiver<SimulationUpdate>,
    sim_duration: Arc<AtomicF64>,
    thread_init_state: Option<ThreadInitState>,
    thread: Option<JoinHandle<()>>,
}

impl Engine {
    pub fn new(level: &Level) -> Self {
        let (cmd_tx, cmd_rx) = mpsc::channel();
        let (sim_tx, sim_rx) = mpsc::channel();
        Engine {
            multiplier_index: DEFAULT_MULTIPLIER_INDEX,
            time_scale: MULTIPLIERS[DEFAULT_MULTIPLIER_INDEX],
            sender: cmd_tx,
            receiver: sim_rx,
            sim_duration: Arc::new(AtomicF64::default()),
            thread_init_state: Some(ThreadInitState {
                block_map: BlockMap::from_level(level),
                receiver: cmd_rx,
                sender: sim_tx,
            }),
            thread: None,
        }
    }

    fn send_command(&self, cmd: Command) {
        self.sender.send(cmd).unwrap();
    }

    pub fn receive_command(&self) -> Result<SimulationUpdate, TryRecvError> {
        self.receiver.try_recv()
    }

    pub fn increase_simulation_speed(&mut self) {
        if self.multiplier_index < MULTIPLIERS.len() - 1 {
            self.multiplier_index += 1;
            self.time_scale = MULTIPLIERS[self.multiplier_index];
            self.send_command(Command::SetTimeScale(self.time_scale));
        }
    }

    pub fn decrease_simulation_speed(&mut self) {
        if self.multiplier_index > 0 {
            self.multiplier_index -= 1;
            self.time_scale = MULTIPLIERS[self.multiplier_index];
            self.send_command(Command::SetTimeScale(self.time_scale));
        }
    }

    pub fn spawn_train(&self) {
        let event = Command::TrainSpawn(Box::new(TrainSpawnState {
            direction: Direction::Even,
            speed_mps: 0.0,
            spawn_point: TrackPoint {
                block_id: 2,
                offset_m: 600.0,
            },
        }));
        self.send_command(event);
    }

    pub fn despawn_train(&self, id: TrainId) {
        self.send_command(Command::TrainDespawn(id));
    }

    pub fn time_scale_formatted(&self) -> String {
        if self.time_scale >= 1.0 {
            format!("{}x", self.time_scale as u32)
        } else {
            format!("{:.1}x", self.time_scale)
        }
    }

    pub fn sim_duration_formatted(&self) -> String {
        let sim_duration = self.sim_duration.load(Ordering::Relaxed);
        format!("{:5} us", (sim_duration * 1_000_000.0) as u32)
    }

    pub fn start(&mut self) {
        if self.thread.is_none() {
            let sim_duration = self.sim_duration.clone();
            let init = self.thread_init_state.take().unwrap();
            self.thread = Some(
                thread::Builder::new()
                    .name("SimThread".into())
                    .spawn(move || SimulationState::new(init).simulate(sim_duration))
                    .unwrap(),
            );
        }
    }

    pub fn stop(&mut self) {
        if let Some(thread) = self.thread.take() {
            self.send_command(Command::Shutdown);
            thread.join().unwrap();
        }
    }
}
