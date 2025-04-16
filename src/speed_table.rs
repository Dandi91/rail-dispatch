use crate::common::{Drawable, draw_text_centered};
use raylib::prelude::*;

const PADDING: i32 = 60;
const X_OFFSET: i32 = 20;
const TRAIN_GRAPH_HEIGHT: i32 = 100;
const TIME_LABELS_HEIGHT: i32 = 20;
const TRAIN_HEADER_HEIGHT: i32 = 20;
const GRAPH_HEIGHT: i32 = TRAIN_GRAPH_HEIGHT + TIME_LABELS_HEIGHT;
const TRAIN_CARD_HEIGHT: i32 = TRAIN_HEADER_HEIGHT + GRAPH_HEIGHT;

const MAX_TRAINS_VISIBLE: i32 = 6;
const WIDGET_WIDTH: i32 = 980;
const WIDGET_HEIGHT: i32 = MAX_TRAINS_VISIBLE * TRAIN_CARD_HEIGHT + PADDING;
const WIDTH: i32 = WIDGET_WIDTH - PADDING + 1;

pub struct SpeedTable {
    graph_image: Image,
    graph_texture: Option<Texture2D>,
    texture_needs_updating: bool,

    num_trains: i32,
    height: i32,
    screen_image: Image,
    screen_texture: Option<Texture2D>,

    scroll: Vector2,
    view: Rectangle,
}

impl SpeedTable {
    pub fn new() -> Self {
        let height = 1; // initially no trains are registered, so keep it at minimum
        SpeedTable {
            graph_image: Image::gen_image_color(WIDTH, GRAPH_HEIGHT, Color::BLANK),
            graph_texture: None,
            texture_needs_updating: true,
            num_trains: 0,
            height,
            screen_image: Image::gen_image_color(WIDTH, height, Color::BLANK),
            screen_texture: None,
            scroll: Vector2::default(),
            view: Rectangle::default(),
        }
    }

    fn draw_no_trains(&self, d: &mut RaylibDrawHandle) {
        let font_size = 40;
        let x = WIDGET_WIDTH / 2;
        let y = (100 - font_size) / 2;
        draw_text_centered(d, "No trains", x, y, font_size, Color::BLACK);
    }

    fn update_graph_texture(&mut self, d: &mut RaylibDrawHandle, thread: &RaylibThread) {
        if self.texture_needs_updating {
            if self.graph_texture.is_none() {
                let texture_result = d.load_texture_from_image(thread, &self.graph_image);
                self.graph_texture = Some(texture_result.expect("Failed to load texture"));
            }
            self.texture_needs_updating = false;
        }
    }
}

impl Drawable for SpeedTable {
    fn draw(&mut self, d: &mut RaylibDrawHandle, thread: &RaylibThread) {
        d.clear_background(Color::LIGHTGRAY);
        d.set_window_size(WIDGET_WIDTH, WIDGET_HEIGHT);

        if self.num_trains == 0 {
            self.draw_no_trains(d);
            return;
        }

        self.update_graph_texture(d, thread);
        match self.screen_texture {
            Some(ref mut texture) => {
                if texture.height != self.height {
                    let texture_result = d.load_texture_from_image(thread, &self.screen_image);
                    self.screen_texture = Some(texture_result.expect("Failed to load texture"));
                } else {
                    let data = unsafe {
                        std::slice::from_raw_parts(
                            self.screen_image.data as *const u8,
                            self.screen_image.get_pixel_data_size(),
                        )
                    };
                    texture.update_texture(data).unwrap();
                }
            }
            None => {
                let texture_result = d.load_texture_from_image(thread, &self.screen_image);
                self.screen_texture = Some(texture_result.expect("Failed to load texture"));
            }
        }

        let half_padding = PADDING / 2;
        let scroll_bar_width = 20;
        (_, self.view, self.scroll) = d.gui_scroll_panel(
            Rectangle {
                width: WIDGET_WIDTH as f32,
                height: WIDGET_HEIGHT as f32,
                ..Rectangle::default()
            },
            "Train speed graphs",
            Rectangle {
                width: (WIDTH + PADDING - scroll_bar_width) as f32,
                height: (self.height + half_padding) as f32,
                ..Rectangle::default()
            },
            self.scroll,
            self.view,
        );

        d.draw_scissor_mode(
            self.view.x as i32,
            self.view.y as i32,
            self.view.width as i32,
            self.view.height as i32,
            |mut d| {
                let scroll_offset_x = half_padding + self.scroll.x as i32 - scroll_bar_width / 2;
                let scroll_offset_y = half_padding + self.scroll.y as i32;
                // draw speed grid for every train
                for idx in 0..self.num_trains {
                    let offset_y = idx * TRAIN_CARD_HEIGHT + TRAIN_HEADER_HEIGHT;
                    d.draw_texture(
                        self.graph_texture.as_ref().unwrap(),
                        scroll_offset_x,
                        offset_y + scroll_offset_y,
                        Color::WHITE,
                    );
                }
                // draw train speed graphs on top
                d.draw_texture(
                    self.screen_texture.as_ref().unwrap(),
                    scroll_offset_x,
                    scroll_offset_y,
                    Color::WHITE,
                );
            },
        );
    }
}
