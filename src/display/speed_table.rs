use crate::common::{LowerMultiple, TrainId, draw_text_centered, image_draw_text_centered};
use crate::display::train::TrainDisplayState;
use crate::simulation::train::TrainStatusUpdate;
use chrono::{NaiveDateTime, Timelike};
use itertools::Itertools;
use raylib::error::Error;
use raylib::prelude::*;
use std::iter::zip;

const PADDING: i32 = 40;
const V_PADDING: i32 = 10;
const X_OFFSET: i32 = 20;
const LABEL_OFFSET: i32 = 4;
const TRAIN_GRID_HEIGHT: i32 = 100;
const TIME_LABELS_HEIGHT: i32 = 20;
const TRAIN_HEADER_HEIGHT: i32 = 20;
const GRID_HEIGHT: i32 = TRAIN_GRID_HEIGHT + TIME_LABELS_HEIGHT;
const TRAIN_CARD_HEIGHT: i32 = TRAIN_HEADER_HEIGHT + GRID_HEIGHT;

const WIDGET_WIDTH: i32 = MAX_HORIZONTAL_SECONDS + PADDING + X_OFFSET;
const WIDTH: i32 = WIDGET_WIDTH - PADDING + 1;

pub const MAX_HORIZONTAL_MINUTES: i32 = 10;
pub const MAX_HORIZONTAL_SECONDS: i32 = MAX_HORIZONTAL_MINUTES * 60;
pub const KEEP_TAIL_S: i32 = 120;

#[derive(Default)]
struct TrainSpeedEntry {
    id: TrainId,
    number: String,
    next_block_m: f64,
    speed_mps: f64,
    target_speed_mps: f64,
    controls_percentage: i32,
    braking_distance_m: f64,
    signal_distance_m: f64,
    updated: bool,
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
    pub fn get_width() -> i32 {
        WIDGET_WIDTH
    }

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
            updated: true,
            ..TrainSpeedEntry::default()
        });
        self.height += TRAIN_CARD_HEIGHT;
        self.screen_image.resize_canvas(WIDTH, self.height, 0, 0, Color::BLANK);
    }

    pub fn unregister_train(&mut self, train_id: TrainId) {
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

    pub fn scroll_horizontally(&mut self, d: &RaylibDrawHandle, now: NaiveDateTime) {
        self.generate_time_labels(d, now);
        self.screen_image
            .draw_rectangle(0, 0, WIDTH - KEEP_TAIL_S, self.height, Color::BLANK);
        // reset train updates to draw them at least once
        for train in &mut self.trains {
            train.updated = true;
        }
    }

    pub fn cleanup_tail(&mut self) {
        self.screen_image
            .draw_rectangle(WIDTH - KEEP_TAIL_S, 0, WIDTH, self.height, Color::BLANK);
    }

    pub fn update(&mut self, elapsed_seconds: f64, train_updates: &[TrainStatusUpdate]) {
        for update in train_updates {
            let entry = self.trains.iter_mut().find_position(|t| t.id == update.id);
            if let Some((.., train)) = entry {
                train.next_block_m = update.next_block_m;
                train.speed_mps = update.speed_mps;
                train.target_speed_mps = update.target_speed_mps;
                train.controls_percentage = update.control_percentage;
                train.signal_distance_m = update.signal_distance_m;
                train.braking_distance_m = update.braking_distance_m;
                train.updated = true;
            }
        }

        let speed_color = Color::new(0xBB, 0x00, 0x00, 0xFF);
        let target_speed_color = Color::ORANGE;
        let max_speed_mps = 100.0 / 3.6;

        let speed_to_coord = |offset_y: i32, speed_mps: f64| -> i32 {
            let norm = 1.0 - (speed_mps.clamp(0.0, max_speed_mps) / max_speed_mps);
            (norm * TRAIN_GRID_HEIGHT as f64).trunc() as i32 + offset_y + TRAIN_HEADER_HEIGHT
        };

        let time_x = elapsed_seconds.round() as i32 % MAX_HORIZONTAL_SECONDS + X_OFFSET;
        self.trains.iter().enumerate().for_each(|(index, train)| {
            let offset_y = index as i32 * TRAIN_CARD_HEIGHT;
            let target_speed_y = speed_to_coord(offset_y, train.target_speed_mps);
            let speed_y = speed_to_coord(offset_y, train.speed_mps);

            self.screen_image.draw_pixel(time_x, target_speed_y, target_speed_color);
            self.screen_image.draw_pixel(time_x, speed_y, speed_color);
        });
    }

    /// Since drawing text takes ages, this is moved into a separate method, which is only called once per frame.
    /// It draws labels only for the trains that moved since last time (tracked with `TrainSpeedEntry.updated`),
    /// and only those that are visible in the scroll window at the moment.
    fn update_train_labels(&mut self) {
        let font_size = 10;
        self.trains
            .iter_mut()
            .enumerate()
            .filter(|(.., train)| train.updated)
            .for_each(|(index, train)| {
                let offset_y = index as i32 * TRAIN_CARD_HEIGHT;
                let screen_pos = offset_y + self.scroll.y as i32;
                if screen_pos + TRAIN_HEADER_HEIGHT >= 0 && screen_pos <= self.view.height as i32 {
                    self.screen_image
                        .draw_rectangle(X_OFFSET, offset_y, WIDTH, TRAIN_HEADER_HEIGHT, Color::BLANK);
                    let text_y = offset_y + font_size / 2;
                    let train_status_line = format!(
                        "#{} | block {:.3} m | {:.0} km/h | signal {:.0} m | braking {:.0} m | {}%",
                        &train.number,
                        train.next_block_m,
                        train.speed_mps * 3.6,
                        train.signal_distance_m,
                        train.braking_distance_m,
                        train.controls_percentage,
                    );
                    self.screen_image
                        .draw_text(&train_status_line, X_OFFSET, text_y, font_size, Color::BLACK);
                    train.updated = false;
                }
            });
    }

    fn generate_time_labels(&mut self, d: &RaylibDrawHandle, now: NaiveDateTime) {
        let span_length = MAX_HORIZONTAL_MINUTES as u32;
        let span_start = now.minute().lower_multiple(span_length);
        let time_labels =
            (span_start..span_start + span_length).map(|minute| format!("{:02}:{:02}", now.hour(), minute));
        // clear place before printing new text
        self.grid_image
            .draw_rectangle(0, TRAIN_GRID_HEIGHT + 1, WIDTH, TIME_LABELS_HEIGHT, Color::BLANK);
        zip((X_OFFSET..WIDTH).step_by(60), time_labels).for_each(|(x, label)| {
            image_draw_text_centered(
                d,
                &mut self.grid_image,
                &label,
                x,
                TRAIN_GRID_HEIGHT + LABEL_OFFSET,
                10,
                Color::BLACK,
            );
        });
        self.texture_needs_updating = true;
    }

    fn draw_speed_grid(&mut self) {
        let line_color = Color::new(0x8D, 0x8F, 0x94, 0xFF);
        let speed_labels = [None, Some("80"), Some("60"), Some("40"), Some("20"), None];
        // horizontal lines
        for (y, label) in zip((0..GRID_HEIGHT).step_by(20), speed_labels) {
            self.grid_image.draw_line(X_OFFSET, y, WIDTH, y, &line_color);
            if let Some(label) = label {
                self.grid_image.draw_text(label, 0, y - LABEL_OFFSET, 10, Color::BLACK);
            }
        }
        // vertical lines
        for x in (X_OFFSET..=WIDTH).step_by(60) {
            self.grid_image.draw_line(x, 0, x, TRAIN_GRID_HEIGHT, &line_color);
        }
    }

    fn draw_no_trains(&self, d: &mut RaylibDrawHandle, extent: &Rectangle) {
        let font_size = 40;
        let x = extent.width as i32 / 2;
        let y = (100 - font_size) / 2;
        draw_text_centered(
            d,
            "No trains",
            x + extent.x as i32,
            y + extent.y as i32,
            font_size,
            Color::BLACK,
        );
    }

    fn update_grid_texture(&mut self, d: &mut RaylibDrawHandle, thread: &RaylibThread) {
        if self.texture_needs_updating {
            match self.grid_texture {
                Some(ref mut texture) => update_texture(texture, &self.grid_image).unwrap(),
                None => self.grid_texture = d.load_texture_from_image(thread, &self.grid_image).ok(),
            };
            self.texture_needs_updating = false;
        }
    }

    pub fn draw(&mut self, d: &mut RaylibDrawHandle, thread: &RaylibThread, extent: &Rectangle) {
        d.draw_rectangle_rec(extent, Color::LIGHTGRAY);

        if self.trains.is_empty() {
            self.draw_no_trains(d, extent);
            return;
        }

        self.update_grid_texture(d, thread);
        self.update_train_labels();
        match self.screen_texture {
            Some(ref mut texture) => {
                if texture.height != self.height {
                    self.screen_texture = d.load_texture_from_image(thread, &self.screen_image).ok()
                } else {
                    update_texture(texture, &self.screen_image).unwrap();
                }
            }
            None => {
                self.screen_texture = d.load_texture_from_image(thread, &self.screen_image).ok();
            }
        }

        let half_padding = PADDING / 2;
        let v_padding = half_padding + V_PADDING;
        let scroll_bar_width = 20;
        d.gui_set_style(
            GuiControl::DEFAULT,
            GuiDefaultProperty::BACKGROUND_COLOR,
            Color::BLANK.color_to_int(),
        );
        (_, self.view, self.scroll) = d.gui_scroll_panel(
            extent,
            "Train speed graphs",
            Rectangle {
                x: extent.x,
                y: extent.y,
                width: (WIDTH + PADDING - scroll_bar_width) as f32,
                height: (self.height + V_PADDING) as f32,
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
                let scroll_offset_x = half_padding + self.scroll.x as i32 - scroll_bar_width / 2 + extent.x as i32;
                let scroll_offset_y = v_padding + self.scroll.y as i32 + extent.y as i32;
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

fn update_texture(texture: &mut Texture2D, image: &Image) -> Result<(), Error> {
    let data = unsafe { std::slice::from_raw_parts(image.data as *const u8, image.get_pixel_data_size()) };
    texture.update_texture(data)
}
