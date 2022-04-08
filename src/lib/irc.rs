use super::ratelimit::Limiter;
use futures_util::{
    stream::{SplitSink, SplitStream},
    SinkExt, StreamExt,
};
use std::sync::Arc;
use tokio::{
    net::TcpStream,
    runtime::Runtime,
    sync::mpsc::{channel, error::SendError, Receiver, Sender},
    task::JoinHandle,
    time::{sleep, Duration},
};
use tokio_tungstenite::{connect_async, tungstenite::Message, MaybeTlsStream, WebSocketStream};

const INIT_MESSAGES: u32 = 2; // How many JOIN/PASS messages we send in the init
const PING_INTERVAL: u64 = 60;
const CLIENT_NOTICE: &str = ":tmi.twitch.tv NOTICE * :";

pub type IRCTasks = Vec<JoinHandle<()>>;
pub type IRCMessage = String;
pub type ChannelContent = Message;
pub type ChannelError = SendError<ChannelContent>;
pub type ChannelResult = Result<(), ChannelError>;

pub struct IRCClient {
    pub sender: Sender<ChannelContent>,
    pub limiter: Arc<Limiter>,
    pub futures: IRCTasks,
}

impl IRCClient {
    pub fn connect(
        rt: &Runtime,
        bot_username: &str,
        channel_name: &str,
        oauth_token: &str,
        chan_sender: Sender<IRCMessage>,
        message_limit: u32,
    ) -> Self {
        let (mut ws, _) = rt
            .block_on(connect_async("wss://irc-ws.chat.twitch.tv:443"))
            .expect("Failed to connect to TMI");

        let auth = [
            format!("PASS {oauth_token}"),
            format!("NICK {bot_username}"),
            format!("JOIN #{channel_name}"),
            "CAP REQ :twitch.tv/commands".to_string(),
        ];

        rt.block_on(async {
            for msg in auth {
                ws.feed(Message::Text(msg)).await.unwrap();
            }
            ws.flush().await.unwrap();
        });
        let (ws_sender, ws_receiver) = ws.split();
        let (sender, chan_receiver) = channel::<ChannelContent>(100);
        let mut futures: IRCTasks = Vec::new();

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
        futures.push(rt.spawn(async move {
            writer.write().await;
            println!("IRC Writer exited");
        }));

        let mut reader = IRCReader {
            receiver: ws_receiver,
            sender: chan_sender,
            echo: sender.clone(),
        };
        futures.push(rt.spawn(async move {
            reader.read().await;
            println!("IRC Reader exited");
        }));

        let ping_sender = sender.clone();
        futures.push(rt.spawn(async move {
            let d = Duration::from_secs(PING_INTERVAL);
            while ping_sender.send(Message::Ping(Vec::new())).await.is_ok() {
                sleep(d).await;
            }
            println!("PING exited");
        }));

        Self {
            sender,
            limiter,
            futures,
        }
    }

    pub fn get_sender(&self) -> Sender<ChannelContent> {
        self.sender.clone()
    }

    pub fn send(&self, msg: String) -> ChannelResult {
        self.sender.blocking_send(Message::Text(msg))
    }
}

pub struct IRCWriter {
    receiver: Receiver<ChannelContent>,
    sender: SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>,
    limiter: Arc<Limiter>,
}

impl IRCWriter {
    pub async fn write(&mut self) {
        while let Some(msg) = self.receiver.recv().await {
            if msg.is_close() {
                println!("Sender exiting...");
                self.sender.send(msg).await.ok();
                self.receiver.close();
                return;
            }

            self.limiter.wait();
            if let Err(err) = self.sender.send(msg).await {
                println!("Send error: {err}");
                self.sender.send(Message::Close(None)).await.unwrap();
            }
        }
    }
}

pub struct IRCReader {
    echo: Sender<ChannelContent>,
    sender: Sender<IRCMessage>,
    receiver: SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>,
}

impl IRCReader {
    async fn extract_msg(&self, msg: String) -> Option<IRCMessage> {
        match msg.trim_end() {
            "PING :tmi.twitch.tv" => {
                self.echo
                    .send(Message::Text("PONG :tmi.twitch.tv".into()))
                    .await
                    .expect("Unable to respond to PING");
            }

            _ if msg.starts_with(":tmi.twitch.tv 001") => {
                println!("Connected successfully");
            }

            line => match msg.strip_prefix(CLIENT_NOTICE) {
                Some(notice) => {
                    print!("Notice: {notice}");
                }
                None => return Some(line.to_owned()),
            },
        }
        None
    }

    pub async fn read(&mut self) {
        while let Some(msg) = self.receiver.next().await {
            match msg {
                Ok(msg) => match msg {
                    Message::Frame(_) => {
                        panic!("Got Message:Frame!");
                    }
                    Message::Close(_) => {
                        self.echo.send(msg).await.ok();
                        println!("Receiver exiting...");
                        return;
                    }
                    Message::Ping(data) => {
                        self.echo.send(Message::Pong(data)).await.unwrap();
                    }
                    Message::Pong(_) => {}
                    Message::Text(msg) => {
                        if let Some(result) = self.extract_msg(msg).await {
                            if self.sender.send(result).await.is_err() {
                                return;
                            };
                        }
                    }
                    Message::Binary(data) => {
                        println!("Binary data {data:?}");
                        if let Ok(msg) = String::from_utf8(data) {
                            println!("Decoded into: {msg}");
                            if let Some(result) = self.extract_msg(msg).await {
                                if self.sender.send(result).await.is_err() {
                                    return;
                                };
                            }
                        }
                    }
                },
                Err(e) => {
                    println!("Error reading from socket: {e}");
                    self.echo.send(Message::Close(None)).await.ok();
                    return;
                }
            }
        }
    }
}
