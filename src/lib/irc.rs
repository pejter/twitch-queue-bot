use super::ratelimit::Limiter;
use std::sync::mpsc::{channel, Receiver, SendError, Sender};
use std::sync::Arc;
use std::time::Duration;
use websocket::{
    receiver::Reader, sender::Writer, sync::stream::TcpStream, ClientBuilder, OwnedMessage,
    WebSocketResult,
};

const INIT_MESSAGES: u32 = 2; // How many JOIN/PASS messages we send in the init
const PING_INTERVAL: u64 = 60;
const CLIENT_NOTICE: &str = ":tmi.twitch.tv NOTICE * :";

pub type IRCMessage = String;
pub type ChannelContent = OwnedMessage;
pub type ChannelError = SendError<ChannelContent>;
pub type ChannelResult = Result<(), ChannelError>;

pub struct IRCClient {
    pub sender: Sender<ChannelContent>,
    pub limiter: Arc<Limiter>,
}

impl IRCClient {
    pub fn connect(
        bot_username: &str,
        channel_name: &str,
        oauth_token: &str,
        chan_sender: Sender<IRCMessage>,
        message_limit: u32,
    ) -> WebSocketResult<Self> {
        // Use insecure adress until TlsStream implements Splittable
        // https://github.com/websockets-rs/rust-websocket/issues/150
        let mut ws = ClientBuilder::new("ws://irc-ws.chat.twitch.tv:80")
            .unwrap()
            .connect_insecure()?;

        let auth = [
            format!("PASS {}", oauth_token),
            format!("NICK {}", bot_username),
            format!("JOIN #{}", channel_name),
            "CAP REQ :twitch.tv/commands".to_string(),
        ];

        for msg in auth {
            ws.send_message(&OwnedMessage::Text(msg))?;
        }

        let (ws_receiver, ws_sender) = ws.split()?;
        let (sender, chan_receiver) = channel::<ChannelContent>();

        let limiter = Arc::new(Limiter::new(
            message_limit,
            message_limit.saturating_sub(INIT_MESSAGES).into(),
            Duration::from_secs(30),
        ));
        let mut writer = IRCWriter {
            receiver: chan_receiver,
            sender: ws_sender,
            limiter: limiter.clone(),
        };
        std::thread::spawn(move || writer.write());

        let mut reader = IRCReader {
            receiver: ws_receiver,
            sender: chan_sender,
            echo: sender.clone(),
        };
        std::thread::spawn(move || reader.read());

        let ping_sender = sender.clone();
        std::thread::spawn(move || loop {
            let d = Duration::from_secs(PING_INTERVAL);
            std::thread::sleep(d);
            ping_sender.send(OwnedMessage::Ping(Vec::new())).unwrap();
        });

        Ok(Self { sender, limiter })
    }

    pub fn get_sender(&self) -> Sender<ChannelContent> {
        self.sender.clone()
    }

    pub fn send(&self, msg: String) -> ChannelResult {
        self.sender.send(OwnedMessage::Text(msg))
    }
}

pub struct IRCWriter {
    receiver: Receiver<ChannelContent>,
    sender: Writer<TcpStream>,
    limiter: Arc<Limiter>,
}

impl IRCWriter {
    pub fn write(&mut self) {
        for msg in self.receiver.iter() {
            match msg {
                OwnedMessage::Close(_) => {
                    println!("Sender exiting...");
                    self.sender.send_message(&msg).ok();
                    return;
                }
                _ => {
                    self.limiter.wait();
                    self.sender.send_message(&msg).unwrap_or_else(|err| {
                        println!("Send error: {}", err);
                        self.sender
                            .send_message(&OwnedMessage::Close(None))
                            .unwrap();
                    });
                }
            }
        }
    }
}

pub struct IRCReader {
    echo: Sender<ChannelContent>,
    sender: Sender<IRCMessage>,
    receiver: Reader<TcpStream>,
}

impl IRCReader {
    fn extract_msg(&self, msg: String) -> Option<IRCMessage> {
        match msg.trim_end() {
            "PING :tmi.twitch.tv" => {
                self.echo
                    .send(OwnedMessage::Text("PONG :tmi.twitch.tv".into()))
                    .expect("Unable to respond to PING");
            }

            _ if msg.starts_with(":tmi.twitch.tv 001") => {
                println!("Connected successfully");
            }

            line => match msg.strip_prefix(CLIENT_NOTICE) {
                Some(notice) => {
                    print!("Notice: {}", notice);
                }
                None => return Some(line.to_owned()),
            },
        }
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
                        if let Some(result) = self.extract_msg(msg) {
                            if self.sender.send(result).is_err() {
                                return;
                            };
                        }
                    }
                    OwnedMessage::Binary(data) => {
                        println!("Binary data {:?}", data);
                        if let Ok(msg) = String::from_utf8(data) {
                            println!("Decoded into: {}", msg);
                            if let Some(result) = self.extract_msg(msg) {
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
}
