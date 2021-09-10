use std::collections::HashMap;
use std::fs::File;
use std::io::prelude::*;
use std::io::{BufReader, Error};

pub fn read() -> Result<HashMap<String, String>, Error> {
    let file = File::open("config.txt")?;
    let reader = BufReader::new(file);

    Ok(HashMap::from_iter(reader.lines().map(|line| {
        let line = line.unwrap();
        let results: Vec<&str> = line.splitn(2, '=').collect();
        if results.len() == 2 {
            (results[0].trim().to_string(), results[1].trim().to_string())
        } else {
            panic!("Malformed config entry '{}'", line);
        }
    })))
}
