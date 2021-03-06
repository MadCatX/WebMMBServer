use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

use crate::mmb::advanced_params;
use crate::server::api;

const KEY_FIRST_STAGE: &'static str = "firstStage";
const KEY_LAST_STAGE: &'static str = "lastStage";
const KEY_BASE_ITRS_SF: &'static str = "baseInteractionScaleFactor";
const KEY_NUM_REP_INTVLS: &'static str = "numReportingIntervals";

pub struct ParsedRaw {
    pub first_stage: i32,
    pub last_stage: i32,
    pub num_reporting_intervals: i32,
}

pub struct Stages {
    pub first: i32,
    pub last: i32,
}

impl std::convert::From<&api::BondMobility> for String {
    fn from(bm: &api::BondMobility) -> String {
        match bm {
            api::BondMobility::Rigid => String::from("Rigid"),
            api::BondMobility::Torsion => String::from("Torsion"),
            api::BondMobility::Free => String::from("Free"),
        }
    }
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

fn keyed_to_txt<T: std::fmt::Display>(key: &str, value: T) -> String {
    format!("{} {}\n", key, value)
}


fn keyless_to_txt(keyless: &Vec<String>) -> String {
    keyless.iter().fold(String::new(), |p, c| format!("{}\n{}\n", p, c))
}

fn mobilizers_to_txt(mobilizers: &Vec<api::Mobilizer>) -> String {
    let mut txt = String::new();

    for m in mobilizers.iter() {
        let mut line = format!("mobilizer {}", String::from(&m.bond_mobility));

        if m.chain.is_some() {
            line.push_str(format!(" {}", m.chain.as_ref().unwrap()).as_str());

            if m.first_residue.is_some() && m.last_residue.is_some() {
                line.push_str(format!(" {} {}", m.first_residue.unwrap(), m.last_residue.unwrap()).as_str());
            }
        }

        txt += (line + "\n").as_str();
    }

    txt
}

fn commands_to_txt(commands: &api::JsonCommands, stage: i32) -> Result<String, String> {
    let mut txt = String::new();

    txt += keyed_to_txt(KEY_FIRST_STAGE, stage).as_str();
    txt += keyed_to_txt(KEY_LAST_STAGE, stage).as_str();

    txt += keyed_to_txt(KEY_BASE_ITRS_SF, commands.base_interaction_scale_factor).as_str();
    txt += keyed_to_txt("useMultithreadedComputation", commands.use_multithreaded_computation).as_str();
    txt += keyed_to_txt("temperature", commands.temperature).as_str();
    txt += keyed_to_txt("reportingInterval", commands.reporting_interval).as_str();
    txt += keyed_to_txt(KEY_NUM_REP_INTVLS, commands.num_reporting_intervals).as_str();

    if commands.set_default_MD_parameters {
        txt += "setDefaultMDParameters";
    }

    txt += keyless_to_txt(&commands.sequences).as_str();
    txt += keyless_to_txt(&commands.double_helices).as_str();
    txt += keyless_to_txt(&commands.base_interactions).as_str();
    txt += keyless_to_txt(&commands.ntcs).as_str();

    txt += mobilizers_to_txt(&commands.mobilizers).as_str();
    match advanced_params::to_txt(&commands.adv_params) {
        Ok(s) => txt += s.as_str(),
        Err(e) => return Err(e.to_string()),
    }

    Ok(txt)
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

pub fn stages(commands: &api::JsonCommands) -> Result<Stages, String> {
    if commands.last_stage < commands.first_stage {
        return Err(String::from("Last stage number cannot be lower than first stage"))
    }

    Ok(Stages { first: commands.first_stage, last: commands.last_stage })
}

pub fn write(path: &PathBuf, mapped: &api::JsonCommands, stage: i32) -> Result<(), String> {
    let parsed = match commands_to_txt(mapped, stage) {
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
