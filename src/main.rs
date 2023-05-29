mod bot;
mod config;

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use bot::{chat::Message, Bot, Config, SendResult};

use tokio::{runtime::Builder, signal};
use tracing::{debug, error, info, warn};

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

#[tracing::instrument(skip(bot))]
fn handle_command(bot: &mut Bot, channel: &str, user: &str, msg: &str) -> SendResult {
    info!("{user}: {msg}");
    let is_mod = user.to_lowercase() == channel.to_lowercase();
    match msg.trim_end() {
        "!join" => bot.join(user),
        "!leave" => bot.leave(user),
        "!position" => bot.position(user),
        "!length" => bot.length(),
        // Mod commands
        "!next" => mod_command!(is_mod, user, { bot.next() }),
        "!list" => mod_command!(is_mod, user, { bot.list() }),
        "!clear" => mod_command!(is_mod, user, { bot.clear() }),
        "!open" => mod_command!(is_mod, user, { bot.open() }),
        "!close" => mod_command!(is_mod, user, { bot.close() }),
        "!reset" => mod_command!(is_mod, user, { bot.reset() }),
        "!save" => mod_command!(is_mod, user, { bot.save() }),
        command if command.starts_with("!select") => mod_command!(is_mod, user, {
            match command.split_once(' ') {
                None => bot
                    .chat
                    .send_msg("You must provide a name for the queue".into()),
                Some(name) => bot.select(name.1),
            }
        }),
        command if command.starts_with("!create") => mod_command!(is_mod, user, {
            match command.split_once(' ') {
                None => bot
                    .chat
                    .send_msg("You must provide a name for the queue".into()),
                Some(name) => bot.create(name.1),
            }
        }),
        // Not a command
        _ => Ok(()),
    }
}

fn main() {
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

    let rt = Builder::new_current_thread().enable_all().build().unwrap();
    let rt_handle = rt.handle().clone();

    let closed = Arc::new(AtomicBool::new(false));
    // Side thread that will run our tokio runtime
    std::thread::spawn({
        let closed = closed.clone();
        move || {
            rt.block_on(async move {
                match signal::ctrl_c().await {
                    Ok(()) => {
                        info!("Received Ctrl-C, exiting...");
                        closed.store(true, Ordering::SeqCst);
                    }
                    Err(err) => {
                        error!("Unable to listen for shutdown signal: {err}");
                        // we also shut down in case of error
                    }
                }
            });
        }
    });

    info!("Creating bot");
    let mut bot = Bot::new(
        rt_handle,
        Config::new(oauth_token, bot_username, channel_name),
    );

    bot.chat
        .send_msg(format!(
            "Hello there gamers! {bot_username} is now in chat.",
        ))
        .expect("Unable to send greeting");

    while !closed.load(Ordering::Relaxed) {
        match bot.chat.recv_msg() {
            None => {
                break;
            }
            Some(msg) => match msg {
                Message::UserText(user, text) => {
                    if let Err(e) = handle_command(&mut bot, channel_name, &user, &text) {
                        warn!("Couldn't send message: {e}");
                    };
                }
            },
        }
    }
    info!("Bot exited");
}
