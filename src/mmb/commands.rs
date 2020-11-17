use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use serde_json;

const KEYLESS_ENTRIES: &'static [&'static str] = &["sequences", "doubleHelices", "baseInteractions", "ntcs"];

fn is_item_keyless(name: &str) -> bool {
    KEYLESS_ENTRIES.contains(&name)
}

fn keyless_commands_item(keyless: &Vec<String>) -> String {
    keyless.iter().fold(String::new(), |p, c| format!("{}\n{}\n", p, c))
}

fn mapped_commands_to_txt(data: &serde_json::Value) -> Option<String> {
    let parsed: serde_json::Result<HashMap<String, Vec<String>>> = serde_json::from_value(data.clone());
    if parsed.is_err() {
        return None;
    }

    let mapped = parsed.unwrap();
    let mut txt = String::new();
    let mut keyless = String::new();

    for (k, v) in &mapped {
        if is_item_keyless(k.as_str()) {
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

    txt.push_str(keyless.as_str());
    Some(txt)
}

pub fn write_commands(path: &PathBuf, commands: &serde_json::Value) -> Result<(), String> {
    let parsed = mapped_commands_to_txt(&commands);
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
