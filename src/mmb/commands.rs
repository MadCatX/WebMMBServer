use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use serde_json;

const KEY_FIRST_STAGE: &'static str = "firstStage";
const KEY_LAST_STAGE: &'static str = "lastStage";
const KEYLESS_ENTRIES: &'static [&'static str] = &["sequences", "doubleHelices", "baseInteractions", "ntcs"];
const IGNORED_KEYS: &'static [&'static str] = &[KEY_FIRST_STAGE, KEY_LAST_STAGE];

pub type MappedJson = HashMap<String, Vec<String>>;

pub struct Stages {
    pub first: i32,
    pub last: i32,
}

fn keyless_commands_item(keyless: &Vec<String>) -> String {
    keyless.iter().fold(String::new(), |p, c| format!("{}\n{}\n", p, c))
}

fn mapped_commands_to_txt(mapped: &MappedJson, stage: i32) -> Option<String> {
    let mut txt = String::new();
    let mut keyless = String::new();

    for (k, v) in mapped {
        if IGNORED_KEYS.contains(&k.as_str()) {
            continue;
        } else if KEYLESS_ENTRIES.contains(&k.as_str()) {
            let item = keyless_commands_item(&v);
            if k == "sequences" {
                keyless = format!("{}{}", item, keyless);
            } else {
                keyless.push_str(item.as_str());
            }
        } else {
            let mut item = format!("{} ", k);
            for i in v {
                item.push_str(format!("{} ", i).as_str());
            }
            item.push('\n');

            txt.push_str(item.as_str());
        }
    }

    txt.push_str(format!("{} {}\n", KEY_FIRST_STAGE, stage).as_str());
    txt.push_str(format!("{} {}\n", KEY_LAST_STAGE, stage).as_str());

    txt.push_str(keyless.as_str());
    Some(txt)
}

pub fn json_to_mapped(data: &serde_json::Value) -> Result<MappedJson, String> {
    match serde_json::from_value::<MappedJson>(data.clone()) {
        Ok(v) => Ok(v),
        Err(e) => Err(e.to_string()),
    }
}

pub fn stages(mapped: &MappedJson) -> Option<Stages> {
    let mut first: Option<i32> = None;
    let mut last: Option<i32> = None;

    for (k, v) in mapped {
        if v.len() < 1 {
            continue;
        }
        if k == KEY_FIRST_STAGE {
            match v[0].parse::<i32>() {
                Ok(v) => first = Some(v),
                Err(_) => return None,
            }
        } else if k == KEY_LAST_STAGE {
            match v[0].parse::<i32>() {
                Ok(v) => last = Some(v),
                Err(_) => return None,
            }
        }

        if first.is_some() && last.is_some() {
            return Some(Stages{ first: first.unwrap(), last: last.unwrap() })
        }
    }

    return None;
}

pub fn write(path: &PathBuf, mapped: &MappedJson, stage: i32) -> Result<(), String> {
    let parsed = mapped_commands_to_txt(mapped, stage);
    if parsed.is_none() {
        return Err(String::from("Invalid MMB commands"));
    }
    
    let mut fh = match File::create(path) {
        Ok(f) => f,
        Err(e) => return Err(e.to_string())
    };

    let txt_cmds = parsed.unwrap();

    match fh.write_all(txt_cmds.as_bytes()) {
        Ok(_) => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}
