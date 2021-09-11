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
        let mut socket_recv = socket.try_clone()?;
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
                socket_recv
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

    pub fn send_msg(&mut self, msg: &str) -> Result<(), SendError<ChannelContent>> {
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
        self.modlist.extend(modlist.map(|s| s.to_owned()));
        match self
            .modlist
            .contains(&self.config.bot_username.to_lowercase())
        {
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
}
