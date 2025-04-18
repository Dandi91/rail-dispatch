mod clock;
mod common;
mod consts;
mod display;
mod event;
mod game_state;
mod lamp;
mod level;
mod simulation;

use crate::game_state::GameState;
use crate::level::Level;
use raylib::prelude::*;

fn main() {
    let (mut rl, thread) = init().size(1024, 960).title("Rail Dispatch").resizable().build();

    rl.set_target_fps(60);

    let level = Level::load_from_file("resources/level.toml");
    let mut state = GameState::new(&level);
    state.start_game();

    while !rl.window_should_close() {
        state.process_updates();
        let mut d = rl.begin_drawing(&thread);
        state.process_input(&d);
        state.draw(&mut d, &thread);
        d.draw_fps(3, 5);
    }

    state.stop_game();
}
