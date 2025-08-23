use crate::common::draw_text_centered;
use crate::consts::TRACK_WIDTH;
use crate::display::lamp::{LAMP_COLOR_GRAY, LAMP_COLOR_GREEN, LAMP_COLOR_RED, Lamp, LampId, LampState};
use crate::display::signal::TrackSignalCommonState;
use crate::level::{Level, SignalData};
use chrono::NaiveDateTime;
use raylib::prelude::*;
use std::collections::HashMap;

const BOARD_BACKGROUND: Color = Color::new(0x64, 0xA0, 0x64, 0xFF);
const FLASH_INTERVAL: f64 = 0.65;

pub struct DisplayBoard {
    current_time: String,
    width: u32,
    height: u32,
    board_texture: Option<RenderTexture2D>,
    lamps: HashMap<LampId, Lamp>,
    signals: HashMap<usize, SignalData>,
}

impl DisplayBoard {
    pub fn new(level: &Level, width: u32, height: u32) -> Self {
        DisplayBoard {
            current_time: String::default(),
            width,
            height,
            board_texture: None,
            lamps: level.lamps.iter().cloned().map(|l| (l.id, l)).collect(),
            signals: level.signals.iter().cloned().map(|sig| (sig.id, sig)).collect(),
        }
    }

    fn generate_board_texture(&self, d: &mut RaylibDrawHandle, thread: &RaylibThread) -> RenderTexture2D {
        let mut texture = d.load_render_texture(thread, self.width, self.height).unwrap();
        let signals = TrackSignalCommonState::new(d, thread);

        d.draw_texture_mode(thread, &mut texture, |mut d| {
            d.draw_rectangle(0, 50, 300, TRACK_WIDTH, Color::BLACK);
            for signal in self.signals.values() {
                let lamp = self.lamps.get(&signal.lamp_id).unwrap();
                signals.draw(&mut d, lamp.x, lamp.y, &signal.name, signal.direction);
            }
        });

        texture
    }

    pub fn clock_update(&mut self, current_time: NaiveDateTime) {
        self.current_time = current_time.format("%H:%M:%S").to_string();
    }

    pub fn process_update(&mut self, lamp_id: LampId, new_state: bool) {
        if let Some(lamp) = self.lamps.get_mut(&lamp_id) {
            lamp.state = if new_state {
                LampState::ON(if lamp_id >= 100 {
                    LAMP_COLOR_GREEN
                } else {
                    LAMP_COLOR_RED
                })
            } else {
                LampState::OFF(LAMP_COLOR_GRAY)
            }
        }
    }

    pub fn draw(&mut self, d: &mut RaylibDrawHandle, thread: &RaylibThread) {
        if self.board_texture.is_none() {
            self.board_texture = self.generate_board_texture(d, thread).into();
        }
        let texture = self.board_texture.as_ref().unwrap();

        d.clear_background(BOARD_BACKGROUND);
        d.draw_texture_rec(
            texture,
            Rectangle {
                width: texture.width() as f32,
                height: -texture.height() as f32,
                ..Default::default()
            },
            Vector2::default(),
            Color::WHITE,
        );
        draw_text_centered(d, &self.current_time, d.get_screen_width() / 2, 3, 20, Color::RAYWHITE);

        let flash_state = (d.get_time() / FLASH_INTERVAL) as i32 % 2 > 0;
        for lamp in self.lamps.values() {
            lamp.draw(d, flash_state);
        }
    }
}
