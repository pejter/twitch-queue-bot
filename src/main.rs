#![feature(print_internals)]

mod config;
mod lib;
mod termcolor;

use lib::{Bot, ChatConfig};
use std::io::BufRead;
use std::sync::mpsc::SendError;
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

fn clear(bot: &mut Bot) -> Result<(), SendError<String>> {
    bot.queue.clear();
    bot.chat.send_msg("Queue has been cleared")
}

fn push(bot: &mut Bot, user: &str) -> Result<(), SendError<String>> {
    match bot.queue.iter().position(|x| x == user) {
        Some(idx) => bot.chat.send_msg(&format!(
            "@{}: You're already in queue at position {}",
            user,
            idx + 1
        )),
        None => {
            bot.queue.push(user.to_owned());
            bot.chat.send_msg(&format!(
                "@{}: You've been added to queue at position {}",
                user,
                bot.queue.len()
            ))
        }
    }
}

fn remove(bot: &mut Bot, user: &str) -> Result<(), SendError<String>> {
    match bot.queue.iter().position(|x| x == user) {
        Some(idx) => {
            bot.queue.remove(idx);
            Ok(bot
                .chat
                .send_msg(&format!("@{}: You've been removed from the queue", user))?)
        }
        None => bot
            .chat
            .send_msg(&format!("@{}: You were not queued", user)),
    }
}

fn find(bot: &mut Bot, user: &str) -> Result<(), SendError<String>> {
    match bot.queue.iter().position(|x| x == user) {
        Some(idx) => bot
            .chat
            .send_msg(&format!("@{} you are number {} in queue", user, idx + 1)),
        None => bot
            .chat
            .send_msg(&format!("@{}: You're not currently queued", user)),
    }
}

fn shift(bot: &mut Bot) -> Result<(), SendError<String>> {
    let queue = &bot.queue;
    match queue.first() {
        Some(user) => bot
            .chat
            .send_msg(&format!("Next person in queue: @{}", user)),
        None => bot.chat.send_msg("The queue is currently empty"),
    }
}

fn length(bot: &mut Bot) -> Result<(), SendError<String>> {
    bot.chat
        .send_msg(&format!("There are {} people in queue", bot.queue.len()))
}

fn handle_command(bot: &mut Bot, user: &str, msg: &str) -> Result<(), SendError<String>> {
    let modlist = &bot.chat.modlist;
    match msg.trim_end() {
        "!join" => push(bot, user),
        "!leave" => remove(bot, user),
        "!position" => find(bot, user),
        "!length" => length(bot),
        // Mod commands
        "!next" => mod_command!(modlist, user, { shift(bot) }),
        "!clear" => mod_command!(modlist, user, { clear(bot) }),
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
                println!("Mods: {:#?}", bot.chat.modlist)
            }
        }
        line if line.contains("USERSTATE") => {}

        _ => {
            println!("Received: {}", msg)
        }
    }
}
fn main() {
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

    let mut bot = Bot::new(ChatConfig {
        oauth_token: oauth_token.to_owned(),
        bot_username: bot_username.to_owned(),
        channel_name: channel_name.to_owned(),
    })
    .unwrap();

    loop {
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
        colorprintln!(Color::Blue, "Reconnecting");
        bot.reconnect().unwrap();
    }
}
