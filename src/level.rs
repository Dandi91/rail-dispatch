use crate::common::Direction;
use crate::lamp::Lamp;
use serde::Deserialize;
use std::fs;
use std::io::Read;

#[derive(Deserialize)]
pub struct Level {
    pub lamps: Vec<Lamp>,
    pub blocks: Vec<BlockData>,
    pub connections: Vec<ConnectionData>,
    pub signals: Vec<SignalData>,
}

#[derive(Deserialize, Default)]
pub struct BlockData {
    pub id: usize,
    pub length: f64,
    pub lamp_id: usize,
}

#[derive(Deserialize)]
pub struct ConnectionData {
    pub start: usize,
    pub end: usize,
}

#[derive(Deserialize)]
pub struct SignalData {
    pub id: usize,
    pub x: i32,
    pub y: i32,
    pub name: String,
    pub direction: Direction,
}

impl Level {
    pub fn load_from_file(path: &str) -> Level {
        let mut file = fs::File::open(path).unwrap();
        let mut contents = String::new();
        file.read_to_string(&mut contents).unwrap();
        toml::from_str(&contents).unwrap()
    }
}
