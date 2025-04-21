use crate::common::Direction;
use crate::display::lamp::LAMP_COLOR_GRAY;
use crate::level::SignalData;
use raylib::prelude::*;

const LEG_LENGTH: i32 = 5;
const WIDTH: i32 = 14;
const HEIGHT: i32 = 6;
const FONT_SIZE: f32 = 16.5;

pub struct TrackSignalCommonState {
    font: Font,
    texture: RenderTexture2D,
}

impl TrackSignalCommonState {
    pub fn new(d: &mut RaylibDrawHandle, thread: &RaylibThread) -> Self {
        let mut texture = d
            .load_render_texture(thread, (WIDTH + LEG_LENGTH) as u32, HEIGHT as u32)
            .unwrap();
        d.draw_texture_mode(thread, &mut texture, |mut d| {
            d.draw_rectangle_rounded(
                Rectangle {
                    x: LEG_LENGTH as f32,
                    y: 0.0,
                    width: WIDTH as f32,
                    height: HEIGHT as f32,
                },
                1.0,
                4,
                Color::BLACK,
            );
            d.draw_line_ex(
                Vector2 {
                    x: 0.0,
                    y: (HEIGHT / 2) as f32,
                },
                Vector2 {
                    x: LEG_LENGTH as f32,
                    y: (HEIGHT / 2) as f32,
                },
                2.0,
                Color::BLACK,
            );
            d.draw_line_ex(
                Vector2 { x: 1.0, y: 0.0 },
                Vector2 {
                    x: 1.0,
                    y: HEIGHT as f32,
                },
                1.0,
                Color::BLACK,
            );
        });
        TrackSignalCommonState {
            font: Self::load_font(d, thread),
            texture,
        }
    }

    fn load_font(d: &mut RaylibDrawHandle, thread: &RaylibThread) -> Font {
        // https://github.com/raysan5/raylib/discussions/2499
        let codepoints: Vec<u16> = (32..512).map(|i| if i > 127 { 0x380 + i } else { i }).collect();
        let codepoints_string = String::from_utf16(codepoints.as_slice()).unwrap();
        let font_path = "resources/font/OpirusOpikRegular-RgDv.ttf";
        d.load_font_ex(thread, font_path, 33, Some(&codepoints_string)).unwrap()
    }
}

pub type SignalId = usize;

pub struct TrackSignal {
    id: SignalId,
    name: String,
    source_rect: Rectangle,
    texture_position: Vector2,
    lamp_rect: Rectangle,
    text_position: Vector2,
}

impl TrackSignal {
    pub fn new(id: SignalId, x: i32, y: i32, name: String, direction: Direction) -> Self {
        let (source_rect, texture_position, text_offset) = match direction {
            Direction::Even => (
                Rectangle {
                    x: 0.0,
                    y: 0.0,
                    width: (WIDTH + LEG_LENGTH) as f32,
                    height: HEIGHT as f32,
                },
                Vector2 {
                    x: (x - LEG_LENGTH - 1) as f32,
                    y: (y - 1) as f32,
                },
                -(LEG_LENGTH + 10 + 3), // TODO: take text width into account
            ),
            Direction::Odd => (
                // to flip a texture, use negative source width/height
                // https://www.reddit.com/r/raylib/comments/nvtyqn/how_do_i_flip_a_texture/
                Rectangle {
                    x: 0.0,
                    y: 0.0,
                    width: -(WIDTH + LEG_LENGTH) as f32,
                    height: HEIGHT as f32,
                },
                Vector2 {
                    x: (x - 1) as f32,
                    y: (y - 1) as f32,
                },
                WIDTH + LEG_LENGTH + 3,
            ),
        };
        TrackSignal {
            id,
            name,
            source_rect,
            texture_position,
            lamp_rect: Rectangle {
                x: x as f32,
                y: y as f32,
                width: (WIDTH - 2) as f32,
                height: (HEIGHT - 2) as f32,
            },
            text_position: Vector2 {
                x: (x + text_offset) as f32,
                y: (y - 6) as f32,
            },
        }
    }

    pub fn draw(&self, d: &mut RaylibDrawHandle, common: &TrackSignalCommonState) {
        d.draw_texture_rec(&common.texture, &self.source_rect, &self.texture_position, Color::WHITE);
        d.draw_rectangle_rounded(&self.lamp_rect, 1.0, 4, LAMP_COLOR_GRAY);
        d.draw_text_ex(
            &common.font,
            &self.name,
            &self.text_position,
            FONT_SIZE,
            1.0,
            Color::BLACK,
        );
    }
}

impl From<SignalData> for TrackSignal {
    fn from(data: SignalData) -> Self {
        TrackSignal::new(data.id, data.x, data.y, data.name, data.direction)
    }
}
