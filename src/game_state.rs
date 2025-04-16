use crate::common::Drawable;
use crate::display_board::DisplayBoard;
use crate::engine::Engine;
use crate::level::Level;
use crate::speed_table::SpeedTable;
use raylib::RaylibThread;
use raylib::consts::KeyboardKey;
use raylib::drawing::RaylibDrawHandle;

enum State {
    Board,
    SpeedTable,
}

const DEFAULT_STATE: State = State::Board;

pub struct GameState<'a> {
    state: State,
    level: &'a Level,
    board: DisplayBoard<'a>,
    speed_table: SpeedTable,
    pub engine: Engine,
}

impl GameState<'_> {
    pub fn new(level: &Level) -> GameState {
        GameState {
            state: State::Board,
            level,
            board: DisplayBoard::new(&level),
            speed_table: SpeedTable::new(),
            engine: Engine::new(level),
        }
    }

    fn debug_spawn_train(&mut self) {
        let train_number = self.engine.add_train();
        println!("Train {} registered", train_number);
    }

    fn debug_despawn_train(&mut self) {
        if let Some(train) = self.engine.remove_last_train() {
            println!("Train {} deregistered", train.number);
        }
    }

    pub fn process_input(&mut self, d: &RaylibDrawHandle) {
        if d.is_key_pressed(KeyboardKey::KEY_S) {
            self.state = match self.state {
                State::SpeedTable => DEFAULT_STATE,
                _ => State::SpeedTable,
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
}

impl Drawable for GameState<'_> {
    fn draw(&mut self, d: &mut RaylibDrawHandle, thread: &RaylibThread) {
        match self.state {
            State::Board => self.board.draw(d, thread),
            State::SpeedTable => self.speed_table.draw(d, thread),
        }
    }
}
