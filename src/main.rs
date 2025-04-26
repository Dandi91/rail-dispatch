#![windows_subsystem = "windows"]

mod clock;
mod common;
mod consts;
mod display;
mod event;
mod game_state;
mod level;
mod simulation;

use crate::game_state::GameState;
use raylib::prelude::*;

fn main() {
    let title = "Rail Dispatch";
    let (width, height) = (1024, 960);
    let (mut rl, thread) = init().size(width, height).title(title).resizable().build();
    rl.set_target_fps(60);

    let mut state = GameState::new(width as u32, height as u32);
    state.start_game();

    while !rl.window_should_close() {
        let mut d = rl.begin_drawing(&thread);
        state.process_updates(&d);
        state.process_input(&d);
        state.draw(&mut d, &thread);
        d.draw_fps(3, 5);
    }

    state.stop_game();
}
