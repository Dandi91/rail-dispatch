mod common;
mod consts;
mod display_board;
mod engine;
mod game_state;
mod lamp;
mod level;
mod train;

use crate::common::Drawable;
use crate::game_state::GameState;
use crate::level::Level;
use raylib::prelude::*;

fn main() {
    let (mut rl, thread) = init()
        .size(1024, 960)
        .title("Rail Dispatch")
        .resizable()
        .build();

    rl.set_target_fps(30);

    let level = Level::load_from_file("resources/level.toml");
    let mut state = GameState::new(&level);

    while !rl.window_should_close() {
        let mut d = rl.begin_drawing(&thread);
        state.process_input(&d);
        state.draw(&mut d);
        d.draw_fps(3, 5);
        d.draw_text(&state.engine.sim_duration_formatted(), 700, 3, 20, Color::RAYWHITE);
        d.draw_text(&state.engine.time_scale_formatted(), 800, 3, 20, Color::RAYWHITE);
    }
}
