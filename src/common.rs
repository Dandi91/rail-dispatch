use bevy::prelude::*;
use serde::de::Visitor;
use serde::{Deserialize, Deserializer, de::Error};
use serde_repr::Deserialize_repr;
use std::fmt;
use std::ops::Neg;
use std::time::Instant;

pub type TrainId = u32;
pub type BlockId = u32;
pub type SignalId = u32;
pub type LampId = u32;

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

#[derive(Reflect, Copy, Clone)]
pub struct HexColor(Srgba);

impl<'de> Deserialize<'de> for HexColor {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct ColorVisitor;

        impl<'de> Visitor<'de> for ColorVisitor {
            type Value = HexColor;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a hex color string (e.g., #ff0000)")
            }

            fn visit_str<E: Error>(self, v: &str) -> Result<Self::Value, E> {
                Srgba::hex(v).map_err(E::custom).map(HexColor)
            }
        }

        deserializer.deserialize_str(ColorVisitor)
    }
}

impl From<HexColor> for Color {
    fn from(c: HexColor) -> Self {
        c.0.into()
    }
}

pub trait SpeedConv {
    fn value(&self) -> f64;

    fn kmh(&self) -> f64 {
        self.value() * 3.6
    }

    fn mps(&self) -> f64 {
        self.value() / 3.6
    }
}

impl SpeedConv for f64 {
    fn value(&self) -> f64 {
        *self
    }
}

#[cfg(test)]
pub fn wrap<T: PartialOrd>(value: T, low: T, high: T) -> T {
    if value > high {
        return low;
    }
    if value < low {
        return high;
    }
    value
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
