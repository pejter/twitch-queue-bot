use std::ops::Add;
use std::time::{Duration, Instant};

pub struct Limiter {
    interval: Duration,
    tokens: u64,
    capacity: u64,
    last: Option<Instant>,
}

impl Limiter {
    pub fn new(capacity: u64, interval: Duration) -> Self {
        if capacity == 0 {
            panic!("Capacity can't be zero!")
        }
        Self {
            interval,
            capacity,
            tokens: capacity,
            last: None,
        }
    }

    pub fn refill(&mut self) {
        if let Some(last) = self.last {
            let since = Instant::now().saturating_duration_since(last);
            if since > self.interval {
                self.tokens = self.capacity;
                self.last = None;
            }
        }
    }

    fn start_timeout(&mut self) {
        if self.last == None {
            self.last = Some(Instant::now());
        }
    }

    pub fn wait(&self) {
        if let Some(last) = self.last {
            let until_next = last
                .add(self.interval)
                .saturating_duration_since(Instant::now());
            std::thread::sleep(until_next)
        }
    }

    pub fn take(&mut self) {
        loop {
            self.refill();
            self.start_timeout();
            if self.tokens == 0 {
                self.wait();
            } else {
                return self.tokens -= 1;
            }
        }
    }
}
