use crate::common::SimObject;

pub struct Train {
    pub number: String,
}

impl Train {
    pub fn new() -> Self {
        Train {
            number: rand::random_range(1000..=9999).to_string(),
        }
    }
}

impl SimObject for Train {
    fn tick(&mut self, dt: f64) {
        todo!()
    }
}
