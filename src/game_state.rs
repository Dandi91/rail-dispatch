use crate::display::display_board::DisplayBoard;
use crate::display::speed_table::SpeedTable;
use crate::display::train::TrainDisplayState;
use crate::event::SimulationUpdate;
use crate::level::Level;
use crate::simulation::engine::Engine;
use itertools::Itertools;
use raylib::RaylibThread;
use raylib::color::Color;
use raylib::consts::KeyboardKey;
use raylib::drawing::{RaylibDraw, RaylibDrawHandle};

enum UIState {
    Board,
    SpeedTable,
}

const DEFAULT_UI_STATE: UIState = UIState::Board;

pub struct GameState {
    // UI
    ui_state: UIState,
    board: DisplayBoard,
    speed_table: SpeedTable,
    // Logic
    engine: Engine,
    trains: Vec<TrainDisplayState>,
}

impl GameState {
    pub fn new() -> GameState {
        let level = Level::load_from_file("resources/level.toml");
        GameState {
            ui_state: UIState::Board,
            engine: Engine::new(&level),
            board: DisplayBoard::new(&level),
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

    pub fn process_updates(&mut self) {
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
                            self.trains.swap_remove(pos);
                        }
                    }
                    SimulationUpdate::TrainState(state) => {
                        self.speed_table.process_train_update(&state);
                    }
                    SimulationUpdate::BlockOccupation(block_id, state) => {
                        println!("Processing block {} occupied({})", block_id, state);
                        self.board.process_update(block_id, state);
                    }
                    SimulationUpdate::Clock(elapsed_seconds) => {
                        self.speed_table.update(elapsed_seconds);
                    }
                },
                Err(..) => return,
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
            UIState::Board => self.board.draw(d),
            UIState::SpeedTable => self.speed_table.draw(d, thread),
        };
        d.draw_text(&self.engine.sim_duration_formatted(), 700, 3, 20, Color::RAYWHITE);
        d.draw_text(&self.engine.time_scale_formatted(), 800, 3, 20, Color::RAYWHITE);
    }
}
