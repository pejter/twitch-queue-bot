mod bot;
mod config;

use bot::{chat::Message, Bot, Client, Config, SendResult};

use tokio::{runtime::Builder, signal};
use tracing::{error, info, warn};

macro_rules! mod_command {
    ($modlist:tt,$user:tt,$b:block) => {
        match $modlist.contains($user) {
            true => $b,
            false => {
                info!("User {} not authorised to perform mod commands", $user);
                Ok(())
            }
        }
    };
}

#[tracing::instrument(skip(bot))]
fn handle_command(bot: &mut Bot, user: &str, msg: &str) -> SendResult {
    info!("{user}: {msg}");
    let modlist = &bot.chat.modlist;
    match msg.trim_end() {
        "!join" => bot.join(user),
        "!leave" => bot.leave(user),
        "!position" => bot.position(user),
        "!length" => bot.length(),
        // Mod commands
        "!next" => mod_command!(modlist, user, { bot.next() }),
        "!list" => mod_command!(modlist, user, { bot.list() }),
        "!clear" => mod_command!(modlist, user, { bot.clear() }),
        "!open" => mod_command!(modlist, user, { bot.open() }),
        "!close" => mod_command!(modlist, user, { bot.close() }),
        "!reset" => mod_command!(modlist, user, { bot.reset() }),
        "!save" => mod_command!(modlist, user, { bot.save() }),
        command if command.starts_with("!select") => mod_command!(modlist, user, {
            match command.split_once(' ') {
                None => bot.chat.send_msg("You must provide a name for the queue"),
                Some(name) => bot.select(name.1),
            }
        }),
        command if command.starts_with("!create") => mod_command!(modlist, user, {
            match command.split_once(' ') {
                None => bot.chat.send_msg("You must provide a name for the queue"),
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

    info!("Creating bot");
    let rt = Builder::new_current_thread().enable_all().build().unwrap();

    let mut bot = Bot::new(&rt, Config::new(oauth_token, bot_username, channel_name));

    let sockets = bot.chat.closing();
    // Side thread that will run our tokio runtime
    std::thread::spawn(move || {
        rt.block_on(async move {
            match signal::ctrl_c().await {
                Ok(()) => {
                    info!("Received Ctrl-C, exiting...");
                    Client::disconnect(&sockets).await.ok();
                    sockets.closed().await;
                }
                Err(err) => {
                    error!("Unable to listen for shutdown signal: {err}");
                    // we also shut down in case of error
                }
            }
        });
    });

    bot.chat
        .send_msg(&format!(
            "Hello there gamers! {bot_username} is now in chat.",
        ))
        .expect("Unable to send greeting");

    loop {
        match bot.chat.recv_msg() {
            None => {
                break;
            }
            Some(msg) => match msg {
                Message::UserText(user, text) => {
                    if let Err(e) = handle_command(&mut bot, &user, &text) {
                        warn!("Couldn't send message: {e}");
                    };
                }
            },
        }
    }
    info!("Bot exited");
}
