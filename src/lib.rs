use std::collections::HashSet;
use std::io;
use std::io::prelude::*;
use std::net::{Shutdown, TcpStream};

#[derive(Clone)]
pub struct ChatConfig {
    pub oauth_token: String,
    pub bot_username: String,
    pub channel_name: String,
}

pub struct ChatClient {
    config: ChatConfig,
    pub socket: TcpStream,
    pub modlist: HashSet<String>,
}

pub struct Bot {
    pub chat: ChatClient,
    pub queue: Vec<String>,
}

impl ChatClient {
    pub fn connect(config: ChatConfig) -> io::Result<Self> {
        let socket = TcpStream::connect("irc.chat.twitch.tv:6667")?;

        let mut client = Self {
            socket,
            config,
            modlist: HashSet::new(),
        };

        client.send_raw(&format!("PASS {}", client.config.oauth_token));
        client.send_raw(&format!("NICK {}", client.config.bot_username));
        client.send_raw(&format!("JOIN #{}", client.config.channel_name));
        client.send_raw("CAP REQ :twitch.tv/commands");
        client.send_msg("/mods");

        match client.socket.take_error()? {
            Some(error) => {
                println!("{}", error);
            }
            None => {
                println!("No error");
            }
        }

        Ok(client)
    }

    pub fn disconnect(&mut self) -> io::Result<()> {
        self.socket.shutdown(Shutdown::Both)
    }

    pub fn send_raw(&mut self, msg: &str) {
        println!("Sending: {}", msg);
        self.socket
            .write_all(format!("{}\r\n", msg).as_bytes())
            .expect("Sending message failed");
    }

    pub fn send_msg(&mut self, msg: &str) {
        self.send_raw(&format!("PRIVMSG #{} :{}", self.config.channel_name, msg));
    }

    pub fn get_reader(&self) -> io::Result<io::BufReader<TcpStream>> {
        Ok(io::BufReader::new(self.socket.try_clone()?))
    }

    pub fn set_modlist<'a>(&mut self, modlist: impl Iterator<Item = &'a str>) {
        let chan = &self.config.channel_name;
        self.modlist.clear();
        self.modlist.insert(chan.to_owned());
        self.modlist.extend(modlist.map(|s| s.to_owned()));
    }
}
impl Bot {
    pub fn new(config: ChatConfig) -> io::Result<Self> {
        Ok(Self {
            chat: ChatClient::connect(config)?,
            queue: Vec::new(),
        })
    }
    pub fn reconnect(&mut self) -> io::Result<()> {
        let config = self.chat.config.clone();
        self.chat = ChatClient::connect(config)?;
        Ok(())
    }
}
