use std::collections::VecDeque;
use std::ops::Add;
use std::time::{Duration, Instant};

pub struct Limiter {
    interval: Duration,
    tokens: usize,
    window: VecDeque<Instant>,
}

impl Limiter {
    pub fn new(capacity: usize, interval: Duration) -> Self {
        if capacity == 0 {
            panic!("Capacity can't be zero!")
        }
        Self {
            interval,
            tokens: capacity,
            window: VecDeque::new(),
        }
    }

    pub fn refill(&mut self) {
        let now = Instant::now();
        let num_expired = self.window.partition_point(|&i| i < now);
        self.window = self.window.split_off(num_expired);
        self.tokens += num_expired;
    }

    pub fn take(&mut self) {
        self.window.push_back(Instant::now().add(self.interval));
        self.tokens -= 1
    }

    pub fn wait(&mut self) {
        loop {
            self.refill();
            if self.tokens == 0 {
                let wait_time = match self.window.get(0) {
                    Some(future) => future.saturating_duration_since(Instant::now()),
                    None => Duration::from_millis(100), // This should never happen
                };
                std::thread::sleep(wait_time);
            } else {
                self.take();
                return;
            }
        }
    }
}
