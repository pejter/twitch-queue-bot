pub mod chat;
mod irc;
mod queue;
mod ratelimit;

pub use chat::{Client, Config, SendResult};
pub use queue::{PushError, Queue};
use tokio::runtime::Runtime;
use tracing::debug;

mod messages {
    pub const QUEUE_NOT_LOADED: &str = "No Queue selected";
    pub const QUEUE_CLOSED: &str = "Queue is currently closed";
}

pub struct Bot {
    pub chat: Client,
    pub queue: Option<Queue>,
}

impl Bot {
    pub fn new(rt: &Runtime, config: Config) -> Self {
        debug!("Creating data dir {}", queue::DATA_DIR);
        std::fs::DirBuilder::new()
            .recursive(true)
            .create(queue::DATA_DIR)
            .unwrap();

        debug!("Creating bot");
        Self {
            chat: Client::new(rt, config),
            queue: None,
        }
    }

    pub fn create(&mut self, name: &str) -> SendResult {
        self.queue = Some(Queue::new(name));
        self.chat
            .send_msg(&format!("Queue \"{name}\" has been created and selected"))
    }

    pub fn select(&mut self, name: &str) -> SendResult {
        match Queue::load(name) {
            Some(queue) => {
                self.queue = Some(queue);
                let name = &self.queue.as_ref().unwrap().name;
                Ok(self
                    .chat
                    .send_msg(&format!("Queue \"{name}\" is now selected"))?)
            }
            None => Ok(self
                .chat
                .send_msg(&format!("A queue named {} doesn't exist", name))?),
        }
    }

    pub fn save(&mut self) -> SendResult {
        match &self.queue {
            None => Ok(self.chat.send_msg(messages::QUEUE_NOT_LOADED)?),
            Some(queue) => {
                queue.save();
                self.chat.send_msg(&format!("Queue {} saved", queue.name))?;
                Ok(())
            }
        }
    }

    pub fn open(&mut self) -> SendResult {
        match self.queue.as_mut() {
            None => Ok(self.chat.send_msg(messages::QUEUE_NOT_LOADED)?),
            Some(queue) => match queue.open() {
                Err(_) => Ok(self.chat.send_msg("Queue is already open")?),
                Ok(_) => Ok(self.chat.send_msg("Queue is now open")?),
            },
        }
    }

    pub fn close(&mut self) -> SendResult {
        match self.queue.as_mut() {
            None => Ok(self.chat.send_msg(messages::QUEUE_NOT_LOADED)?),
            Some(queue) => match queue.close() {
                Err(_) => Ok(self.chat.send_msg("Queue is already closed")?),
                Ok(_) => Ok(self.chat.send_msg("Queue has been closed")?),
            },
        }
    }

    pub fn clear(&mut self) -> SendResult {
        match self.queue.as_mut() {
            None => Ok(self.chat.send_msg(messages::QUEUE_NOT_LOADED)?),
            Some(queue) => {
                queue.clear();
                self.chat.send_msg("Queue has been cleared")
            }
        }
    }

    pub fn join(&mut self, user: &str) -> SendResult {
        match self.queue.as_mut() {
            None => Ok(self.chat.send_msg(messages::QUEUE_NOT_LOADED)?),
            Some(queue) => {
                if queue.is_open {
                    match queue.push(user) {
                        Err(PushError::Played) => self.chat.send_msg(&format!(
                            "@{user}: You've already played. Wait until queue reset to join again.",
                        )),
                        Err(PushError::Present(idx)) => self.chat.send_msg(&format!(
                            "@{user}: You're already in queue at position {}",
                            idx + 1
                        )),
                        Ok(idx) => self.chat.send_msg(&format!(
                            "@{user}: You've been added to the queue at position {}",
                            idx + 1
                        )),
                    }
                } else {
                    Ok(self.chat.send_msg(messages::QUEUE_CLOSED)?)
                }
            }
        }
    }

    pub fn leave(&mut self, user: &str) -> SendResult {
        match self.queue.as_mut() {
            None => Ok(self.chat.send_msg(messages::QUEUE_NOT_LOADED)?),
            Some(queue) => match queue.remove(user) {
                Ok(_) => self
                    .chat
                    .send_msg(&format!("@{user}: You've been removed from the queue")),
                Err(_) => self.chat.send_msg(&format!("@{user}: You were not queued")),
            },
        }
    }

    pub fn reset(&mut self) -> SendResult {
        match self.queue.as_mut() {
            None => Ok(self.chat.send_msg(messages::QUEUE_NOT_LOADED)?),
            Some(queue) => {
                queue.reset();
                self.chat.send_msg("Player history has been reset!")
            }
        }
    }

    pub fn next(&mut self) -> SendResult {
        match self.queue.as_mut() {
            None => Ok(self.chat.send_msg(messages::QUEUE_NOT_LOADED)?),
            Some(queue) => match queue.shift() {
                None => self.chat.send_msg("The queue is currently empty"),
                Some(user) => {
                    let next_msg = format!("@{user} is next!");
                    match queue.first() {
                        None => self
                            .chat
                            .send_msg(&format!("{next_msg} That's the last one.")),
                        Some(user) => self
                            .chat
                            .send_msg(&format!("{next_msg} @{user} is up after that.")),
                    }
                }
            },
        }
    }

    pub fn position(&self, user: &str) -> SendResult {
        match &self.queue {
            None => Ok(self.chat.send_msg(messages::QUEUE_NOT_LOADED)?),
            Some(queue) => match queue.find(user) {
                Some(idx) => self
                    .chat
                    .send_msg(&format!("@{user} you are number {} in queue", idx + 1)),
                None => self
                    .chat
                    .send_msg(&format!("@{user}: You're not currently queued")),
            },
        }
    }

    pub fn length(&self) -> SendResult {
        match &self.queue {
            None => Ok(self.chat.send_msg(messages::QUEUE_NOT_LOADED)?),
            Some(queue) => self
                .chat
                .send_msg(&format!("There are {} people in queue", queue.len())),
        }
    }

    pub fn list(&self) -> SendResult {
        fn format_list<T: AsRef<str> + std::fmt::Display>(l: &[T]) -> String {
            l.iter()
                .enumerate()
                .map(|(i, s)| format!("[{}. {}]", i + 1, s))
                .collect::<Vec<_>>()
                .join(", ")
        }

        match &self.queue {
            None => Ok(self.chat.send_msg(messages::QUEUE_NOT_LOADED)?),
            Some(queue) => {
                const MAX_LIST: usize = 5;
                let l = queue.list();
                println!("Logging full list: {l:?}");
                match l.len() {
                    0 => self.chat.send_msg("The queue is currently empty"),
                    1..=MAX_LIST => self
                        .chat
                        .send_msg(&format!("People in queue: {}", format_list(l))),
                    n => self.chat.send_msg(&format!(
                        "People in queue (first {MAX_LIST} out of {n}): {}",
                        format_list(&l[..MAX_LIST])
                    )),
                }
            }
        }
    }
}
