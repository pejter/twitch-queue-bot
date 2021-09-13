mod config;
mod lib;
mod termcolor;

use lib::{Bot, ChannelError, ChatConfig};
use std::io::BufRead;
use std::{thread, time};

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

fn handle_command(bot: &mut Bot, user: &str, msg: &str) -> Result<(), ChannelError> {
    let modlist = &bot.chat.modlist;
    match msg.trim_end() {
        "!join" => bot.push(user),
        "!leave" => bot.remove(user),
        "!position" => bot.find(user),
        "!length" => bot.length(),
        // Mod commands
        "!next" => mod_command!(modlist, user, { bot.shift() }),
        "!list" => mod_command!(modlist, user, { bot.list() }),
        "!clear" => mod_command!(modlist, user, { bot.clear() }),
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
                let idx = line[1..].find(':').unwrap();
                &line[idx + 2..]
            };
            handle_command(bot, user, msg).expect("Couldn't send message");
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
            .send_msg(
                "The queue is now open! Available commands: !join, !leave, !position, !length",
            )
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
