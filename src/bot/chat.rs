use std::{sync::Arc, time::Duration};

use tokio::{
    sync::{mpsc::UnboundedReceiver, RwLock},
    time::timeout,
};
use tracing::{debug, info};
use twitch_irc::{
    login::StaticLoginCredentials, message::ServerMessage, ClientConfig, Error, SecureWSTransport,
    TwitchIRCClient,
};

type Transport = SecureWSTransport;
type Credentials = StaticLoginCredentials;
type IRCError = Error<Transport, Credentials>;

#[derive(Debug)]
pub enum Message {
    UserText(bool, String, String),
}

pub type Reader = UnboundedReceiver<ServerMessage>;

#[derive(Debug)]
pub enum SendError {
    ClientError(IRCError),
    ClientClosed,
}

pub type SendResult = Result<(), SendError>;

#[derive(Clone)]
pub struct Config {
    pub oauth_token: String,
    pub bot_username: String,
    pub channel_name: String,
}

impl From<IRCError> for SendError {
    fn from(error: IRCError) -> Self {
        Self::ClientError(error)
    }
}

impl std::fmt::Display for SendError {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        match self {
            Self::ClientClosed => write!(fmt, "Client closed"),
            Self::ClientError(e) => e.fmt(fmt),
        }
    }
}

const TIMEOUT: Duration = Duration::from_secs(1);

impl Config {
    pub fn new(oauth_token: &str, bot_username: &str, channel_name: &str) -> Self {
        Self {
            oauth_token: oauth_token.to_owned(),
            bot_username: bot_username.to_lowercase(),
            channel_name: channel_name.to_lowercase(),
        }
    }
}

pub struct Client {
    config: Config,
    reader: Reader,
    client: Option<TwitchIRCClient<Transport, Credentials>>,
    pub closed: Arc<RwLock<bool>>,
}

impl Client {
    pub fn new(config: Config) -> Self {
        info!("Creating twitch chat client");
        let closed = Arc::new(RwLock::new(false));
        let creds = StaticLoginCredentials::new(
            config.bot_username.to_owned(),
            Some(config.oauth_token.to_owned()),
        );
        let irc_config = ClientConfig::new_simple(creds);
        let (reader, client) = TwitchIRCClient::<Transport, _>::new(irc_config);

        client
            .join(config.channel_name.to_owned())
            .expect("Couldn't join channel");

        debug!("Creating chat client");
        Self {
            config,
            reader,
            client: Some(client),
            closed,
        }
    }

    pub async fn recv_msg(&mut self) -> Option<Message> {
        loop {
            if *self.closed.read().await && self.client.is_some() {
                debug!("Chat closed, dropping client");
                self.client = None;
            }
            if let Ok(msg) = timeout(TIMEOUT, self.reader.recv()).await {
                match msg {
                    None => return None,
                    Some(line) => {
                        debug!("> {line:?}");
                        match line {
                            ServerMessage::Privmsg(msg) => {
                                let user = msg.sender.login;
                                let channel = msg.channel_login;
                                let mod_tag = msg.source.tags.0.get("mod");
                                debug!(?mod_tag);
                                let text = msg.message_text;
                                let is_mod =
                                    user == channel || mod_tag == Some(&Some(String::from("1")));
                                return Some(Message::UserText(is_mod, user, text));
                            }
                            ServerMessage::Whisper(msg) => {
                                info!("> Whisper ({}): {}", msg.sender.login, msg.message_text);
                            }

                            _ => {}
                        }
                    }
                }
            }
        }
    }

    #[tracing::instrument(skip(self))]
    pub async fn send_msg(&self, msg: String) -> SendResult {
        info!("< {msg}");
        let channel = self.config.channel_name.to_owned();
        match &self.client {
            None => Err(SendError::ClientClosed),
            Some(client) => Ok(client.say(channel, msg).await?),
        }
    }
}
