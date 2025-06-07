use raylib::color::Color;
use raylib::drawing::{RaylibDraw, RaylibDrawHandle};
use raylib::texture::Image;
use serde_repr::Deserialize_repr;
use std::ops::Neg;
use std::time::Instant;

pub type TrainId = usize;

#[derive(Deserialize_repr, PartialEq, Copy, Clone, Default, Debug, Hash, Eq)]
#[repr(i8)]
pub enum Direction {
    #[default]
    Even = 1,
    Odd = -1,
}

impl Direction {
    pub fn reverse(&self) -> Direction {
        match self {
            Direction::Even => Direction::Odd,
            Direction::Odd => Direction::Even,
        }
    }

    pub fn apply_sign<T: Neg<Output = T>>(&self, value: T) -> T {
        match self {
            Direction::Even => value,
            Direction::Odd => value.neg(),
        }
    }
}

pub fn draw_text_centered(d: &mut RaylibDrawHandle, text: &str, x: i32, y: i32, font_size: i32, color: Color) {
    let width = d.measure_text(text, font_size);
    d.draw_text(text, x - width / 2, y, font_size, color);
}

pub fn image_draw_text_centered(
    d: &RaylibDrawHandle,
    image: &mut Image,
    text: &str,
    x: i32,
    y: i32,
    font_size: i32,
    color: Color,
) {
    let width = d.measure_text(text, font_size);
    image.draw_text(text, x - width / 2, y, font_size, color);
}

pub struct Profiler {
    now: Instant,
}

impl Profiler {
    #![allow(dead_code)]
    pub fn new() -> Self {
        Profiler { now: Instant::now() }
    }
}

impl Drop for Profiler {
    fn drop(&mut self) {
        println!("Scope took {} us", self.now.elapsed().as_micros());
    }
}
