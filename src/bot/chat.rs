use tokio::{runtime::Handle, sync::mpsc::UnboundedReceiver};
use tracing::{debug, info, warn};
use twitch_irc::{
    login::StaticLoginCredentials, message::ServerMessage, ClientConfig, Error, SecureWSTransport,
    TwitchIRCClient,
};

type Transport = SecureWSTransport;
type Credentials = StaticLoginCredentials;
type IRCError = Error<Transport, Credentials>;
pub type SendResult = Result<(), IRCError>;

#[derive(Debug)]
pub enum Message {
    UserText(String, String),
}

#[derive(Clone)]
pub struct Config {
    pub oauth_token: String,
    pub bot_username: String,
    pub channel_name: String,
}

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
    rt: Handle,
    receiver: UnboundedReceiver<ServerMessage>,
    client: TwitchIRCClient<Transport, Credentials>,
    config: Config,
}

impl Client {
    pub fn new(rt: Handle, config: Config) -> Self {
        info!("Creating twitch chat client");
        let _guard = rt.enter();
        let creds = StaticLoginCredentials::new(
            config.bot_username.to_owned(),
            Some(config.oauth_token.to_owned()),
        );
        let irc_config = ClientConfig::new_simple(creds);
        let (receiver, client) = TwitchIRCClient::<Transport, _>::new(irc_config);

        client
            .join(config.channel_name.to_owned())
            .expect("Couldn't join channel");

        debug!("Creating chat client");
        Self {
            rt,
            client,
            config,
            receiver,
        }
    }

    pub fn recv_msg(&mut self) -> Option<Message> {
        while let Some(line) = self.receiver.blocking_recv() {
            debug!("> {line:?}");
            match line {
                ServerMessage::Privmsg(msg) => {
                    return Some(Message::UserText(msg.sender.login, msg.message_text));
                }
                ServerMessage::Whisper(msg) => {
                    info!("> Whisper ({}): {}", msg.sender.login, msg.message_text)
                }

                _ => {}
            }
        }
        None
    }

    #[tracing::instrument(skip(self))]
    pub fn send_msg(&self, msg: String) -> SendResult {
        info!("< {msg}");
        let channel = self.config.channel_name.to_owned();
        self.rt.block_on(self.client.say(channel, msg))
    }
}
