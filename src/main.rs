#[macro_use]
extern crate lazy_static;

use std::io::prelude::*;
use std::io::BufReader;
use std::net::TcpStream;

mod config;

lazy_static! {
    static ref SOCKET: TcpStream =
        TcpStream::connect("irc.chat.twitch.tv:6667").expect("Couldn't connect to twitch servers");
    static ref CHANNEL_NAME: String = {
        config::read()
            .unwrap()
            .get("CHANNEL_NAME")
            .expect("CHANNEL_NAME must be present in the config")
            .to_string()
    };
}

fn mod_command<F: FnOnce()>(modlist: &[String], user: &str, f: F) {
    match modlist.contains(&user.to_string()) {
        true => f(),
        false => {
            println!("User {} not authorised to perform mod commands", user);
        }
    };
}

fn send_raw(msg: &str) {
    println!("Sending: {}", msg);
    (&*SOCKET)
        .write_all(format!("{}\r\n", msg).as_bytes())
        .unwrap_or_else(|err| panic!("Sending message failed: {}", err));
}

fn send_msg(msg: &str) {
    send_raw(&format!("PRIVMSG #{} :{}", *CHANNEL_NAME, msg));
}

fn clear(queue: &mut Vec<String>) {
    queue.clear();
    println!("Queue cleared");
}

fn push(queue: &mut Vec<String>, user: &str) {
    match queue.iter().position(|x| x == user) {
        Some(idx) => {
            send_msg(&format!(
                "@{}: You're already in queue at position {}",
                user,
                idx + 1
            ));
        }
        None => {
            queue.push(user.to_owned());
            send_msg(&format!(
                "@{}: You've been added to queue at position {}",
                user,
                queue.len()
            ));
        }
    }
}

fn remove(queue: &mut Vec<String>, user: &str) {
    match queue.iter().position(|x| x == user) {
        Some(idx) => {
            queue.remove(idx);
            send_msg(&format!("@{}: You've been removed from the queue", user));
        }
        None => {
            send_msg(&format!("@{}: You were not queued", user));
        }
    }
}

fn find(queue: &[String], user: &str) {
    match queue.iter().position(|x| x == user) {
        Some(idx) => {
            send_msg(&format!("@{} you are number {} in queue", user, idx + 1));
        }
        None => {
            send_msg(&format!("@{}: You're not currently queued", user));
        }
    }
}

fn shift(queue: &[String]) {
    match queue.first() {
        Some(user) => {
            send_msg(&format!("Next person in queue: @{}", user));
        }
        None => {
            send_msg("The queue is currently empty");
        }
    }
}

fn length(queue: &[String]) {
    {
        send_msg(&format!("There are {} people in queue", queue.len()));
    }
}

fn handle_command(modlist: &[String], queue: &mut Vec<String>, user: &str, msg: &str) {
    match msg.trim_end() {
        "!join" => push(queue, user),
        "!leave" => remove(queue, user),
        "!position" => find(queue, user),
        "!length" => length(queue),
        // Mod commands
        "!next" => mod_command(modlist, user, || shift(queue)),
        "!clear" => mod_command(modlist, user, || clear(queue)),
        // Not a command
        default => {
            println!("{}: {}", user, default)
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

    let reader = BufReader::new(&*SOCKET);
    let mut queue: Vec<String> = Vec::new();

    send_raw(&format!("PASS {}", oauth_token));
    send_raw(&format!("NICK {}", bot_username));
    send_raw(&format!("JOIN #{}", *CHANNEL_NAME));
    send_raw("CAP REQ :twitch.tv/commands");
    send_msg("/mods");

    let mut modlist: Vec<String> = vec![CHANNEL_NAME.to_string()];
    let mut lines = reader.lines();
    while let Some(Ok(line)) = lines.next() {
        match line.as_str() {
            "PING :tmi.twitch.tv" => send_raw("PONG :tmi.twitch.tv"),
            _ if line.contains("PRIVMSG") => {
                let user = {
                    let idx = line.find('!').unwrap();
                    &line[1..idx]
                };
                let msg = {
                    let idx = line[1..].find(':').unwrap();
                    &line[idx + 2..]
                };
                handle_command(&modlist, &mut queue, user, msg);
            }
            _ if line.contains("NOTICE") => {
                let prefix = "The moderators of this channel are: ";
                if let Some(idx) = line.find(prefix) {
                    modlist = vec![CHANNEL_NAME.to_string()];
                    modlist.extend(line[idx + prefix.len()..].split(", ").map(String::from));
                    println!("Mods: {:#?}", modlist)
                }
            }

            _ => {
                println!("Received: {}", line)
            }
        }
    }
}
