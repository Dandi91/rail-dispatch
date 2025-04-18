use crate::common::{TrainID, draw_text_centered};
use crate::display::train::TrainDisplayState;
use crate::simulation::train::TrainStatusUpdate;
use itertools::Itertools;
use raylib::prelude::*;
use std::iter::zip;

const PADDING: i32 = 60;
const X_OFFSET: i32 = 20;
const TRAIN_GRID_HEIGHT: i32 = 100;
const TIME_LABELS_HEIGHT: i32 = 20;
const TRAIN_HEADER_HEIGHT: i32 = 20;
const GRID_HEIGHT: i32 = TRAIN_GRID_HEIGHT + TIME_LABELS_HEIGHT;
const TRAIN_CARD_HEIGHT: i32 = TRAIN_HEADER_HEIGHT + GRID_HEIGHT;

const MAX_TRAINS_VISIBLE: i32 = 6;
const WIDGET_WIDTH: i32 = 980;
const WIDGET_HEIGHT: i32 = MAX_TRAINS_VISIBLE * TRAIN_CARD_HEIGHT + PADDING;
const WIDTH: i32 = WIDGET_WIDTH - PADDING + 1;

#[derive(Default)]
struct TrainSpeedEntry {
    id: TrainID,
    number: String,
    next_block_m: f64,
    speed_mps: f64,
    target_speed_mps: f64,
    controls_percentage: i32,
    // braking_distance_m: f64,
}

pub struct SpeedTable {
    grid_image: Image,
    grid_texture: Option<Texture2D>,
    texture_needs_updating: bool,

    trains: Vec<TrainSpeedEntry>,
    height: i32,
    screen_image: Image,
    screen_texture: Option<Texture2D>,

    scroll: Vector2,
    view: Rectangle,
}

impl SpeedTable {
    pub fn new() -> Self {
        let height = 1; // initially no trains are registered, so keep it at minimum
        let mut result = SpeedTable {
            grid_image: Image::gen_image_color(WIDTH, GRID_HEIGHT, Color::BLANK),
            grid_texture: None,
            texture_needs_updating: true,
            trains: Vec::new(),
            height,
            screen_image: Image::gen_image_color(WIDTH, height, Color::BLANK),
            screen_texture: None,
            scroll: Vector2::default(),
            view: Rectangle::default(),
        };
        result.draw_speed_grid();
        result
    }

    pub fn register_train(&mut self, train: &TrainDisplayState) {
        self.trains.push(TrainSpeedEntry {
            id: train.id,
            number: train.number.clone(),
            ..TrainSpeedEntry::default()
        });
        self.height += TRAIN_CARD_HEIGHT;
        self.screen_image.resize_canvas(WIDTH, self.height, 0, 0, Color::BLANK);
    }

    pub fn unregister_train(&mut self, train_id: TrainID) {
        let entry = self.trains.iter().find_position(|t| t.id == train_id);
        if let Some((index, ..)) = entry {
            if index != self.trains.len() - 1 {
                let y = index as i32 * TRAIN_CARD_HEIGHT;
                let height = self.height - (y + TRAIN_CARD_HEIGHT);

                // copy and crop trains below the deleted one
                let mut bottom_part = self.screen_image.clone();
                bottom_part.crop(Rectangle {
                    x: 0.0,
                    y: (y + TRAIN_CARD_HEIGHT) as f32,
                    width: WIDTH as f32,
                    height: height as f32,
                });

                // clean deleted train and everything below it
                self.screen_image
                    .draw_rectangle(0, y, WIDTH, height + TRAIN_CARD_HEIGHT, Color::BLANK);

                // copy saved part back in place
                self.screen_image.draw(
                    &bottom_part,
                    Rectangle {
                        x: 0.0,
                        y: 0.0,
                        width: WIDTH as f32,
                        height: height as f32,
                    },
                    Rectangle {
                        x: 0.0,
                        y: y as f32,
                        width: WIDTH as f32,
                        height: height as f32,
                    },
                    Color::WHITE,
                );
            }

            self.height -= TRAIN_CARD_HEIGHT;
            self.trains.remove(index);
            self.screen_image.resize_canvas(WIDTH, self.height, 0, 0, Color::BLANK);
        }
    }

    pub fn process_train_update(&mut self, update: &TrainStatusUpdate) {
        let entry = self.trains.iter_mut().find_position(|t| t.id == update.id);
        if let Some((.., train)) = entry {
            train.next_block_m = update.next_block_m;
            train.speed_mps = update.speed_mps;
            train.target_speed_mps = update.target_speed_mps;
            train.controls_percentage = update.control_percentage;
        }
    }

    pub fn update(&mut self, elapsed_seconds: f64) {
        let speed_color = Color::new(0xBB, 0x00, 0x00, 0xFF);
        let target_speed_color = Color::ORANGE;
        let max_speed_mps = 100.0 / 3.6;
        let max_time_s = 900;

        let speed_to_coord = |offset_y: i32, speed_mps: f64| -> i32 {
            let norm = 1.0 - (speed_mps.clamp(0.0, max_speed_mps) / max_speed_mps);
            (norm * TRAIN_GRID_HEIGHT as f64).trunc() as i32 + offset_y + TRAIN_HEADER_HEIGHT
        };

        let font_size = 10;
        let time_x = elapsed_seconds.round() as i32 % max_time_s + X_OFFSET;
        self.trains.iter().enumerate().for_each(|(index, train)| {
            let offset_y = index as i32 * TRAIN_CARD_HEIGHT;
            let target_speed_y = speed_to_coord(offset_y, train.target_speed_mps);
            let speed_y = speed_to_coord(offset_y, train.speed_mps);

            self.screen_image
                .draw_rectangle(0, offset_y, WIDTH, TRAIN_HEADER_HEIGHT, Color::BLANK);
            let text_y = offset_y + font_size / 2;
            let train_status_line = format!(
                "#{} | next block in {:.3} m | {:.0} km/h | {}%",
                &train.number,
                train.next_block_m,
                train.speed_mps * 3.6,
                train.controls_percentage,
            );
            self.screen_image
                .draw_text(&train_status_line, X_OFFSET, text_y, font_size, Color::BLACK);
            self.screen_image.draw_pixel(time_x, target_speed_y, target_speed_color);
            self.screen_image.draw_pixel(time_x, speed_y, speed_color);
        });
    }

    fn draw_speed_grid(&mut self) {
        let line_color = Color::new(0x8D, 0x8F, 0x94, 0xFF);
        let speed_labels = [None, Some("80"), Some("60"), Some("40"), Some("20"), None];
        let label_offset = 4;
        // horizontal lines
        for (y, label) in zip((0..GRID_HEIGHT).step_by(20), speed_labels) {
            self.grid_image.draw_line(X_OFFSET, y, WIDTH, y, &line_color);
            if let Some(label) = label {
                self.grid_image.draw_text(label, 0, y - label_offset, 10, Color::BLACK);
            }
        }
        // vertical lines
        for x in (X_OFFSET..=WIDTH).step_by(60) {
            self.grid_image.draw_line(x, 0, x, TRAIN_GRID_HEIGHT, &line_color);
        }
    }

    fn draw_no_trains(&self, d: &mut RaylibDrawHandle) {
        let font_size = 40;
        let x = WIDGET_WIDTH / 2;
        let y = (100 - font_size) / 2;
        draw_text_centered(d, "No trains", x, y, font_size, Color::BLACK);
    }

    fn update_grid_texture(&mut self, d: &mut RaylibDrawHandle, thread: &RaylibThread) {
        if self.texture_needs_updating {
            if self.grid_texture.is_none() {
                self.grid_texture = d.load_texture_from_image(thread, &self.grid_image).ok();
            }
            self.texture_needs_updating = false;
        }
    }

    pub fn draw(&mut self, d: &mut RaylibDrawHandle, thread: &RaylibThread) {
        d.clear_background(Color::LIGHTGRAY);
        d.set_window_size(WIDGET_WIDTH, WIDGET_HEIGHT);

        if self.trains.is_empty() {
            self.draw_no_trains(d);
            return;
        }

        self.update_grid_texture(d, thread);
        match self.screen_texture {
            Some(ref mut texture) => {
                if texture.height != self.height {
                    self.screen_texture = d.load_texture_from_image(thread, &self.screen_image).ok()
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
                self.screen_texture = d.load_texture_from_image(thread, &self.screen_image).ok();
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
                let texture = self.grid_texture.as_ref().unwrap();
                for idx in 0..self.trains.len() as i32 {
                    let offset_y = idx * TRAIN_CARD_HEIGHT + TRAIN_HEADER_HEIGHT;
                    d.draw_texture(texture, scroll_offset_x, offset_y + scroll_offset_y, Color::WHITE);
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
