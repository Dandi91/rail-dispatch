use raylib::drawing::RaylibDrawHandle;
use serde_repr::Deserialize_repr;

#[derive(Deserialize_repr)]
#[repr(i8)]
pub enum Direction {
    Even = 1,
    Odd = -1,
}

impl Direction {
    pub fn reverse(self) -> Direction {
        match self {
            Direction::Even => Direction::Odd,
            Direction::Odd => Direction::Even,
        }
    }
}

pub trait Drawable {
    fn draw(&self, d: &mut RaylibDrawHandle);
}


pub trait SimObject: Send + Sync {
    fn tick(&mut self, dt: f64);
}