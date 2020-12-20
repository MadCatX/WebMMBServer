use std::fs;
use std::path::PathBuf;
use serde::Deserialize;
use serde_json;

const EXAMPLE_COMMANDS_NAME: &'static str = "commands.json";
const LIST_MANIFEST_NAME: &'static str = "list.json";

#[derive(Deserialize)]
pub struct Example {
    pub name: String,
    pub description: String,
    dir: String,
}

pub type List = Vec<Example>;

pub fn example_data(mut path: PathBuf, name: &String) -> Result<String, String> {
    let list = get_examples(path.clone())?;
    let example = match list.into_iter().find(|item| item.name == *name) {
        Some(v) => v,
        None => return Err(format!("Example named {} does not exist", name)),
    };

    path.push(example.dir); path.push(EXAMPLE_COMMANDS_NAME);
    if !path.exists() {
        return Err(format!("Data for example {} is not available", name));
    }

    match fs::read_to_string(path) {
        Ok(v) => Ok(v),
        Err(e) => Err(format!("Cannot read data for example {}: {}", name, e.to_string())),
    }
}

pub fn get_examples(mut path: PathBuf) -> Result<List, String> {
    if !path.exists() {
        return Err(String::from("Path to examples data does not exist"));
    }

    path.push(LIST_MANIFEST_NAME);
    let data = match fs::read_to_string(path) {
        Ok(v) => v,
        Err(e) => return Err(e.to_string()),
    };
    let list = match serde_json::from_str::<List>(data.as_str()) {
        Ok(v) => v,
        Err(e) => return Err(e.to_string()),
    };

    Ok(list)
}
