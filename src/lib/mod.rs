pub mod chat;
mod queue;
mod ratelimit;

pub use chat::{ChannelResult, ChatClient, ChatConfig};
pub use queue::Queue;
use std::error::Error;
use std::io;

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

    pub fn reconnect(&mut self) -> io::Result<()> {
        let config = self.chat.config();
        self.chat = ChatClient::connect(config)?;
        Ok(())
    }

    pub fn create(&mut self, name: &str) -> ChannelResult {
        match self.queue {
            Some(_) => Ok(self
                .chat
                .send_msg("A queue is currently open, close it to create more queues")?),
            None => {
                Queue::new(name);
                Ok(self.chat.send_msg(&format!(
                    "Queue \"{}\" has been created and is now open",
                    name
                ))?)
            }
        }
    }

    pub fn open(&mut self, name: &str) -> ChannelResult {
        match self.queue {
            Some(_) => Ok(self.chat.send_msg("Another queue is currently open")?),
            None => match Queue::load(name) {
                Some(queue) => {
                    self.queue = Some(queue);
                    Ok(self.chat.send_msg(&format!(
                        "Queue \"{}\" is now open",
                        self.queue.as_ref().unwrap().name
                    ))?)
                }
                None => Ok(self
                    .chat
                    .send_msg(&format!("A queue named {} doesn't exist", name))?),
            },
        }
    }

    pub fn close(&mut self) -> ChannelResult {
        match &self.queue {
            None => Ok(self.chat.send_msg("Queue is currently closed")?),
            Some(queue) => {
                self.chat.send_msg("Queue closed")?;
                queue.save();
                self.queue = None;
                Ok(())
            }
        }
    }

    pub fn clear(&mut self) -> ChannelResult {
        match self.queue.as_mut() {
            None => Ok(self.chat.send_msg("Queue is currently closed")?),
            Some(queue) => {
                queue.clear();
                self.chat.send_msg("Queue has been cleared")
            }
        }
    }

    pub fn join(&mut self, user: &str) -> ChannelResult {
        match self.queue.as_mut() {
            None => Ok(self.chat.send_msg("Queue is currently closed")?),
            Some(queue) => match queue.push(user) {
                (false, idx) => self.chat.send_msg(&format!(
                    "@{}: You're already in queue at position {}",
                    user,
                    idx + 1
                )),
                (true, idx) => self.chat.send_msg(&format!(
                    "@{}: You've been added to the queue at position {}",
                    user,
                    idx + 1
                )),
            },
        }
    }

    pub fn leave(&mut self, user: &str) -> ChannelResult {
        match self.queue.as_mut() {
            None => Ok(self.chat.send_msg("Queue is currently closed")?),
            Some(queue) => match queue.remove(user) {
                true => self
                    .chat
                    .send_msg(&format!("@{}: You've been removed from the queue", user)),
                false => self
                    .chat
                    .send_msg(&format!("@{}: You were not queued", user)),
            },
        }
    }

    pub fn next(&mut self) -> ChannelResult {
        match self.queue.as_mut() {
            None => Ok(self.chat.send_msg("Queue is currently closed")?),
            Some(queue) => match queue.shift() {
                None => self.chat.send_msg("The queue is currently empty"),
                Some(user) => {
                    self.chat.send_msg(&format!("@{}: It's your turn!", user))?;
                    match queue.first() {
                        None => self
                            .chat
                            .send_msg("That was the last one. No more people in the queue"),
                        Some(user) => self
                            .chat
                            .send_msg(&format!("@{}: You're now first in the queue", user)),
                    }
                }
            },
        }
    }

    pub fn position(&self, user: &str) -> ChannelResult {
        match &self.queue {
            None => Ok(self.chat.send_msg("Queue is currently closed")?),
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
            None => Ok(self.chat.send_msg("Queue is currently closed")?),
            Some(queue) => self
                .chat
                .send_msg(&format!("There are {} people in queue", queue.len())),
        }
    }

    pub fn list(&self) -> ChannelResult {
        match &self.queue {
            None => Ok(self.chat.send_msg("Queue is currently closed")?),
            Some(queue) => {
                let l = queue.list();
                println!("Logging full list: {:?}", l);
                const MAX_LIST: usize = 5;
                match l.len() {
                    0 => self.chat.send_msg("The queue is currently empty"),
                    1..=MAX_LIST => self
                        .chat
                        .send_msg(&format!("People in queue: {}", l.join(", "))),
                    n => self.chat.send_msg(&format!(
                        "People in queue (first {} out of {}): {}",
                        MAX_LIST,
                        n,
                        l[..MAX_LIST].join(", ")
                    )),
                }
            }
        }
    }
}
