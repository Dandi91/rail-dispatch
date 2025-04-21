use crate::common::draw_text_centered;
use crate::consts::TRACK_WIDTH;
use crate::display::lamp::{LAMP_COLOR_GRAY, LAMP_COLOR_RED, Lamp, LampState};
use crate::display::signal::{TrackSignal, TrackSignalCommonState};
use crate::level::Level;
use crate::simulation::block::BlockId;
use chrono::NaiveDateTime;
use raylib::prelude::*;
use std::collections::HashMap;

const BOARD_BACKGROUND: Color = Color::new(0x64, 0xA0, 0x64, 0xFF);
const FLASH_INTERVAL: f64 = 0.65;

pub struct DisplayBoard {
    current_time: String,
    signal_common: Option<TrackSignalCommonState>,
    lamps: HashMap<usize, Lamp>,
    signals: HashMap<usize, TrackSignal>,
}

impl DisplayBoard {
    pub fn new(level: &Level) -> Self {
        DisplayBoard {
            current_time: String::default(),
            signal_common: None,
            lamps: level.lamps.iter().cloned().map(|l| (l.id, l)).collect(),
            signals: level.signals.iter().cloned().map(|sig| (sig.id, sig.into())).collect(),
        }
    }

    pub fn clock_update(&mut self, current_time: NaiveDateTime) {
        self.current_time = current_time.format("%H:%M:%S").to_string();
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

    pub fn draw(&mut self, d: &mut RaylibDrawHandle, thread: &RaylibThread) {
        if self.signal_common.is_none() {
            self.signal_common = Some(TrackSignalCommonState::new(d, thread));
        }

        d.clear_background(BOARD_BACKGROUND);
        draw_text_centered(d, &self.current_time, d.get_screen_width() / 2, 3, 20, Color::RAYWHITE);

        d.draw_rectangle(0, 50, 300, TRACK_WIDTH, Color::BLACK);
        let flash_state = (d.get_time() / FLASH_INTERVAL) as i32 % 2 > 0;
        for lamp in self.lamps.values() {
            lamp.draw(d, flash_state);
        }

        for signal in self.signals.values() {
            signal.draw(d, self.signal_common.as_ref().unwrap());
        }
    }
}
