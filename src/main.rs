#![feature(print_internals)]

mod config;
mod lib;
mod termcolor;

use lib::Bot;
use std::io::{self, BufRead};
use std::{thread, time};

#[macro_use]
use termcolor::{Color};

macro_rules! mod_command {
    ($modlist:tt,$user:tt,$b:block) => {
        match $modlist.contains($user) {
            true => $b,
            false => {
                println!("User {} not authorised to perform mod commands", $user);
            }
        }
    };
}

fn clear(bot: &mut Bot) {
    bot.queue.clear();
    println!("Queue cleared");
}

fn push(bot: &mut Bot, user: &str) {
    match bot.queue.iter().position(|x| x == user) {
        Some(idx) => {
            bot.send_msg(&format!(
                "@{}: You're already in queue at position {}",
                user,
                idx + 1
            ));
        }
        None => {
            bot.queue.push(user.to_owned());
            bot.send_msg(&format!(
                "@{}: You've been added to queue at position {}",
                user,
                bot.queue.len()
            ));
        }
    }
}

fn remove(bot: &mut Bot, user: &str) {
    match bot.queue.iter().position(|x| x == user) {
        Some(idx) => {
            bot.queue.remove(idx);
            bot.send_msg(&format!("@{}: You've been removed from the queue", user));
        }
        None => {
            bot.send_msg(&format!("@{}: You were not queued", user));
        }
    }
}

fn find(bot: &mut Bot, user: &str) {
    match bot.queue.iter().position(|x| x == user) {
        Some(idx) => {
            bot.send_msg(&format!("@{} you are number {} in queue", user, idx + 1));
        }
        None => {
            bot.send_msg(&format!("@{}: You're not currently queued", user));
        }
    }
}

fn shift(bot: &mut Bot) {
    let queue = &bot.queue;
    match queue.first() {
        Some(user) => {
            bot.send_msg(&format!("Next person in queue: @{}", user));
        }
        None => {
            bot.send_msg("The queue is currently empty");
        }
    }
}

fn length(bot: &mut Bot) {
    {
        bot.send_msg(&format!("There are {} people in queue", bot.queue.len()));
    }
}

fn handle_command(bot: &mut Bot, user: &str, msg: &str) {
    let modlist = &bot.modlist;
    match msg.trim_end() {
        "!join" => push(bot, user),
        "!leave" => remove(bot, user),
        "!position" => find(bot, user),
        "!length" => length(bot),
        // Mod commands
        "!next" => mod_command!(modlist, user, {
            shift(bot);
        }),
        "!clear" => mod_command!(modlist, user, {
            clear(bot);
        }),
        // Not a command
        default => {
            println!("{}: {}", user, default)
        }
    }
}
fn message_handler(bot: &mut Bot, msg: String) {
    match msg.as_str() {
        "PING :tmi.twitch.tv" => bot.send_raw("PONG :tmi.twitch.tv"),
        line if line.contains("PRIVMSG") => {
            // DEBUG
            for x in 1..101 {
                bot.send_msg(&format!("{}", x));
            }
            // END DEBUG
            let user = {
                let idx = line.find('!').unwrap();
                &line[1..idx]
            };
            let msg = {
                let idx = line[1..].find(':').unwrap();
                &line[idx + 2..]
            };
            handle_command(bot, user, msg);
        }
        line if line.contains("NOTICE") => {
            let prefix = "The moderators of this channel are: ";
            if let Some(idx) = line.find(prefix) {
                let modlist = line[idx + prefix.len()..].split(", ");
                bot.set_modlist(modlist);
                println!("Mods: {:#?}", bot.modlist)
            }
        }

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
        .expect("CHANNEL_NAME must be present in the config")
        .to_string();

    let mut bot = Bot::new(&channel_name);
    loop {
        bot.connect();
        bot.send_raw(&format!("PASS {}", oauth_token));
        bot.send_raw(&format!("NICK {}", bot_username));
        bot.send_raw(&format!("JOIN #{}", channel_name));
        bot.send_raw("CAP REQ :twitch.tv/commands");
        bot.send_msg("/mods");

        for result in bot.get_reader().lines() {
            match result {
                Ok(line) => message_handler(&mut bot, line),
                Err(e) => match e.kind() {
                    io::ErrorKind::ConnectionReset => {
                        let duration = time::Duration::from_secs(10);
                        colorprintln!(
                            Color::Blue,
                            "Connection reset, sleeping {}s ...",
                            duration.as_secs()
                        );
                        thread::sleep(duration);
                    }
                    _ => {
                        colorprintln!(Color::BgRed, "Error while reading from socket: {}", e);
                        match bot.disconnect() {
                            Ok(_) => colorprintln!(Color::Red, "Disconnected"),
                            Err(e) => {
                                colorprintln!(Color::Green, "Error while disconnecting: {}", e)
                            }
                        }
                    }
                },
            }
        }
    }
}
