use std::collections::VecDeque;
use std::ops::Add;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::RwLock;
use std::time::{Duration, Instant};

pub struct Limiter {
    interval: Duration,
    capacity: AtomicUsize,
    tokens: AtomicUsize,
    window: RwLock<VecDeque<Instant>>,
}

impl Limiter {
    pub fn new(capacity: usize, tokens: usize, interval: Duration) -> Self {
        if capacity == 0 {
            panic!("Capacity can't be zero!")
        }

        Self {
            interval,
            window: RwLock::new(VecDeque::new()),
            tokens: AtomicUsize::new(tokens),
            capacity: AtomicUsize::new(capacity),
        }
    }

    pub fn set_capacity(&self, new_cap: usize) {
        let old_cap = self.capacity.swap(new_cap, Ordering::SeqCst);
        self.tokens.fetch_add(new_cap - old_cap, Ordering::SeqCst);
    }

    fn refill(&self) {
        let now = Instant::now();
        let mut window = self.window.write().unwrap();
        let num_expired = window.partition_point(|&i| i < now);
        window.drain(..num_expired);
        // This might panic if we ever get unmodded while sending a lot of messages
        self.tokens.fetch_add(num_expired, Ordering::SeqCst);
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
