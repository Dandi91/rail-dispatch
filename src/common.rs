use raylib::color::Color;
use raylib::drawing::{RaylibDraw, RaylibDrawHandle};
use serde_repr::Deserialize_repr;
use std::ops::Neg;

pub type TrainID = usize;

#[derive(Deserialize_repr, PartialEq, Copy, Clone)]
#[repr(i8)]
pub enum Direction {
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

pub fn draw_text_centered(
    d: &mut RaylibDrawHandle,
    text: &str,
    x: i32,
    y: i32,
    font_size: i32,
    color: Color,
) {
    let width = d.measure_text(text, font_size);
    d.draw_text(text, x - width / 2, y, font_size, color);
}
