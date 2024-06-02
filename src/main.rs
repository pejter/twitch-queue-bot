mod bot;
mod config;

use bot::{Bot, Config, Message, SendResult};

use tokio::signal;
use tracing::{debug, info, warn};

macro_rules! mod_command {
    ($is_mod:tt,$user:tt,$b:block) => {
        match $is_mod {
            true => {
                debug!("User {} is a moderator", $user);
                $b
            }
            false => {
                info!("User {} not authorised to perform mod commands", $user);
                Ok(())
            }
        }
    };
}

const CMD_PREFIX: char = '!';

#[tracing::instrument(skip(bot))]
async fn handle_command(bot: &mut Bot, is_mod: bool, user: &str, msg: &str) -> SendResult {
    info!("{user}: {msg}");
    let msg = match msg.strip_prefix(CMD_PREFIX) {
        None => return Ok(()),
        Some(msg) => msg.trim_end(),
    };
    let (cmd, args) = match msg.split_once(' ') {
        None => (msg, None),
        Some((cmd, args)) => (cmd, Some(args)),
    };
    match (cmd.to_lowercase().as_str(), args) {
        ("join", name) => bot.join(user, name).await,
        ("name", name) => bot.name(user, name).await,
        ("leave", _) => bot.leave(user).await,
        ("position", _) => bot.position(user).await,
        ("length", _) => bot.length().await,
        // Mod commands
        ("next", _) => mod_command!(is_mod, user, { bot.next().await }),
        ("list", _) => mod_command!(is_mod, user, { bot.list().await }),
        ("clear", _) => mod_command!(is_mod, user, { bot.clear().await }),
        ("open", _) => mod_command!(is_mod, user, { bot.open().await }),
        ("close", _) => mod_command!(is_mod, user, { bot.close().await }),
        ("reset", _) => mod_command!(is_mod, user, { bot.reset().await }),
        ("save", _) => mod_command!(is_mod, user, { bot.save().await }),
        ("select", name) => mod_command!(is_mod, user, {
            match name {
                Some(name) => bot.select(name).await,
                None => {
                    bot.send_msg("You must provide a name for the queue".into())
                        .await
                }
            }
        }),
        ("create", name) => mod_command!(is_mod, user, {
            match name {
                Some(name) => bot.create(name).await,
                None => {
                    bot.send_msg("You must provide a name for the queue".into())
                        .await
                }
            }
        }),
        // Not a command
        _ => Ok(()),
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .compact()
        .with_target(false)
        .with_file(true)
        .with_line_number(true)
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    info!("Reading config");
    let config = config::read().unwrap();
    let oauth_token = config
        .get("OAUTH_TOKEN")
        .expect("OAUTH_TOKEN must be present in the config");
    let bot_username = config
        .get("BOT_USERNAME")
        .expect("BOT_USERNAME must be present in the config");
    let channel_name = config
        .get("CHANNEL_NAME")
        .expect("CHANNEL_NAME must be present in the config");

    info!("Creating bot");
    let mut bot = Bot::new(Config::new(oauth_token, bot_username, channel_name));

    let closed = bot.chat.closed.clone();
    tokio::spawn(async move {
        match signal::ctrl_c().await {
            Ok(()) => {
                info!("Received Ctrl-C, exiting...");
                *closed.write().await = true;
                info!("Chat closed");
            }
            Err(err) => {
                panic!("Unable to listen for shutdown signal: {err}");
            }
        }
    });

    bot.send_msg(format!(
        "Hello there gamers! {bot_username} is now in chat.",
    ))
    .await
    .expect("Unable to send greeting");

    loop {
        match bot.recv_msg().await {
            None => {
                break;
            }
            Some(msg) => match msg {
                Message::UserText(is_mod, user, text) => {
                    if let Err(e) = handle_command(&mut bot, is_mod, &user, &text).await {
                        warn!("Couldn't send message: {e}");
                    };
                }
            },
        }
    }
    info!("Bot exited");
}
