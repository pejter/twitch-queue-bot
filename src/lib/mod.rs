mod ratelimit;

use ratelimit::Limiter;
use std::collections::HashSet;
use std::error::Error;
use std::io;
use std::io::prelude::*;
use std::net::TcpStream;
use std::sync::mpsc::{channel, SendError, Sender};
use std::sync::Arc;
use std::time::Duration;

pub type ChannelContent = String;
pub type ChannelError = SendError<ChannelContent>;

const INIT_MESSAGES: usize = 2; // How many JOIN/PASS messages we send in the init
const USER_RATE_LIMIT: usize = 20;
const MOD_RATE_LIMIT: usize = 100;

#[derive(Clone)]
pub struct ChatConfig {
    pub oauth_token: String,
    pub bot_username: String,
    pub channel_name: String,
}

impl ChatConfig {
    pub fn new(oauth_token: &str, bot_username: &str, channel_name: &str) -> Self {
        Self {
            oauth_token: oauth_token.to_owned(),
            bot_username: bot_username.to_lowercase(),
            channel_name: channel_name.to_lowercase(),
        }
    }
}

pub struct ChatClient {
    config: ChatConfig,
    socket: TcpStream,
    sender: Sender<ChannelContent>,
    limiter: Arc<Limiter>,
    pub modlist: HashSet<String>,
}

pub struct Bot {
    pub chat: ChatClient,
    pub queue: Vec<String>,
}

impl ChatClient {
    pub fn connect(config: ChatConfig) -> Result<Self, Box<dyn Error>> {
        let socket = TcpStream::connect("irc.chat.twitch.tv:6667")?;
        let mut thread_socket = socket.try_clone()?;
        let (sender, receiver) = channel();
        let limiter = Arc::new(Limiter::new(
            USER_RATE_LIMIT,
            USER_RATE_LIMIT.saturating_sub(INIT_MESSAGES),
            Duration::from_secs(30),
        ));

        let limiter_inner = Arc::clone(&limiter);
        std::thread::spawn(move || {
            for msg in receiver.iter() {
                limiter_inner.wait();
                thread_socket
                    .write_all(format!("{}\r\n", msg).as_bytes())
                    .expect("Sending message failed");
            }
        });

        let mut client = Self {
            socket,
            config,
            sender,
            limiter,
            modlist: HashSet::new(),
        };

        client.send_raw(&format!("PASS {}", client.config.oauth_token))?;
        client.send_raw(&format!("NICK {}", client.config.bot_username))?;
        client.send_raw(&format!("JOIN #{}", client.config.channel_name))?;
        client.send_raw("CAP REQ :twitch.tv/commands")?;
        client.send_msg("/mods")?;

        match client.socket.take_error()? {
            Some(error) => Err(Box::new(error)),
            None => Ok(client),
        }
    }

    pub fn send_raw(&mut self, msg: &str) -> io::Result<()> {
        self.socket.write_all(format!("{}\r\n", msg).as_bytes())
    }

    pub fn send_msg(&self, msg: &str) -> Result<(), SendError<ChannelContent>> {
        println!("< {}", msg);
        self.sender
            .send(format!("PRIVMSG #{} :{}", self.config.channel_name, msg))
    }

    pub fn get_reader(&self) -> io::Result<io::BufReader<TcpStream>> {
        Ok(io::BufReader::new(self.socket.try_clone()?))
    }

    pub fn set_modlist<'a>(&mut self, modlist: impl Iterator<Item = &'a str>) {
        let chan = &self.config.channel_name;
        self.modlist.clear();
        self.modlist.insert(chan.to_owned());
        self.modlist.extend(modlist.map(String::from));
        match self.modlist.contains(&self.config.bot_username) {
            true => self.limiter.set_capacity(MOD_RATE_LIMIT),
            false => self.limiter.set_capacity(USER_RATE_LIMIT),
        }
    }
}

impl Bot {
    pub fn new(config: ChatConfig) -> Result<Self, Box<dyn Error>> {
        Ok(Self {
            chat: ChatClient::connect(config)?,
            queue: Vec::new(),
        })
    }

    pub fn reconnect(&mut self) -> Result<(), Box<dyn Error>> {
        let config = self.chat.config.clone();
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
                    .send_msg(&format!("@{}: It's you turn!", self.queue.remove(0)))?;
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
}
