pub mod chat;
mod queue;

pub use chat::{Client, Config, SendResult};
pub use queue::{PushError, Queue};
use tokio::runtime::Handle;
use tracing::debug;

mod messages {
    pub const QUEUE_NOT_LOADED: &str = "No Queue selected";
    pub const QUEUE_CLOSED: &str = "Queue is currently closed";
    pub const QUEUE_CLOSE: &str = "Queue has been closed";
    pub const QUEUE_CLOSE_ERROR: &str = "Queue is already closed";
    pub const QUEUE_OPEN: &str = "Queue is now open";
    pub const QUEUE_OPEN_ERROR: &str = "Queue is already open";
    pub const QUEUE_CLEAR: &str = "Queue has been cleared";
    pub const QUEUE_EMPTY: &str = "The queue is currently empty";
    pub const PLAYER_HISTORY_RESET: &str = "Player history has been reset!";
}

pub struct Bot {
    pub chat: Client,
    pub queue: Option<Queue>,
}

impl Bot {
    pub fn new(rt: Handle, config: Config) -> Self {
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
            .send_msg(format!("Queue \"{name}\" has been created and selected"))
    }

    pub fn select(&mut self, name: &str) -> SendResult {
        match Queue::load(name) {
            Some(queue) => {
                self.queue = Some(queue);
                let name = &self.queue.as_ref().unwrap().name;
                Ok(self
                    .chat
                    .send_msg(format!("Queue \"{name}\" is now selected"))?)
            }
            None => Ok(self
                .chat
                .send_msg(format!("A queue named {name} doesn't exist"))?),
        }
    }

    pub fn save(&mut self) -> SendResult {
        match &self.queue {
            None => Ok(self.chat.send_msg(messages::QUEUE_NOT_LOADED.into())?),
            Some(queue) => {
                queue.save();
                self.chat.send_msg(format!("Queue {} saved", queue.name))?;
                Ok(())
            }
        }
    }

    pub fn open(&mut self) -> SendResult {
        match self.queue.as_mut() {
            None => Ok(self.chat.send_msg(messages::QUEUE_NOT_LOADED.into())?),
            Some(queue) => match queue.open() {
                Err(_) => Ok(self.chat.send_msg(messages::QUEUE_OPEN_ERROR.into())?),
                Ok(_) => Ok(self.chat.send_msg(messages::QUEUE_OPEN.into())?),
            },
        }
    }

    pub fn close(&mut self) -> SendResult {
        match self.queue.as_mut() {
            None => Ok(self.chat.send_msg(messages::QUEUE_NOT_LOADED.into())?),
            Some(queue) => match queue.close() {
                Err(_) => Ok(self.chat.send_msg(messages::QUEUE_CLOSE_ERROR.into())?),
                Ok(_) => Ok(self.chat.send_msg(messages::QUEUE_CLOSE.into())?),
            },
        }
    }

    pub fn clear(&mut self) -> SendResult {
        match self.queue.as_mut() {
            None => Ok(self.chat.send_msg(messages::QUEUE_NOT_LOADED.into())?),
            Some(queue) => {
                queue.clear();
                self.chat.send_msg(messages::QUEUE_CLEAR.into())
            }
        }
    }

    pub fn join(&mut self, user: &str) -> SendResult {
        match self.queue.as_mut() {
            None => Ok(self.chat.send_msg(messages::QUEUE_NOT_LOADED.into())?),
            Some(queue) => {
                if queue.is_open {
                    match queue.push(user) {
                        Err(PushError::Played) => self.chat.send_msg(format!(
                            "@{user}: You've already played. Wait until queue reset to join again.",
                        )),
                        Err(PushError::Present(idx)) => self.chat.send_msg(format!(
                            "@{user}: You're already in queue at position {}",
                            idx + 1
                        )),
                        Ok(idx) => self.chat.send_msg(format!(
                            "@{user}: You've been added to the queue at position {}",
                            idx + 1
                        )),
                    }
                } else {
                    Ok(self.chat.send_msg(messages::QUEUE_CLOSED.into())?)
                }
            }
        }
    }

    pub fn leave(&mut self, user: &str) -> SendResult {
        match self.queue.as_mut() {
            None => Ok(self.chat.send_msg(messages::QUEUE_NOT_LOADED.into())?),
            Some(queue) => match queue.remove(user) {
                Ok(_) => self
                    .chat
                    .send_msg(format!("@{user}: You've been removed from the queue")),
                Err(_) => self.chat.send_msg(format!("@{user}: You were not queued")),
            },
        }
    }

    pub fn reset(&mut self) -> SendResult {
        match self.queue.as_mut() {
            None => Ok(self.chat.send_msg(messages::QUEUE_NOT_LOADED.into())?),
            Some(queue) => {
                queue.reset();
                self.chat.send_msg(messages::PLAYER_HISTORY_RESET.into())
            }
        }
    }

    pub fn next(&mut self) -> SendResult {
        match self.queue.as_mut() {
            None => Ok(self.chat.send_msg(messages::QUEUE_NOT_LOADED.into())?),
            Some(queue) => match queue.shift() {
                None => self.chat.send_msg(messages::QUEUE_EMPTY.into()),
                Some(user) => {
                    let next_msg = format!("@{user} is next!");
                    match queue.first() {
                        None => self
                            .chat
                            .send_msg(format!("{next_msg} That's the last one.")),
                        Some(user) => self
                            .chat
                            .send_msg(format!("{next_msg} @{user} is up after that.")),
                    }
                }
            },
        }
    }

    pub fn position(&self, user: &str) -> SendResult {
        match &self.queue {
            None => Ok(self.chat.send_msg(messages::QUEUE_NOT_LOADED.into())?),
            Some(queue) => match queue.find(user) {
                Some(idx) => self
                    .chat
                    .send_msg(format!("@{user} you are number {} in queue", idx + 1)),
                None => self
                    .chat
                    .send_msg(format!("@{user}: You're not currently queued")),
            },
        }
    }

    pub fn length(&self) -> SendResult {
        match &self.queue {
            None => Ok(self.chat.send_msg(messages::QUEUE_NOT_LOADED.into())?),
            Some(queue) => self
                .chat
                .send_msg(format!("There are {} people in queue", queue.len())),
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
            None => Ok(self.chat.send_msg(messages::QUEUE_NOT_LOADED.into())?),
            Some(queue) => {
                const MAX_LIST: usize = 5;
                let l = queue.list();
                println!("Logging full list: {l:?}");
                match l.len() {
                    0 => self.chat.send_msg(messages::QUEUE_EMPTY.into()),
                    1..=MAX_LIST => self
                        .chat
                        .send_msg(format!("People in queue: {}", format_list(l))),
                    n => self.chat.send_msg(format!(
                        "People in queue (first {MAX_LIST} out of {n}): {}",
                        format_list(&l[..MAX_LIST])
                    )),
                }
            }
        }
    }
}
