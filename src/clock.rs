use chrono::{Local, NaiveDateTime, NaiveTime, TimeDelta};
use std::collections::VecDeque;
use std::ops::Add;

#[derive(Copy, Clone)]
pub enum ClockEvent {
    TrainInfoUpdate,
    ClockUpdate,
    SpeedTableScroll,
    SpeedTableTailClean,
}

pub struct ClockPayload {
    pub event: ClockEvent,
    pub elapsed_time: f64,
    pub current_time: NaiveDateTime,
}

struct PeriodicEvent {
    left: f64,
    period: f64,
    event: ClockEvent,
}

pub struct Clock {
    elapsed_seconds: f64,
    start_point: NaiveDateTime,
    periodic_events: VecDeque<PeriodicEvent>,
}

impl Clock {
    pub fn new(start_point: Option<NaiveDateTime>) -> Self {
        let default = Local::now().with_time(NaiveTime::default()).unwrap();
        Clock {
            elapsed_seconds: 0.0,
            start_point: start_point.unwrap_or(default.naive_local()),
            periodic_events: VecDeque::new(),
        }
    }

    pub fn tick(&mut self, dt: f64) -> Vec<ClockPayload> {
        self.elapsed_seconds += dt;
        self.handle_periodic_events(dt)
    }

    pub fn current(&self) -> NaiveDateTime {
        let delta = TimeDelta::microseconds((self.elapsed_seconds * 1_000_000.0) as i64);
        self.start_point.add(delta)
    }

    pub fn datetime_to_elapsed_seconds(&self, dt: NaiveDateTime) -> f64 {
        (dt - self.start_point).num_microseconds().unwrap() as f64 / 1_000_000.0
    }

    pub fn subscribe_event(&mut self, event: ClockEvent, period: f64, start_at: Option<NaiveDateTime>) {
        let left = match start_at {
            Some(start_at) => self.datetime_to_elapsed_seconds(start_at) - self.elapsed_seconds,
            None => period,
        };

        let idx = self.periodic_events.partition_point(|x| x.left < left);
        self.periodic_events.insert(idx, PeriodicEvent { left, period, event });
    }

    fn handle_periodic_events(&mut self, dt: f64) -> Vec<ClockPayload> {
        let mut num_fired = None;
        for (idx, event) in self.periodic_events.iter_mut().enumerate() {
            event.left -= dt;
            if event.left <= 0.0 {
                num_fired = Some(idx);
            }
        }

        if let Some(num_fired) = num_fired {
            let mut result = Vec::with_capacity(num_fired + 1);
            let current_time = self.current();
            for _ in 0..=num_fired {
                let mut fired = self
                    .periodic_events
                    .pop_front()
                    .expect("There are at least num_fired items in the deque");
                result.push(ClockPayload {
                    event: fired.event,
                    elapsed_time: self.elapsed_seconds + fired.left,
                    current_time,
                });
                fired.left += fired.period;

                let idx = self.periodic_events.partition_point(|x| x.left < fired.left);
                self.periodic_events.insert(idx, fired);
            }
            result
        } else {
            Vec::default()
        }
    }
}
