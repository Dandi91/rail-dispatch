use crate::event::Event;
use chrono::{Local, NaiveDateTime, TimeDelta};
use std::collections::VecDeque;
use std::ops::Add;

pub enum ClockEvent {
    Every100ms(f64),
}

struct PeriodicEvent {
    left: f64,
    period: f64,
    event: Event<Clock>,
}

impl PeriodicEvent {
    fn new(left: f64, period: f64, event: Event<Clock>) -> Self {
        PeriodicEvent { left, period, event }
    }

    fn notify(&self, clock: &Clock) {
        self.event.notify(clock)
    }

    fn reset(&mut self) {
        self.left += self.period;
    }
}

pub struct Clock {
    pub elapsed_seconds: f64,
    start_point: NaiveDateTime,
    periodic_events: VecDeque<PeriodicEvent>,
}

impl Clock {
    pub fn new(start_point: Option<NaiveDateTime>) -> Self {
        Clock {
            elapsed_seconds: 0.0,
            start_point: start_point.unwrap_or(Local::now().naive_local()),
            periodic_events: VecDeque::new(),
        }
    }

    pub fn tick(&mut self, dt: f64) {
        self.elapsed_seconds += dt;
    }

    pub fn current(&self) -> NaiveDateTime {
        let delta = TimeDelta::microseconds((self.elapsed_seconds * 1_000_000.0) as i64);
        self.start_point.add(delta)
    }

    pub fn datetime_to_elapsed_seconds(&self, dt: NaiveDateTime) -> f64 {
        (dt - self.start_point).num_microseconds().unwrap() as f64 / 1_000_000.0
    }

    pub fn subscribe_periodic_event(&mut self, period: f64, callback: fn(&Clock), start_at: Option<NaiveDateTime>) {
        let event = Event::new(callback);
        let left = match start_at {
            Some(start_at) => self.datetime_to_elapsed_seconds(start_at) - self.elapsed_seconds,
            None => period,
        };

        let idx = self.periodic_events.partition_point(|x| x.left < left);
        self.periodic_events
            .insert(idx, PeriodicEvent::new(left, period, event));
    }

    fn handle_periodic_events(&mut self, dt: f64) {
        let mut num_fired = None;
        for (idx, event) in self.periodic_events.iter_mut().enumerate() {
            event.left -= dt;
            if event.left <= 0.0 {
                num_fired = Some(idx);
            }
        }

        if let Some(idx) = num_fired {
            for _ in 0..=idx {
                let mut fired = self
                    .periodic_events
                    .pop_front()
                    .expect("There are at least num_fired items in the dequeue");
                fired.notify(self);
                fired.reset();

                let idx = self.periodic_events.partition_point(|x| x.left < fired.left);
                self.periodic_events.insert(idx, fired);
            }
        }
    }
}
