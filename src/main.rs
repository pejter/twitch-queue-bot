#![feature(result_into_ok_or_err)]

mod config;
mod lib;
mod termcolor;

use lib::{Bot, ChannelResult, ChatConfig};
use std::io::BufRead;
use std::{thread, time};

use termcolor::Color;

// If someone with a nickname of length 1 sent us a message it would look like this
// Which means we're safe to skip at least this many characters for message detection
const TWITCH_ENVELOPE_LEN: usize = ":_!_@_.tmi.twitch.tv PRIVMSG #_ ".len();

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
        "!save" => mod_command!(modlist, user, { bot.save() }),
        "!unload" => mod_command!(modlist, user, { bot.unload() }),
        command if command.starts_with("!load") => mod_command!(modlist, user, {
            match command.split_once(" ") {
                None => bot.chat.send_msg("You must provide a name for the queue"),
                Some(name) => bot.load(name.1),
            }
        }),
        command if command.starts_with("!create") => mod_command!(modlist, user, {
            match command.split_once(" ") {
                None => bot.chat.send_msg("You must provide a name for the queue"),
                Some(name) => bot.create(name.1),
            }
        }),
        // Not a command
        default => {
            println!("{}: {}", user, default);
            Ok(())
        }
    }
}

fn message_handler(bot: &mut Bot, msg: String) {
    match msg.as_str() {
        "PING :tmi.twitch.tv" => bot
            .chat
            .send_raw("PONG :tmi.twitch.tv")
            .expect("Unable to respond to PING"),
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
            if let Err(e) = handle_command(bot, user, msg) {
                println!("Couldn't send message: {}", e);
            };
        }
        line if line.contains("NOTICE") => {
            let prefix = "The moderators of this channel are: ";
            if let Some(idx) = line.find(prefix) {
                let modlist = line[idx + prefix.len()..].split(", ");
                bot.chat.set_modlist(modlist);
                println!("Moderators: {:#?}", bot.chat.modlist)
            }
        }
        line if line.contains("USERSTATE") => {}

        _ => {
            println!("> {}", msg)
        }
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

    loop {
        bot.chat
            .send_msg(&format!(
                "Hello there gamers! {} is now in chat.",
                bot_username
            ))
            .expect("Unable to send greeting");
        let reader = bot.chat.get_reader().expect("Getting chat reader failed");
        for result in reader.lines() {
            match result {
                Ok(line) => message_handler(&mut bot, line),
                Err(error) => {
                    colorprintln!(Color::Red, "Error while reading from socket: {}", error);
                }
            }
        }
        let duration = time::Duration::from_secs(25);
        colorprintln!(
            Color::Red,
            "Unexpected EOF, reconnecting after {:?}...",
            duration
        );
        thread::sleep(duration);
        colorprintln!(Color::Green, "Reconnecting");
        bot.reconnect().unwrap();
    }
}
