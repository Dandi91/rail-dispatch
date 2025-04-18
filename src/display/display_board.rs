use crate::consts::TRACK_WIDTH;
use crate::display::lamp::{Lamp, LampState, LAMP_COLOR_GRAY, LAMP_COLOR_RED};
use crate::level::{Level, SignalData};
use crate::simulation::block::BlockId;
use raylib::prelude::*;
use std::collections::HashMap;

const BOARD_BACKGROUND: Color = Color::new(0x64, 0xA0, 0x64, 0xFF);
const FLASH_INTERVAL: f64 = 0.65;

pub struct DisplayBoard {
    lamps: HashMap<usize, Lamp>,
    signals: HashMap<usize, SignalData>,
}

impl DisplayBoard {
    pub fn new(level: &Level) -> Self {
        DisplayBoard {
            lamps: level.lamps.iter().cloned().map(|l| (l.id, l)).collect(),
            signals: level.signals.iter().cloned().map(|sig| (sig.id , sig)).collect(),
        }
    }

    pub fn process_update(&mut self, block_id: BlockId, new_state: bool) {
        if let Some(lamp) = self.lamps.get_mut(&block_id) {
            if new_state {
                lamp.state = LampState::ON(LAMP_COLOR_RED);
            } else {
                lamp.state = LampState::OFF(LAMP_COLOR_GRAY);
            }
        }
    }

    pub fn draw(&mut self, d: &mut RaylibDrawHandle) {
        d.clear_background(BOARD_BACKGROUND);
        d.draw_rectangle(0, 50, 300, TRACK_WIDTH, Color::BLACK);

        let flash_state = (d.get_time() / FLASH_INTERVAL) as i32 % 2 > 0;
        for lamp in self.lamps.values() {
            lamp.draw(d, flash_state);
        }

        // for signal in self.level.signals {
        //     signal.draw(d);
        // }
    }
}
