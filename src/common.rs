use serde_repr::Deserialize_repr;
use std::ops::Neg;
use std::time::Instant;
use bevy::reflect::Reflect;

pub type TrainId = u64;

#[derive(Deserialize_repr, Reflect, PartialEq, Copy, Clone, Default, Debug, Hash, Eq)]
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

pub trait LowerMultiple {
    type Output;
    fn lower_multiple(self, divisor: Self::Output) -> Self::Output;
}

impl LowerMultiple for u32 {
    type Output = u32;
    fn lower_multiple(self, divisor: u32) -> u32 {
        self / divisor * divisor
    }
}

impl LowerMultiple for i32 {
    type Output = i32;
    fn lower_multiple(self, divisor: i32) -> i32 {
        self / divisor * divisor
    }
}

#[allow(dead_code)]
pub fn wrap<T: PartialOrd>(value: T, low: T, high: T) -> T {
    if value > high {
        return low;
    }
    if value < low {
        return high;
    }
    value
}

// pub fn draw_text_centered(d: &mut RaylibDrawHandle, text: &str, x: i32, y: i32, font_size: i32, color: Color) {
//     let width = d.measure_text(text, font_size);
//     d.draw_text(text, x - width / 2, y, font_size, color);
// }

// pub fn image_draw_text_centered(
//     d: &RaylibDrawHandle,
//     image: &mut Image,
//     text: &str,
//     x: i32,
//     y: i32,
//     font_size: i32,
//     color: Color,
// ) {
//     let width = d.measure_text(text, font_size);
//     image.draw_text(text, x - width / 2, y, font_size, color);
// }

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wrap_add() {
        let cases = [(3, 3), (-1, 4), (0, 0), (5, 0)];
        for (val, expected) in cases {
            assert_eq!(wrap(val, 0, 4), expected);
        }
    }
}
