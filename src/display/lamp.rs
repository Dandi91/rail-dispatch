use crate::common::LampId;
use bevy::prelude::*;
use serde::Deserialize;

const DEFAULT_LAMP_HEIGHT: f32 = 5.0;

fn default_lamp_height() -> f32 {
    DEFAULT_LAMP_HEIGHT
}

fn default_lamp_state() -> LampState {
    LampState::OFF(LAMP_COLOR_GRAY)
}

pub const LAMP_COLOR_GRAY: Color = Color::srgba_u8(0x55, 0x55, 0x55, 0xFF);
pub const LAMP_COLOR_YELLOW: Color = Color::srgba_u8(0xFF, 0xFF, 0x40, 0xFF);
pub const LAMP_COLOR_RED: Color = Color::srgba_u8(0xFF, 0x20, 0x20, 0xFF);
pub const LAMP_COLOR_GREEN: Color = Color::srgba_u8(0x00, 0xFF, 0x00, 0xFF);

#[derive(Reflect, Clone)]
pub enum LampState {
    ON(Color),
    OFF(Color),
    FLASHING(Color),
}

#[derive(Deserialize, Reflect, Clone)]
pub struct Lamp {
    pub id: LampId,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    #[serde(default = "default_lamp_height")]
    pub height: f32,
    #[serde(default = "default_lamp_state")]
    #[serde(skip)]
    pub state: LampState,
}

impl Lamp {
    pub fn get_color(&self, flash_state: bool) -> Color {
        match self.state {
            LampState::ON(color) | LampState::OFF(color) => color,
            LampState::FLASHING(color) => {
                if flash_state {
                    color
                } else {
                    LAMP_COLOR_GRAY
                }
            }
        }
    }

    // pub fn draw(&self, d: &mut RaylibDrawHandle, flash_state: bool) {
    //     d.draw_rectangle_rounded(
    //         Rectangle::new(self.x, self.y + 1.0, self.width, self.height),
    //         1.0,
    //         4,
    //         self.get_color(flash_state),
    //     )
    // }
}
