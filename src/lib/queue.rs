use serde::{Deserialize, Serialize};
use std::fs;
use std::io::ErrorKind;

pub const DATA_DIR: &str = "data/";

#[derive(Serialize, Deserialize)]
pub struct Queue {
    pub name: String,
    list: Vec<String>,
}

impl Queue {
    fn slugify(name: &str) -> String {
        name.to_lowercase().replace(' ', "-")
    }

    fn filename(&self) -> String {
        format!("{}{}.json", DATA_DIR, Self::slugify(&self.name))
    }

    pub fn find(&self, user: &str) -> Option<usize> {
        self.list.iter().position(|x| x == user)
    }

    pub fn first(&mut self) -> Option<&String> {
        self.list.first()
    }

    pub fn len(&self) -> usize {
        self.list.len()
    }

    pub fn list(&self) -> &[String] {
        &self.list
    }

    pub fn clear(&mut self) {
        self.list.clear()
    }

    pub fn new(name: &str) -> Self {
        let new = Self {
            name: name.to_owned(),
            list: Vec::new(),
        };
        fs::File::create(new.filename()).unwrap();
        new.save();
        new
    }

    pub fn load(name: &str) -> Option<Self> {
        Some(
            serde_json::from_str(&match fs::read_to_string(format!(
                "data/{}.json",
                Self::slugify(name)
            )) {
                Ok(list) => list,
                Err(error) => match error.kind() {
                    ErrorKind::NotFound => return None,
                    _ => panic!("Error loading queue file: {}", error),
                },
            })
            .unwrap(),
        )
    }

    pub fn save(&self) {
        let filename = self.filename();
        println!("Trying to save queue {} to {}", self.name, filename);
        let file = fs::OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(filename)
            .unwrap();
        serde_json::to_writer(file, self).unwrap()
    }

    pub fn push(&mut self, user: &str) -> Result<usize, usize> {
        match self.find(user) {
            Some(idx) => Err(idx),
            None => {
                self.list.push(user.to_owned());
                Ok(self.list.len() - 1)
            }
        }
    }

    pub fn shift(&mut self) -> Option<String> {
        match self.list.is_empty() {
            true => None,
            false => Some(self.list.remove(0)),
        }
    }

    pub fn remove(&mut self, user: &str) -> Result<(), ()> {
        match self.find(user) {
            None => Err(()),
            Some(idx) => {
                self.list.remove(idx);
                Ok(())
            }
        }
    }
}
