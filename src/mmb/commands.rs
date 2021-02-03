use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use serde_json;

use crate::mmb::advanced_commands;

const ADV_PARAMS: &'static str = "advParams";
const KEY_FIRST_STAGE: &'static str = "firstStage";
const KEY_LAST_STAGE: &'static str = "lastStage";
const KEY_NUM_REP_INTVLS: &'static str = "numReportingIntervals";
const KEYLESS_ENTRIES: &'static [&'static str] = &["sequences", "doubleHelices", "baseInteractions", "ntcs"];
const IGNORED_KEYS: &'static [&'static str] = &[KEY_FIRST_STAGE, KEY_LAST_STAGE];

pub type MappedJson = HashMap<String, serde_json::Value>;

pub struct Stages {
    pub first: i32,
    pub last: i32,
}

fn keyless_commands_item(keyless: &Vec<String>) -> String {
    keyless.iter().fold(String::new(), |p, c| format!("{}\n{}\n", p, c))
}

fn get_value_from_raw<T, F>(lines: &Vec<&str>, key: &'static str, converter: F) -> Option<T> where F: Fn(&str) -> Option<T> {
    let lwr_key = key.to_lowercase();

    for l in lines {
        let segments = l.trim().split(" ").collect::<Vec<_>>();
        if segments.len() < 2 {
            continue;
        }

        if segments[0].to_lowercase() != lwr_key {
            continue;
        }

        for idx in 1..segments.len() {
            if segments[idx].len() < 1 {
                continue;
            }
            return converter(segments[idx]);
        }
    }

    None
}

fn mapped_commands_to_txt(mapped: &MappedJson, stage: i32) -> Result<String, serde_json::Error> {
    let mut txt = String::new();
    let mut keyless = String::new();
    let mut advanced = String::new();

    txt.push_str(format!("{} {}\n", KEY_FIRST_STAGE, stage).as_str());
    txt.push_str(format!("{} {}\n", KEY_LAST_STAGE, stage).as_str());

    for (k, v) in mapped {
        if IGNORED_KEYS.contains(&k.as_str()) {
            continue;
        } else if k == ADV_PARAMS {
            match advanced_commands::advanced_to_string(v.clone()) {
                Ok(s) => advanced = s,
                Err(e) => return Err(e),
            }
        } else if KEYLESS_ENTRIES.contains(&k.as_str()) {
            let sv = serde_json::from_value::<Vec<String>>(v.clone())?;

            let item = keyless_commands_item(&sv);
            if k == "sequences" {
                keyless = format!("{}{}", item, keyless);
            } else {
                keyless.push_str(item.as_str());
            }
        } else {
            let sv = serde_json::from_value::<Vec<String>>(v.clone())?;

            let mut item = format!("{} ", k);
            for i in sv {
                item.push_str(format!("{} ", i).as_str());
            }
            item.push('\n');

            txt.push_str(item.as_str());
        }
    }

    txt.push_str(advanced.as_str());
    txt.push_str(keyless.as_str());
    Ok(txt)
}

pub struct ParsedRaw {
    pub first_stage: i32,
    pub last_stage: i32,
    pub num_reporting_intervals: i32,
}

pub fn json_to_mapped(data: &serde_json::Value) -> Result<MappedJson, String> {
    match serde_json::from_value::<MappedJson>(data.clone()) {
        Ok(v) => Ok(v),
        Err(e) => Err(e.to_string()),
    }
}

pub fn parse_raw(raw: &str) -> Result<ParsedRaw, String> {
    let converter = |s: &str| -> Option<i32> {
        match s.parse::<i32>() {
            Ok(v) => Some(v),
            Err(_) => None,
        }
    };
    let lines = raw.split("\n").collect::<Vec<_>>();

    let first_stage = match get_value_from_raw(&lines, KEY_FIRST_STAGE, converter) {
        Some(v) => v,
        None => return Err(format!("{} was not specified or is invalid", KEY_FIRST_STAGE)),
    };
    let last_stage = match get_value_from_raw(&lines, KEY_LAST_STAGE, converter) {
        Some(v) => v,
        None => return Err(format!("{} was not specified or is invalid", KEY_LAST_STAGE)),
    };
    let num_reporting_intervals = match get_value_from_raw(&lines, KEY_NUM_REP_INTVLS, converter) {
        Some(v) => v,
        None => return Err(format!("{} was not specified or is invalid", KEY_NUM_REP_INTVLS)),
    };

    if first_stage < 1 {
        return Err(format!("{} must be positive", KEY_FIRST_STAGE));
    }
    if first_stage != last_stage {
        return Err(String::from("Multi-stage jobs are currently not supported"));
    }
    if num_reporting_intervals < 1 {
        return Err(format!("{} must be positive", KEY_NUM_REP_INTVLS));
    }

    Ok(
        ParsedRaw {
            first_stage,
            last_stage,
            num_reporting_intervals
        }
    )
}

pub fn stages(mapped: &MappedJson) -> Result<Stages, String> {
    let mut first: Option<i32> = None;
    let mut last: Option<i32> = None;

    for (k, v) in mapped {
        if k == KEY_FIRST_STAGE {
            let sv = match serde_json::from_value::<Vec<String>>(v.clone()) {
                Ok(sv) => sv,
                Err(e) => return Err(e.to_string()),
            };
            if sv.len() != 1 {
                return Err(format!("Invalid vector size for {}", KEY_FIRST_STAGE));
            }

            match sv[0].parse::<i32>() {
                Ok(v) => first = Some(v),
                Err(e) => return Err(e.to_string()),
            }
        } else if k == KEY_LAST_STAGE {
            let sv = match serde_json::from_value::<Vec<String>>(v.clone()) {
                Ok(sv) => sv,
                Err(e) => return Err(e.to_string()),
            };
            if sv.len() != 1 {
                return Err(format!("Invalid vector size for {}", KEY_LAST_STAGE));
            }

            match sv[0].parse::<i32>() {
                Ok(v) => last = Some(v),
                Err(e) => return Err(e.to_string()),
            }
        }

        if first.is_some() && last.is_some() {
            return Ok(Stages{ first: first.unwrap(), last: last.unwrap() })
        }
    }

    return Err(String::from("Stages are not defined properly"));
}

pub fn write(path: &PathBuf, mapped: &MappedJson, stage: i32) -> Result<(), String> {
    let parsed = match mapped_commands_to_txt(mapped, stage) {
        Ok(parsed) => parsed,
        Err(e) => return Err(format!("Invalid MMB commands: {}", e.to_string())),
    };

    write_raw(path, &parsed)
}

pub fn write_raw(path: &PathBuf, raw_commands: &str) -> Result<(), String> {
    let mut fh = match File::create(path) {
        Ok(f) => f,
        Err(e) => return Err(e.to_string())
    };

    match fh.write_all(raw_commands.as_bytes()) {
        Ok(_) => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}
