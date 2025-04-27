use crate::common::Direction;
use raylib::prelude::*;

const LEG_LENGTH: f32 = 5.0;
const WIDTH: f32 = 14.0;
const HEIGHT: f32 = 6.0;
const FONT_SIZE: f32 = 16.5;
const TEXT_OFFSET: f32 = 4.0;

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
                    x: LEG_LENGTH,
                    y: 0.0,
                    width: WIDTH,
                    height: HEIGHT,
                },
                1.0,
                4,
                Color::BLACK,
            );
            d.draw_line_ex(
                Vector2 {
                    x: 0.0,
                    y: HEIGHT / 2.0,
                },
                Vector2 {
                    x: LEG_LENGTH,
                    y: HEIGHT / 2.0,
                },
                2.0,
                Color::BLACK,
            );
            d.draw_line_ex(
                Vector2 { x: 1.0, y: 0.0 },
                Vector2 { x: 1.0, y: HEIGHT },
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

    pub fn draw(&self, d: &mut RaylibDrawHandle, x: f32, y: f32, name: &str, direction: Direction) {
        let x = x - 1.0;
        let text_size = self.font.measure_text(name, FONT_SIZE, 1.0);
        let (source_rect, texture_position, text_offset) = match direction {
            Direction::Even => (
                Rectangle {
                    width: WIDTH + LEG_LENGTH,
                    height: HEIGHT,
                    ..Default::default()
                },
                Vector2 { x: x - LEG_LENGTH, y },
                -(LEG_LENGTH + TEXT_OFFSET + text_size.x),
            ),
            Direction::Odd => (
                // to flip a texture, use negative source width/height
                // https://www.reddit.com/r/raylib/comments/nvtyqn/how_do_i_flip_a_texture/
                Rectangle {
                    width: -(WIDTH + LEG_LENGTH),
                    height: HEIGHT,
                    ..Default::default()
                },
                Vector2 { x, y },
                WIDTH + LEG_LENGTH + TEXT_OFFSET,
            ),
        };
        let text_position = Vector2 {
            x: x + text_offset,
            y: y - 5.0,
        };
        d.draw_texture_rec(&self.texture, source_rect, texture_position, Color::WHITE);
        d.draw_text_ex(&self.font, name, text_position, FONT_SIZE, 1.0, Color::BLACK);
    }
}
