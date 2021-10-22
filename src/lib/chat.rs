use super::ratelimit::Limiter;
use std::collections::HashSet;
use std::error::Error;
use std::net::TcpStream;
use std::sync::mpsc::{channel, Receiver, RecvError, SendError, Sender};
use std::sync::Arc;
use std::time::Duration;
use websocket::{receiver::Reader, ClientBuilder, OwnedMessage, WebSocketResult};

pub type ChannelContent = OwnedMessage;
pub type ChannelError = SendError<ChannelContent>;
pub type ChannelResult = Result<(), ChannelError>;
type ClosingResources = Sender<ChannelContent>;

const INIT_MESSAGES: u32 = 2; // How many JOIN/PASS messages we send in the init
const USER_RATE_LIMIT: u32 = 20;
const MOD_RATE_LIMIT: u32 = 100;
const MODS_INTERVAL: u64 = 600;
const PING_INTERVAL: u64 = 60;

// If someone with a nickname of length 1 sent us a message it would look like this
// Which means we're safe to skip at least this many characters for message detection
const TWITCH_ENVELOPE_LEN: usize = ":_!_@_.tmi.twitch.tv PRIVMSG #_ ".len();

#[derive(Debug)]
pub enum ChatMessage {
    UserText(String, String),
    ModList(Vec<String>),
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

struct ChatReader {
    echo: Sender<ChannelContent>,
    sender: Sender<ChatMessage>,
    receiver: Reader<TcpStream>,
}

impl ChatReader {
    fn parse_message(&self, msg: &str) -> Option<ChatMessage> {
        match msg.trim_end() {
            "PING :tmi.twitch.tv" => {
                self.echo
                    .send(OwnedMessage::Text("PONG :tmi.twitch.tv".into()))
                    .expect("Unable to respond to PING");
            }

            _ if msg.starts_with(":tmi.twitch.tv 001") => {
                println!("Connected successfully");
            }

            line if line.contains("PRIVMSG") => {
                let user = {
                    let idx = line.find('!').unwrap();
                    &line[1..idx]
                };
                let msg = {
                    let line = &line[TWITCH_ENVELOPE_LEN..];
                    let idx = line.find(':').unwrap();
                    &line[idx + 1..]
                };
                return Some(ChatMessage::UserText(user.to_owned(), msg.to_owned()));
            }

            line if line.contains("NOTICE") => {
                const MODS_PREFIX: &str = "The moderators of this channel are: ";
                if let Some(idx) = line.find(MODS_PREFIX) {
                    let prefix_len = idx + MODS_PREFIX.len();
                    let modlist = line[prefix_len..].split(", ");
                    return Some(ChatMessage::ModList(modlist.map(String::from).collect()));
                }
            }

            _ => {}
        };
        None
    }

    pub fn read(&mut self) {
        loop {
            match self.receiver.recv_message() {
                Ok(msg) => match msg {
                    OwnedMessage::Close(_) => {
                        self.echo.send(msg).ok();
                        println!("Receiver exiting...");
                        return;
                    }
                    OwnedMessage::Ping(data) => {
                        self.echo.send(OwnedMessage::Pong(data)).unwrap();
                    }
                    OwnedMessage::Pong(_) => {}
                    OwnedMessage::Text(msg) => {
                        if let Some(result) = self.parse_message(&msg) {
                            if self.sender.send(result).is_err() {
                                return;
                            };
                        }
                    }
                    OwnedMessage::Binary(data) => {
                        println!("Binary data {:?}", data);
                        if let Ok(msg) = String::from_utf8(data) {
                            println!("Decoded into: {}", msg);
                            if let Some(result) = self.parse_message(&msg) {
                                if self.sender.send(result).is_err() {
                                    return;
                                };
                            }
                        }
                    }
                },
                Err(e) => {
                    println!("Error reading from socket: {}", e);
                    self.echo.send(OwnedMessage::Close(None)).ok();
                    return;
                }
            }
        }
    }

    pub fn new(
        receiver: Reader<TcpStream>,
        sender: Sender<ChatMessage>,
        echo: Sender<ChannelContent>,
    ) -> Self {
        Self {
            echo,
            sender,
            receiver,
        }
    }
}

pub struct ChatClient {
    receiver: Receiver<ChatMessage>,
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

        let modlist_request = format!("PRIVMSG #{} :/mods", config.channel_name);
        let auth = [
            format!("PASS {}", config.oauth_token),
            format!("NICK {}", config.bot_username),
            format!("JOIN #{}", config.channel_name),
            "CAP REQ :twitch.tv/commands".to_string(),
            modlist_request.clone(),
        ];

        for msg in auth {
            ws.send_message(&OwnedMessage::Text(msg))?;
        }

        let (ws_receiver, mut ws_sender) = ws.split()?;
        let (sender, chan_receiver) = channel::<ChannelContent>();
        let (chan_sender, receiver) = channel::<ChatMessage>();
        let ping_sender = sender.clone();
        let mods_sender = sender.clone();
        let mut reader = ChatReader::new(ws_receiver, chan_sender, sender.clone());
        std::thread::spawn(move || reader.read());
        std::thread::spawn(move || loop {
            let d = Duration::from_secs(PING_INTERVAL);
            std::thread::sleep(d);
            ping_sender.send(OwnedMessage::Ping(Vec::new())).unwrap();
        });
        std::thread::spawn(move || loop {
            let d = Duration::from_secs(MODS_INTERVAL);
            std::thread::sleep(d);
            mods_sender
                .send(OwnedMessage::Text(modlist_request.clone()))
                .unwrap();
        });

        let limiter = Arc::new(Limiter::new(
            USER_RATE_LIMIT,
            USER_RATE_LIMIT.saturating_sub(INIT_MESSAGES).into(),
            Duration::from_secs(30),
        ));
        let send_limiter = limiter.clone();
        std::thread::spawn(move || {
            for msg in chan_receiver.iter() {
                match msg {
                    OwnedMessage::Close(_) => {
                        println!("Sender exiting...");
                        ws_sender.send_message(&msg).ok();
                        return;
                    }
                    _ => {
                        send_limiter.wait();
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

    pub fn sockets(&self) -> ClosingResources {
        self.sender.clone()
    }

    pub fn recv_msg(&mut self) -> Result<ChatMessage, RecvError> {
        loop {
            match self.receiver.recv()? {
                ChatMessage::ModList(list) => self.set_modlist(list.into_iter()),
                msg => {
                    return Ok(msg);
                }
            }
        }
    }

    pub fn send_msg(&self, msg: &str) -> ChannelResult {
        println!("< {}", msg);
        self.sender.send(OwnedMessage::Text(format!(
            "PRIVMSG #{} :{}\n",
            self.config.channel_name, msg
        )))
    }

    pub fn set_modlist(&mut self, modlist: impl Iterator<Item = String>) {
        let chan = &self.config.channel_name;
        self.modlist.clear();
        self.modlist.insert(chan.to_owned());
        self.modlist.extend(modlist);
        match self.modlist.contains(&self.config.bot_username) {
            true => self.limiter.set_capacity(MOD_RATE_LIMIT),
            false => self.limiter.set_capacity(USER_RATE_LIMIT),
        }
    }
}
