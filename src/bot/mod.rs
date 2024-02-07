pub mod chat;
mod queue;

pub use chat::{Client as ChatClient, Config, Message, SendError, SendResult};
pub use queue::{PushError, Queue};
use tracing::{debug, warn};

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
    pub chat: ChatClient,
    pub queue: Option<Queue>,
}

impl Bot {
    pub fn new(config: Config) -> Self {
        debug!("Creating data dir {}", queue::DATA_DIR);
        std::fs::DirBuilder::new()
            .recursive(true)
            .create(queue::DATA_DIR)
            .unwrap();

        debug!("Creating bot");
        Self {
            chat: ChatClient::new(config),
            queue: None,
        }
    }

    pub async fn recv_msg(&mut self) -> Option<Message> {
        self.chat.recv_msg().await
    }

    pub async fn send_msg(&self, msg: String) -> SendResult {
        match self.chat.send_msg(msg).await {
            Err(SendError::ClientClosed) => {
                warn!("Client has been closed, will not be sent");
                Ok(())
            }
            other => other,
        }
    }

    pub async fn create(&mut self, name: &str) -> SendResult {
        self.queue = Some(Queue::new(name));
        self.send_msg(format!("Queue \"{name}\" has been created and selected"))
            .await
    }

    pub async fn select(&mut self, name: &str) -> SendResult {
        match Queue::load(name) {
            Some(queue) => {
                self.queue = Some(queue);
                let name = &self.queue.as_ref().unwrap().name;
                Ok(self
                    .send_msg(format!("Queue \"{name}\" is now selected"))
                    .await?)
            }
            None => Ok(self
                .send_msg(format!("A queue named {name} doesn't exist"))
                .await?),
        }
    }

    pub async fn save(&mut self) -> SendResult {
        match &self.queue {
            None => Ok(self.send_msg(messages::QUEUE_NOT_LOADED.into()).await?),
            Some(queue) => {
                queue.save();
                self.send_msg(format!("Queue {} saved", queue.name)).await?;
                Ok(())
            }
        }
    }

    pub async fn open(&mut self) -> SendResult {
        match self.queue.as_mut() {
            None => Ok(self.send_msg(messages::QUEUE_NOT_LOADED.into()).await?),
            Some(queue) => match queue.open() {
                Err(_) => Ok(self.send_msg(messages::QUEUE_OPEN_ERROR.into()).await?),
                Ok(_) => Ok(self.send_msg(messages::QUEUE_OPEN.into()).await?),
            },
        }
    }

    pub async fn close(&mut self) -> SendResult {
        match self.queue.as_mut() {
            None => Ok(self.send_msg(messages::QUEUE_NOT_LOADED.into()).await?),
            Some(queue) => match queue.close() {
                Err(_) => Ok(self.send_msg(messages::QUEUE_CLOSE_ERROR.into()).await?),
                Ok(_) => Ok(self.send_msg(messages::QUEUE_CLOSE.into()).await?),
            },
        }
    }

    pub async fn clear(&mut self) -> SendResult {
        match self.queue.as_mut() {
            None => Ok(self.send_msg(messages::QUEUE_NOT_LOADED.into()).await?),
            Some(queue) => {
                queue.clear();
                self.send_msg(messages::QUEUE_CLEAR.into()).await
            }
        }
    }

    pub async fn join(&mut self, user: &str) -> SendResult {
        match self.queue.as_mut() {
            None => Ok(self.send_msg(messages::QUEUE_NOT_LOADED.into()).await?),
            Some(queue) => {
                if queue.is_open {
                    match queue.push(user) {
                        Err(PushError::Played) => {
                            self.send_msg(format!(
                            "@{user}: You've already played. Wait until queue reset to join again.",
                        ))
                            .await
                        }
                        Err(PushError::Present(idx)) => {
                            self.send_msg(format!(
                                "@{user}: You're already in queue at position {}",
                                idx + 1
                            ))
                            .await
                        }
                        Ok(idx) => {
                            self.send_msg(format!(
                                "@{user}: You've been added to the queue at position {}",
                                idx + 1
                            ))
                            .await
                        }
                    }
                } else {
                    Ok(self.send_msg(messages::QUEUE_CLOSED.into()).await?)
                }
            }
        }
    }

    pub async fn leave(&mut self, user: &str) -> SendResult {
        match self.queue.as_mut() {
            None => Ok(self.send_msg(messages::QUEUE_NOT_LOADED.into()).await?),
            Some(queue) => match queue.remove(user) {
                Ok(_) => {
                    self.send_msg(format!("@{user}: You've been removed from the queue"))
                        .await
                }
                Err(_) => self.send_msg(format!("@{user}: You were not queued")).await,
            },
        }
    }

    pub async fn reset(&mut self) -> SendResult {
        match self.queue.as_mut() {
            None => Ok(self.send_msg(messages::QUEUE_NOT_LOADED.into()).await?),
            Some(queue) => {
                queue.reset();
                self.send_msg(messages::PLAYER_HISTORY_RESET.into()).await
            }
        }
    }

    pub async fn next(&mut self) -> SendResult {
        match self.queue.as_mut() {
            None => Ok(self.send_msg(messages::QUEUE_NOT_LOADED.into()).await?),
            Some(queue) => match queue.shift() {
                None => self.send_msg(messages::QUEUE_EMPTY.into()).await,
                Some(user) => {
                    let next_msg = format!("@{user} is next!");
                    match queue.first() {
                        None => {
                            self.send_msg(format!("{next_msg} That's the last one."))
                                .await
                        }
                        Some(user) => {
                            let user = user.to_owned();
                            self.send_msg(format!("{next_msg} @{user} is up after that."))
                                .await
                        }
                    }
                }
            },
        }
    }

    pub async fn position(&self, user: &str) -> SendResult {
        match &self.queue {
            None => Ok(self.send_msg(messages::QUEUE_NOT_LOADED.into()).await?),
            Some(queue) => match queue.find(user) {
                Some(idx) => {
                    self.send_msg(format!("@{user} you are number {} in queue", idx + 1))
                        .await
                }
                None => {
                    self.send_msg(format!("@{user}: You're not currently queued"))
                        .await
                }
            },
        }
    }

    pub async fn length(&self) -> SendResult {
        match &self.queue {
            None => Ok(self.send_msg(messages::QUEUE_NOT_LOADED.into()).await?),
            Some(queue) => {
                self.send_msg(format!("There are {} people in queue", queue.len()))
                    .await
            }
        }
    }

    pub async fn list(&self) -> SendResult {
        fn format_list<T: AsRef<str> + std::fmt::Display>(l: &[T]) -> String {
            l.iter()
                .enumerate()
                .map(|(i, s)| format!("[{}. {}]", i + 1, s))
                .collect::<Vec<_>>()
                .join(", ")
        }

        match &self.queue {
            None => Ok(self.send_msg(messages::QUEUE_NOT_LOADED.into()).await?),
            Some(queue) => {
                const MAX_LIST: usize = 5;
                let l = queue.list();
                println!("Logging full list: {l:?}");
                match l.len() {
                    0 => self.send_msg(messages::QUEUE_EMPTY.into()).await,
                    1..=MAX_LIST => {
                        self.send_msg(format!("People in queue: {}", format_list(l)))
                            .await
                    }
                    n => {
                        self.send_msg(format!(
                            "People in queue (first {MAX_LIST} out of {n}): {}",
                            format_list(&l[..MAX_LIST])
                        ))
                        .await
                    }
                }
            }
        }
    }
}
