pub mod chat;
mod ratelimit;

pub use chat::{ChannelError, ChatClient, ChatConfig};
use std::error::Error;

pub struct Bot {
    pub chat: ChatClient,
    pub queue: Vec<String>,
}

impl Bot {
    pub fn new(config: ChatConfig) -> Result<Self, Box<dyn Error>> {
        Ok(Self {
            chat: ChatClient::connect(config)?,
            queue: Vec::new(),
        })
    }

    pub fn reconnect(&mut self) -> Result<(), Box<dyn Error>> {
        let config = self.chat.config();
        self.chat = ChatClient::connect(config)?;
        Ok(())
    }

    pub fn clear(&mut self) -> Result<(), ChannelError> {
        self.queue.clear();
        self.chat.send_msg("Queue has been cleared")
    }

    pub fn push(&mut self, user: &str) -> Result<(), ChannelError> {
        match self.queue.iter().position(|x| x == user) {
            Some(idx) => self.chat.send_msg(&format!(
                "@{}: You're already in queue at position {}",
                user,
                idx + 1
            )),
            None => {
                self.queue.push(user.to_owned());
                self.chat.send_msg(&format!(
                    "@{}: You've been added to the queue at position {}",
                    user,
                    self.queue.len()
                ))
            }
        }
    }

    pub fn remove(&mut self, user: &str) -> Result<(), ChannelError> {
        match self.queue.iter().position(|x| x == user) {
            Some(idx) => {
                self.queue.remove(idx);
                self.chat
                    .send_msg(&format!("@{}: You've been removed from the queue", user))
            }
            None => self
                .chat
                .send_msg(&format!("@{}: You were not queued", user)),
        }
    }

    pub fn shift(&mut self) -> Result<(), ChannelError> {
        match self.queue.is_empty() {
            true => self.chat.send_msg("The queue is currently empty"),
            false => {
                self.chat
                    .send_msg(&format!("@{}: It's your turn!", self.queue.remove(0)))?;
                match self.queue.first() {
                    None => self
                        .chat
                        .send_msg("That was the last one. No more people in the queue"),
                    Some(user) => self
                        .chat
                        .send_msg(&format!("@{}: You're now first in the queue", user)),
                }
            }
        }
    }

    pub fn find(&self, user: &str) -> Result<(), ChannelError> {
        match self.queue.iter().position(|x| x == user) {
            Some(idx) => {
                self.chat
                    .send_msg(&format!("@{} you are number {} in queue", user, idx + 1))
            }
            None => self
                .chat
                .send_msg(&format!("@{}: You're not currently queued", user)),
        }
    }

    pub fn length(&self) -> Result<(), ChannelError> {
        self.chat
            .send_msg(&format!("There are {} people in queue", self.queue.len()))
    }

    pub fn list(&self) -> Result<(), ChannelError> {
        const MAX_LIST: usize = 5;
        match self.queue.len() {
            0 => self.chat.send_msg("The queue is currently empty"),
            1..=MAX_LIST => self.chat.send_msg(&format!(
                "People in queue: {}",
                self.queue.as_slice().join(", ")
            )),
            n => self.chat.send_msg(&format!(
                "People in queue (first {} out of {}): {}",
                MAX_LIST,
                n,
                self.queue[..MAX_LIST].join(", ")
            )),
        }
    }
}
