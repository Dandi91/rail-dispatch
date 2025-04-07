use crate::common::Drawable;
use crate::consts::TRACK_WIDTH;
use crate::level::Level;
use raylib::prelude::*;
use std::collections::HashMap;
use crate::lamp::Lamp;

const BOARD_BACKGROUND: Color = Color::new(0x64, 0xA0, 0x64, 0xFF);
const FLASH_INTERVAL: f64 = 0.65;

pub struct DisplayBoard<'a> {
    level: &'a Level,
    lamps_by_id: HashMap<usize, &'a Lamp>,
}

impl<'a> DisplayBoard<'a> {
    pub fn new(level: &'a Level) -> Self {
        DisplayBoard {
            level,
            lamps_by_id: level.lamps.iter().map(|l| (l.id, l)).collect(),
        }
    }
}

impl Drawable for DisplayBoard<'_> {
    fn draw(&self, d: &mut RaylibDrawHandle) {
        d.clear_background(BOARD_BACKGROUND);
        d.draw_rectangle(0, 50, 300, TRACK_WIDTH, Color::BLACK);

        let flash_state = (d.get_time() / FLASH_INTERVAL) as i32 % 2 > 0;
        for lamp in &self.level.lamps {
            lamp.draw(d, flash_state);
        }

        // for signal in self.level.signals {
        //     signal.draw(d);
        // }
    }
}
