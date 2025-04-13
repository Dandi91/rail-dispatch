use raylib::drawing::RaylibDrawHandle;
use serde_repr::Deserialize_repr;
use std::ops::Neg;

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

pub trait Drawable {
    fn draw(&self, d: &mut RaylibDrawHandle);
}
