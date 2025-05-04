use crate::clock::ClockEvent;
use crate::display::display_board::DisplayBoard;
use crate::display::speed_table::SpeedTable;
use crate::display::train::TrainDisplayState;
use crate::event::SimulationUpdate;
use crate::level::Level;
use crate::simulation::engine::Engine;
use itertools::Itertools;
use raylib::prelude::*;
use std::sync::mpsc::TryRecvError;

enum UIState {
    Board,
    SpeedTable,
}

const DEFAULT_UI_STATE: UIState = UIState::Board;

pub struct GameState {
    // UI
    sim_duration: f64,
    ui_state: UIState,
    board: DisplayBoard,
    speed_table: SpeedTable,
    // Logic
    engine: Engine,
    trains: Vec<TrainDisplayState>,
}

impl GameState {
    pub fn new(width: u32, height: u32) -> GameState {
        let level = Level::load_from_file("resources/level.toml");
        GameState {
            sim_duration: 0.0,
            ui_state: UIState::Board,
            engine: Engine::new(&level),
            board: DisplayBoard::new(&level, width, height),
            speed_table: SpeedTable::new(),
            trains: Vec::new(),
        }
    }

    fn debug_spawn_train(&self) {
        self.engine.spawn_train();
    }

    fn debug_despawn_train(&self) {
        if let Some(train) = self.trains.first() {
            self.engine.despawn_train(train.id);
        }
    }

    pub fn process_updates(&mut self, d: &RaylibDrawHandle) {
        loop {
            match self.engine.receive_command() {
                Ok(update) => match update {
                    SimulationUpdate::RegisterTrain(train) => {
                        println!("Train {} registered with ID {}", train.number, train.id);
                        self.speed_table.register_train(&train);
                        self.trains.push(train);
                    }
                    SimulationUpdate::UnregisterTrain(id) => {
                        let found = self.trains.iter().find_position(|x| x.id == id);
                        if let Some((pos, train)) = found {
                            println!("Train {} despawned with ID {}", train.number, train.id);
                            self.speed_table.unregister_train(id);
                            self.trains.remove(pos);
                        }
                    }
                    SimulationUpdate::TrainStates(time, updates) => {
                        self.speed_table.update(time, &updates);
                    }
                    SimulationUpdate::LampState(lamp_id, state) => {
                        self.board.process_update(lamp_id, state);
                    }
                    SimulationUpdate::Clock(payload) => match payload.event {
                        ClockEvent::SpeedTableTailClean => self.speed_table.cleanup_tail(),
                        ClockEvent::EveryQuarterHour => self.speed_table.scroll_quarter(d, payload.current_time),
                        ClockEvent::ClockUpdate => self.board.clock_update(payload.current_time),
                        _ => {}
                    },
                    SimulationUpdate::SimDuration(duration) => {
                        self.sim_duration = duration;
                    }
                },
                Err(err) => {
                    match err {
                        TryRecvError::Empty => return,
                        TryRecvError::Disconnected => panic!("SimThread crashed"),
                    };
                }
            }
        }
    }

    pub fn process_input(&mut self, d: &RaylibDrawHandle) {
        if d.is_key_pressed(KeyboardKey::KEY_S) {
            self.ui_state = match self.ui_state {
                UIState::SpeedTable => DEFAULT_UI_STATE,
                _ => UIState::SpeedTable,
            }
        }

        // sim speed control
        if d.is_key_pressed(KeyboardKey::KEY_UP) {
            self.engine.increase_simulation_speed();
        }
        if d.is_key_pressed(KeyboardKey::KEY_DOWN) {
            self.engine.decrease_simulation_speed();
        }

        // debug train spawn
        if d.is_key_pressed(KeyboardKey::KEY_G) {
            self.debug_spawn_train()
        }
        if d.is_key_pressed(KeyboardKey::KEY_H) {
            self.debug_despawn_train()
        }
    }

    pub fn start_game(&mut self) {
        self.engine.start();
    }

    pub fn stop_game(&mut self) {
        self.engine.stop();
    }

    pub fn draw(&mut self, d: &mut RaylibDrawHandle, thread: &RaylibThread) {
        match self.ui_state {
            UIState::Board => self.board.draw(d, thread),
            UIState::SpeedTable => self.speed_table.draw(d, thread),
        };
        d.draw_text(&self.sim_duration_formatted(), 700, 3, 20, Color::RAYWHITE);
        d.draw_text(&self.engine.time_scale_formatted(), 800, 3, 20, Color::RAYWHITE);
    }

    fn sim_duration_formatted(&self) -> String {
        format!("{:5} us", (self.sim_duration * 1_000_000.0) as u32)
    }
}
