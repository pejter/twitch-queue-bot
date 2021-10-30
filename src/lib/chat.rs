use super::irc::{ChannelContent, ChannelResult, IRCClient, IRCMessage};
use std::collections::HashSet;
use std::error::Error;
use std::sync::mpsc::{channel, Receiver, RecvError, Sender};
use std::time::Duration;
use websocket::{OwnedMessage, WebSocketResult};

type ClosingResources = Sender<ChannelContent>;
pub type SendResult = ChannelResult;

const USER_RATE_LIMIT: u32 = 20;
const MOD_RATE_LIMIT: u32 = 100;
const MODS_INTERVAL: u64 = 600;

// If someone with a nickname of length 1 sent us a message it would look like this
// Which means we're safe to skip at least this many characters for message detection
const TWITCH_ENVELOPE_LEN: usize = ":_!_@_.tmi.twitch.tv PRIVMSG #_ ".len();

#[derive(Debug)]
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
    receiver: Receiver<IRCMessage>,
    irc: IRCClient,
    config: ChatConfig,
    pub modlist: HashSet<String>,
}

impl ChatClient {
    pub fn disconnect(sockets: &ClosingResources) -> Result<(), Box<dyn Error>> {
        Ok(sockets.send(OwnedMessage::Close(None))?)
    }

    pub fn new(config: ChatConfig) -> WebSocketResult<Self> {
        let (chan_sender, receiver) = channel::<IRCMessage>();

        let irc = IRCClient::connect(
            &config.bot_username,
            &config.channel_name,
            &config.oauth_token,
            chan_sender,
            USER_RATE_LIMIT,
        )?;

        let modlist_request = format!("PRIVMSG #{} :/mods", config.channel_name);
        let mods_sender = irc.get_sender();
        std::thread::spawn(move || loop {
            let d = Duration::from_secs(MODS_INTERVAL);
            mods_sender
                .send(OwnedMessage::Text(modlist_request.clone()))
                .unwrap();
            std::thread::sleep(d);
        });

        Ok(Self {
            irc,
            config,
            receiver,
            modlist: HashSet::new(),
        })
    }

    pub fn sockets(&self) -> ClosingResources {
        self.irc.get_sender()
    }

    pub fn recv_msg(&mut self) -> Result<ChatMessage, RecvError> {
        loop {
            match self.receiver.recv()? {
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
                    return Ok(ChatMessage::UserText(user.to_owned(), msg.to_owned()));
                }

                line if line.contains("NOTICE") => {
                    const MODS_PREFIX: &str = "The moderators of this channel are: ";
                    if let Some(idx) = line.find(MODS_PREFIX) {
                        let prefix_len = idx + MODS_PREFIX.len();
                        let modlist = line[prefix_len..].split(", ");
                        self.set_modlist(modlist.map(String::from));
                    }
                }

                _ => {}
            }
        }
    }

    pub fn send_msg(&self, msg: &str) -> ChannelResult {
        println!("< {}", msg);
        self.irc
            .send(format!("PRIVMSG #{} :{}\n", self.config.channel_name, msg))
    }

    pub fn set_modlist(&mut self, modlist: impl Iterator<Item = String>) {
        let chan = &self.config.channel_name;
        self.modlist.clear();
        self.modlist.insert(chan.to_owned());
        self.modlist.extend(modlist);
        match self.modlist.contains(&self.config.bot_username) {
            true => self.irc.limiter.set_capacity(MOD_RATE_LIMIT),
            false => self.irc.limiter.set_capacity(USER_RATE_LIMIT),
        }
    }
}
