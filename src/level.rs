use crate::common::Direction;
use crate::display::lamp::Lamp;
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

#[derive(Deserialize, Clone)]
pub struct SignalData {
    pub id: usize,
    pub lamp_id: usize,
    pub block_id: usize,
    pub offset_m: f64,
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
