use std::collections::HashMap;
use std::fs::File;
use std::io::prelude::*;
use std::io::{BufReader, Error};

pub fn read() -> Result<HashMap<String, String>, Error> {
    let file = File::open("config.txt")?;
    let reader = BufReader::new(file);

    Ok(HashMap::from_iter(
        reader
            .lines()
            .map(move |line| {
                line.unwrap()
                    .split_once('=')
                    .map(|(k, v)| (k.to_string(), v.to_string()))
            })
            .flatten(),
    ))
}
