pub mod chat;
mod queue;
mod ratelimit;

pub use chat::{ChannelResult, ChatClient, ChatConfig};
pub use queue::{PushError, Queue};
use std::error::Error;

mod messages {
    pub const QUEUE_NOT_LOADED: &str = "No Queue is currently loaded";
    pub const QUEUE_CLOSED: &str = "Queue is currently closed";
}

pub struct Bot {
    pub chat: ChatClient,
    pub queue: Option<Queue>,
}

impl Bot {
    pub fn new(config: ChatConfig) -> Result<Self, Box<dyn Error>> {
        std::fs::DirBuilder::new()
            .recursive(true)
            .create(queue::DATA_DIR)
            .unwrap();

        Ok(Self {
            chat: ChatClient::connect(config)?,
            queue: None,
        })
    }

    pub fn create(&mut self, name: &str) -> ChannelResult {
        self.queue = Some(Queue::new(name));
        self.chat
            .send_msg(&format!("Queue \"{}\" has been created and loaded", name))
    }

    pub fn unload(&mut self) -> ChannelResult {
        self.queue = None;
        self.chat.send_msg("Queue has been unloaded")
    }

    pub fn load(&mut self, name: &str) -> ChannelResult {
        match Queue::load(name) {
            Some(queue) => {
                self.queue = Some(queue);
                Ok(self.chat.send_msg(&format!(
                    "Queue \"{}\" is now loaded",
                    self.queue.as_ref().unwrap().name
                ))?)
            }
            None => Ok(self
                .chat
                .send_msg(&format!("A queue named {} doesn't exist", name))?),
        }
    }

    pub fn save(&mut self) -> ChannelResult {
        match &self.queue {
            None => Ok(self.chat.send_msg(messages::QUEUE_NOT_LOADED)?),
            Some(queue) => {
                queue.save();
                self.chat.send_msg(&format!("Queue {} saved", queue.name))?;
                Ok(())
            }
        }
    }

    pub fn open(&mut self) -> ChannelResult {
        match self.queue.as_mut() {
            None => Ok(self.chat.send_msg(messages::QUEUE_NOT_LOADED)?),
            Some(queue) => match queue.open() {
                Err(_) => Ok(self.chat.send_msg("Queue is already open")?),
                Ok(_) => Ok(self.chat.send_msg("Queue is now open")?),
            },
        }
    }

    pub fn close(&mut self) -> ChannelResult {
        match self.queue.as_mut() {
            None => Ok(self.chat.send_msg(messages::QUEUE_NOT_LOADED)?),
            Some(queue) => match queue.close() {
                Err(_) => Ok(self.chat.send_msg("Queue is already closed")?),
                Ok(_) => Ok(self.chat.send_msg("Queue has been closed")?),
            },
        }
    }

    pub fn clear(&mut self) -> ChannelResult {
        match self.queue.as_mut() {
            None => Ok(self.chat.send_msg(messages::QUEUE_NOT_LOADED)?),
            Some(queue) => {
                queue.clear();
                self.chat.send_msg("Queue has been cleared")
            }
        }
    }

    pub fn join(&mut self, user: &str) -> ChannelResult {
        match self.queue.as_mut() {
            None => Ok(self.chat.send_msg(messages::QUEUE_NOT_LOADED)?),
            Some(queue) => match queue.is_open {
                false => Ok(self.chat.send_msg(messages::QUEUE_CLOSED)?),
                true => match queue.push(user) {
                    Err(PushError::Played) => self.chat.send_msg(&format!(
                        "@{}: You've already played. Wait until queue reset to join again.",
                        user,
                    )),
                    Err(PushError::Present(idx)) => self.chat.send_msg(&format!(
                        "@{}: You're already in queue at position {}",
                        user,
                        idx + 1
                    )),
                    Ok(idx) => self.chat.send_msg(&format!(
                        "@{}: You've been added to the queue at position {}",
                        user,
                        idx + 1
                    )),
                },
            },
        }
    }

    pub fn leave(&mut self, user: &str) -> ChannelResult {
        match self.queue.as_mut() {
            None => Ok(self.chat.send_msg(messages::QUEUE_NOT_LOADED)?),
            Some(queue) => match queue.remove(user) {
                Ok(_) => self
                    .chat
                    .send_msg(&format!("@{}: You've been removed from the queue", user)),
                Err(_) => self
                    .chat
                    .send_msg(&format!("@{}: You were not queued", user)),
            },
        }
    }

    pub fn reset(&mut self) -> ChannelResult {
        match self.queue.as_mut() {
            None => Ok(self.chat.send_msg(messages::QUEUE_NOT_LOADED)?),
            Some(queue) => {
                queue.reset();
                self.chat.send_msg("Player history has been reset!")
            }
        }
    }

    pub fn next(&mut self) -> ChannelResult {
        match self.queue.as_mut() {
            None => Ok(self.chat.send_msg(messages::QUEUE_NOT_LOADED)?),
            Some(queue) => match queue.shift() {
                None => self.chat.send_msg("The queue is currently empty"),
                Some(user) => {
                    let next_msg = format!("@{} is next!", user);
                    match queue.first() {
                        None => self
                            .chat
                            .send_msg(&format!("{} That's the last one.", next_msg)),
                        Some(user) => self
                            .chat
                            .send_msg(&format!("{} @{} is up after that.", next_msg, user)),
                    }
                }
            },
        }
    }

    pub fn position(&self, user: &str) -> ChannelResult {
        match &self.queue {
            None => Ok(self.chat.send_msg(messages::QUEUE_NOT_LOADED)?),
            Some(queue) => match queue.find(user) {
                Some(idx) => {
                    self.chat
                        .send_msg(&format!("@{} you are number {} in queue", user, idx + 1))
                }
                None => self
                    .chat
                    .send_msg(&format!("@{}: You're not currently queued", user)),
            },
        }
    }

    pub fn length(&self) -> ChannelResult {
        match &self.queue {
            None => Ok(self.chat.send_msg(messages::QUEUE_NOT_LOADED)?),
            Some(queue) => self
                .chat
                .send_msg(&format!("There are {} people in queue", queue.len())),
        }
    }

    pub fn list(&self) -> ChannelResult {
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
                let l = queue.list();
                println!("Logging full list: {:?}", l);
                const MAX_LIST: usize = 5;
                match l.len() {
                    0 => self.chat.send_msg("The queue is currently empty"),
                    1..=MAX_LIST => self
                        .chat
                        .send_msg(&format!("People in queue: {}", format_list(l))),
                    n => self.chat.send_msg(&format!(
                        "People in queue (first {} out of {}): {}",
                        MAX_LIST,
                        n,
                        format_list(&l[..MAX_LIST])
                    )),
                }
            }
        }
    }
}
