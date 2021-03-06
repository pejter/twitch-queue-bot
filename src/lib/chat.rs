use super::irc::{ChannelContent, ChannelResult, IRCClient, IRCMessage};
use std::collections::HashSet;
use tokio::{
    runtime::Runtime,
    sync::mpsc::{channel, Receiver, Sender},
    time::{sleep, Duration},
};
use tokio_tungstenite::tungstenite::Message;

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
    pub async fn disconnect(sockets: &ClosingResources) -> SendResult {
        sockets.send(Message::Close(None)).await
    }

    pub fn new(rt: &Runtime, config: ChatConfig) -> Self {
        let (chan_sender, receiver) = channel::<IRCMessage>(100);

        let irc = IRCClient::connect(
            rt,
            &config.bot_username,
            &config.channel_name,
            &config.oauth_token,
            chan_sender,
            USER_RATE_LIMIT,
        );

        let modlist_request = format!("PRIVMSG #{} :/mods", config.channel_name);
        let mods_sender = irc.get_sender();
        rt.spawn(async move {
            let d = Duration::from_secs(MODS_INTERVAL);
            while mods_sender
                .send(Message::Text(modlist_request.clone()))
                .await
                .is_ok()
            {
                sleep(d).await;
            }
            println!("/mods exiting");
        });

        Self {
            irc,
            config,
            receiver,
            modlist: HashSet::new(),
        }
    }

    pub fn closing(&self) -> ClosingResources {
        self.irc.get_sender()
    }

    pub fn recv_msg(&mut self) -> Option<ChatMessage> {
        while let Some(line) = self.receiver.blocking_recv() {
            match line {
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
                        self.set_modlist(modlist.map(String::from));
                    }
                }

                _ => {}
            }
        }
        None
    }

    pub fn send_msg(&self, msg: &str) -> ChannelResult {
        println!("< {msg}");
        let channel = &self.config.channel_name;
        self.irc.send(format!("PRIVMSG #{channel} :{msg}\n"))
    }

    pub fn set_modlist(&mut self, modlist: impl Iterator<Item = String>) {
        let chan = self.config.channel_name.clone();
        self.modlist.clear();
        self.modlist.insert(chan);
        self.modlist.extend(modlist);
        let cap = if self.modlist.contains(&self.config.bot_username) {
            MOD_RATE_LIMIT
        } else {
            USER_RATE_LIMIT
        };
        self.irc.limiter.set_capacity(cap);
    }
}
