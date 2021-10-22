use std::collections::VecDeque;
use std::ops::Add;
use std::sync::atomic::{AtomicI64, AtomicU32, Ordering};
use std::sync::RwLock;
use std::time::{Duration, Instant};

pub struct Limiter {
    interval: Duration,
    capacity: AtomicU32,
    tokens: AtomicI64,
    window: RwLock<VecDeque<Instant>>,
}

impl Limiter {
    pub fn new(capacity: u32, tokens: i64, interval: Duration) -> Self {
        if capacity == 0 {
            panic!("Capacity must be positive!")
        }

        Self {
            interval,
            window: RwLock::new(VecDeque::new()),
            tokens: AtomicI64::new(tokens),
            capacity: AtomicU32::new(capacity),
        }
    }

    pub fn set_capacity(&self, new_cap: u32) {
        let old_cap = self.capacity.swap(new_cap, Ordering::SeqCst);
        self.tokens
            .fetch_add(i64::from(new_cap) - i64::from(old_cap), Ordering::SeqCst);
    }

    fn refill(&self) {
        let now = Instant::now();
        let mut window = self.window.write().unwrap();
        let num_expired = window.partition_point(|&i| i < now);
        window.drain(..num_expired);
        // Safe to unwrap as the VecDeque can never have more items then self.capacity
        // thus num_expired always fits into a u32 which can be safely converted into an i64
        self.tokens
            .fetch_add(num_expired.try_into().unwrap(), Ordering::SeqCst);
    }

    fn take(&self) {
        self.window
            .write()
            .unwrap()
            .push_back(Instant::now().add(self.interval));
        self.tokens.fetch_sub(1, Ordering::SeqCst);
    }

    pub fn wait(&self) {
        loop {
            self.refill();
            if self.tokens.load(Ordering::Acquire) > 0 {
                self.take();
                return;
            }
            let wait_time = match self.window.read().unwrap().get(0) {
                Some(future) => future.saturating_duration_since(Instant::now()),
                None => Duration::from_millis(100), // This should never happen
            };
            std::thread::sleep(wait_time);
        }
    }
}
