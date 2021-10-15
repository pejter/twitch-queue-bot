use super::ratelimit::Limiter;
use std::collections::HashSet;
use std::error::Error;
use std::net::TcpStream;
use std::sync::mpsc::{channel, SendError, Sender};
use std::sync::Arc;
use std::time::Duration;
use websocket::receiver::Reader;
use websocket::{ClientBuilder, OwnedMessage, WebSocketResult};

pub type ChannelContent = OwnedMessage;
pub type ChannelError = SendError<ChannelContent>;
pub type ChannelResult = Result<(), ChannelError>;
type ClosingResources = Sender<ChannelContent>;

const INIT_MESSAGES: usize = 2; // How many JOIN/PASS messages we send in the init
const USER_RATE_LIMIT: usize = 20;
const MOD_RATE_LIMIT: usize = 100;
const PING_INTERVAL: u64 = 60;

// If someone with a nickname of length 1 sent us a message it would look like this
// Which means we're safe to skip at least this many characters for message detection
const TWITCH_ENVELOPE_LEN: usize = ":_!_@_.tmi.twitch.tv PRIVMSG #_ ".len();

// The length of the mods message without the channel name
const MODS_ENVELOPE_LEN: usize =
    ":tmi.twitch.tv NOTICE # :The moderators of this channel are: ".len();

pub enum ChatMessage {
    UserText(String, String),
}

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
    receiver: Reader<TcpStream>,
    sender: Sender<ChannelContent>,
    limiter: Arc<Limiter>,
    config: ChatConfig,
    pub modlist: HashSet<String>,
}

impl ChatClient {
    pub fn disconnect(sockets: &ClosingResources) -> Result<(), Box<dyn Error>> {
        Ok(sockets.send(OwnedMessage::Close(None))?)
    }

    pub fn connect(config: ChatConfig) -> WebSocketResult<Self> {
        let mut ws = ClientBuilder::new("ws://irc-ws.chat.twitch.tv:80")
            .unwrap()
            .connect_insecure()?;

        let auth = [
            format!("PASS {}", config.oauth_token),
            format!("NICK {}", config.bot_username),
            format!("JOIN #{}", config.channel_name),
            "CAP REQ :twitch.tv/commands".to_string(),
            format!("PRIVMSG #{} :/mods", config.channel_name),
        ];

        for msg in auth {
            ws.send_message(&OwnedMessage::Text(msg))?;
        }

        let limiter = Arc::new(Limiter::new(
            USER_RATE_LIMIT,
            USER_RATE_LIMIT.saturating_sub(INIT_MESSAGES),
            Duration::from_secs(30),
        ));

        let (receiver, mut ws_sender) = ws.split()?;
        let (sender, chan_receiver) = channel::<ChannelContent>();
        let ping_sender = sender.clone();
        let limiter_send = Arc::clone(&limiter);
        std::thread::spawn(move || loop {
            let d = Duration::from_secs(PING_INTERVAL);
            std::thread::sleep(d);
            ping_sender.send(OwnedMessage::Ping(Vec::new())).unwrap();
        });
        std::thread::spawn(move || {
            for msg in chan_receiver.iter() {
                match msg {
                    OwnedMessage::Close(_) => {
                        println!("Sender thread exiting...");
                        ws_sender.send_message(&msg).ok();
                        return;
                    }
                    _ => {
                        limiter_send.wait();
                        ws_sender.send_message(&msg).unwrap_or_else(|err| {
                            println!("Send error: {}", err);
                            ws_sender.send_message(&OwnedMessage::Close(None)).unwrap();
                        });
                    }
                }
            }
        });

        Ok(Self {
            receiver,
            config,
            sender,
            limiter,
            modlist: HashSet::new(),
        })
    }

    pub fn channel_name(&self) -> &str {
        self.config.channel_name.as_str()
    }

    pub fn sockets(&self) -> ClosingResources {
        self.sender.clone()
    }

    fn parse_privmsg(line: &str) -> ChatMessage {
        let user = {
            let idx = line.find('!').unwrap();
            &line[1..idx]
        };
        let msg = {
            let line = &line[TWITCH_ENVELOPE_LEN..];
            let idx = line.find(':').unwrap();
            &line[idx + 1..]
        };
        ChatMessage::UserText(user.to_owned(), msg.to_owned())
    }

    fn parse_message(&mut self, msg: &str) -> Option<ChatMessage> {
        match msg.trim_end() {
            "PING :tmi.twitch.tv" => {
                println!("PONG!");
                self.send_raw("PONG :tmi.twitch.tv")
                    .expect("Unable to respond to PING");
            }

            _ if msg.starts_with(":tmi.twitch.tv 001") => {
                println!("Connected successfully");
            }

            line if msg.contains("The moderators of this channel are: ") => {
                let prefix_len = MODS_ENVELOPE_LEN + self.channel_name().len();
                let modlist = line[prefix_len..].split(", ");
                self.set_modlist(modlist);
                println!("Moderators: {:#?}", self.modlist)
            }

            line if line.contains("PRIVMSG") => {
                return Some(Self::parse_privmsg(line));
            }
            _ => {}
        };
        None
    }

    pub fn recv_msg(&mut self) -> Result<Option<ChatMessage>, Box<dyn Error>> {
        loop {
            match self.receiver.recv_message() {
                Ok(msg) => match msg {
                    OwnedMessage::Close(_) => {
                        self.sender.send(msg).ok();
                        return Ok(None);
                    }
                    OwnedMessage::Ping(data) => self.sender.send(OwnedMessage::Pong(data))?,
                    OwnedMessage::Pong(_) => {}
                    OwnedMessage::Text(msg) => {
                        let result = self.parse_message(&msg);
                        if result.is_some() {
                            return Ok(result);
                        }
                    }
                    OwnedMessage::Binary(data) => {
                        println!("Binary data {:?}", data);
                        if let Ok(msg) = String::from_utf8(data) {
                            println!("Decoded into: {}", msg);
                            if let Some(result) = self.parse_message(&msg) {
                                return Ok(Some(result));
                            }
                        }
                    }
                },
                Err(e) => {
                    println!("Error reading from socket: {}", e);
                    return Err(Box::new(e));
                }
            }
        }
    }

    pub fn send_raw(&self, msg: &str) -> ChannelResult {
        self.sender.send(OwnedMessage::Text(format!("{}\n", msg)))
    }

    pub fn send_msg(&self, msg: &str) -> ChannelResult {
        println!("< {}", msg);
        self.sender.send(OwnedMessage::Text(format!(
            "PRIVMSG #{} :{}\n",
            self.config.channel_name, msg
        )))
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
