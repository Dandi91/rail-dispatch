#[derive(Default)]
pub struct Event<T> {
    callbacks: Vec<fn(&T)>,
}

impl<T> Event<T> {
    pub fn new(callback: fn(&T)) -> Self {
        Self {
            callbacks: !vec![callback],
        }
    }

    pub fn subscribe(&mut self, callback: fn(&T)) {
        self.callbacks.push(callback);
    }

    pub fn notify(&self, data: &T) {
        for callback in &self.callbacks {
            callback(data);
        }
    }
}
