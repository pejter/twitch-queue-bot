use std::collections::HashSet;
use std::io;
use std::io::prelude::*;
use std::net::{Shutdown, TcpStream};

pub struct Bot {
    pub socket: TcpStream,
    pub channel: String,
    pub queue: Vec<String>,
    pub modlist: HashSet<String>,
}

impl Bot {
    pub fn new(channel_name: &str) -> Self {
        Self {
            socket: TcpStream::connect("irc.chat.twitch.tv:6667")
                .expect("Couldn't connect to twitch servers"),
            queue: Vec::new(),
            channel: channel_name.to_string(),
            modlist: HashSet::new(),
        }
    }

    pub fn connect(&mut self) {
        self.socket = TcpStream::connect("irc.chat.twitch.tv:6667")
            .expect("Couldn't connect to twitch servers");
    }
    pub fn disconnect(&mut self) -> io::Result<()> {
        self.socket.shutdown(Shutdown::Both)
    }

    pub fn send_raw(&mut self, msg: &str) {
        println!("Sending: {}", msg);
        self.socket
            .write_all(format!("{}\r\n", msg).as_bytes())
            .unwrap_or_else(|err| panic!("Sending message failed: {}", err));
    }

    pub fn send_msg(&mut self, msg: &str) {
        self.send_raw(&format!("PRIVMSG #{} :{}", self.channel, msg));
    }

    pub fn get_reader(&self) -> io::BufReader<TcpStream> {
        io::BufReader::new(self.socket.try_clone().unwrap())
    }

    pub fn set_modlist<'a>(&mut self, modlist: impl Iterator<Item = &'a str>) {
        let chan = &self.channel;
        self.modlist.clear();
        self.modlist.insert(chan.to_owned());
        self.modlist.extend(modlist.map(|s| s.to_owned()));
    }
}
