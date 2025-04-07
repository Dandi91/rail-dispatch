use crate::common::Drawable;
use crate::display_board::DisplayBoard;
use crate::engine::Engine;
use crate::level::Level;
use crate::train::Train;
use raylib::consts::KeyboardKey;
use raylib::drawing::RaylibDrawHandle;
use std::sync::Arc;

enum State {
    Board,
    SpeedTable,
}

pub struct GameState<'a> {
    state: State,
    level: &'a Level,
    trains: Vec<Arc<Train>>,
    board: DisplayBoard<'a>,
    // speed table
    pub engine: Engine,
}

impl GameState<'_> {
    pub fn new(level: &Level) -> GameState {
        GameState {
            state: State::Board,
            level,
            trains: Vec::new(),
            board: DisplayBoard::new(&level),
            engine: Engine::new(),
        }
    }

    fn debug_spawn_train(&mut self) {
        let train = Arc::new(Train::new());
        self.trains.push(train.clone());
        self.engine.add_sim_object(train.clone());
        println!("Train {} registered", train.number);
    }

    fn debug_despawn_train(&mut self) {
        if self.trains.len() > 0 {
            let train = self.trains.swap_remove(0);
            println!("Train {} deregistered", train.number);
            self.engine.remove_sim_object(train);
        }
    }

    pub fn process_input(&mut self, d: &RaylibDrawHandle) {
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
    fn draw(&self, d: &mut RaylibDrawHandle) {
        match self.state {
            State::Board => self.board.draw(d),
            State::SpeedTable => {}
        }
    }
}
