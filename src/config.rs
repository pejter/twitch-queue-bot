use std::collections::HashMap;
use std::fs::File;
use std::io::prelude::*;
use std::io::{BufReader, Error, ErrorKind};

pub fn read() -> Result<HashMap<String, String>, Error> {
    let file = File::open("config.txt")?;
    let reader = BufReader::new(file);

    let mut map = HashMap::new();
    for line in reader.lines() {
        let line = line.unwrap();
        let results: Vec<&str> = line.splitn(2, '=').collect();
        match results.len() {
            2 => {
                map.insert(results[0].trim().to_string(), results[1].trim().to_string());
            }
            _ => {
                return Err(Error::new(ErrorKind::Other, "Malformed config"));
            }
        }
    }

    Ok(map)
}
