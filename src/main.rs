mod config;
mod lib;
mod termcolor;

use lib::{chat::ChatMessage, Bot, ChannelResult, ChatClient, ChatConfig};

use termcolor::Color;

macro_rules! mod_command {
    ($modlist:tt,$user:tt,$b:block) => {
        match $modlist.contains($user) {
            true => $b,
            false => {
                println!("User {} not authorised to perform mod commands", $user);
                Ok(())
            }
        }
    };
}

fn handle_command(bot: &mut Bot, user: &str, msg: &str) -> ChannelResult {
    println!("{}: {}", user, msg);
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
            match command.split_once(" ") {
                None => bot.chat.send_msg("You must provide a name for the queue"),
                Some(name) => bot.select(name.1),
            }
        }),
        command if command.starts_with("!create") => mod_command!(modlist, user, {
            match command.split_once(" ") {
                None => bot.chat.send_msg("You must provide a name for the queue"),
                Some(name) => bot.create(name.1),
            }
        }),
        // Not a command
        _ => Ok(()),
    }
}

fn main() {
    colorprintln!(Color::Green, "Reading config");
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

    colorprintln!(Color::Green, "Creating bot");
    let mut bot = Bot::new(ChatConfig::new(oauth_token, bot_username, channel_name)).unwrap();

    let sockets = bot.chat.sockets();
    ctrlc::set_handler(move || {
        println!("Received Ctrl-C, exiting...");
        ChatClient::disconnect(&sockets).ok();
    })
    .expect("Error setting Ctrl-C handler");

    bot.chat
        .send_msg(&format!(
            "Hello there gamers! {} is now in chat.",
            bot_username
        ))
        .expect("Unable to send greeting");

    loop {
        match bot.chat.recv_msg() {
            Err(_) => {
                break;
            }
            Ok(msg) => match msg {
                ChatMessage::UserText(user, text) => {
                    if let Err(e) = handle_command(&mut bot, &user, &text) {
                        println!("Couldn't send message: {}", e);
                    };
                }
                other => println!("{:?}", other),
            },
        }
    }
    colorprintln!(Color::Green, "Bot exited");
}
