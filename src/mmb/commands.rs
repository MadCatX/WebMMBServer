use std::collections::HashMap;
use std::convert::TryInto;
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

struct AuthChainMapping {
    pub auth_name: String,
    pub auth_residues: Vec<i32>,
}

type AuthMapping = HashMap<String, AuthChainMapping>;

impl std::fmt::Display for api::BondMobility {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            api::BondMobility::Rigid => write!(f, "Rigid"),
            api::BondMobility::Torsion => write!(f, "Torsion"),
            api::BondMobility::Free => write!(f, "Free"),
        }
    }
}

impl std::fmt::Display for api::EdgeInteraction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            api::EdgeInteraction::WatsonCrick => write!(f, "WatsonCrick"),
            api::EdgeInteraction::SugarEdge=> write!(f, "SugarEdge"),
        }
    }
}

impl std::fmt::Display for api::Orientation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            api::Orientation::Cis => write!(f, "Cis"),
            api::Orientation::Trans => write!(f, "Trans"),
        }
    }
}

fn _get_auth_res_no(ch: &AuthChainMapping, no: i32) -> Option<i32> {
    let idx: usize = match (no - 1).try_into() {
        Ok(v) => v,
        Err(_) => return None,
    };
    if ch.auth_residues.len() <= idx {
        return None;
    } else {
        return Some(ch.auth_residues[idx]);
    }
}

fn _mk_auth_mapping(compounds: &Vec<api::Compound>) -> AuthMapping {
    let mut mapping = HashMap::<String, AuthChainMapping>::new();
    for c in compounds.iter() {
        let mut ch = AuthChainMapping{
            auth_name: c.chain.auth_name.clone(),
            auth_residues: Vec::<i32>::new(),
        };
        for res in c.residues.iter() {
            ch.auth_residues.push(res.auth_number);
        }
        mapping.insert(c.chain.name.clone(), ch);
    }
    return mapping;
}

fn base_interactions_to_txt(bis: &Vec<api::BaseInteraction>, mapping: &AuthMapping) -> Result<String, String> {
    let mut txt = String::new();
    for bi in bis.iter() {
        let ch_1 = match mapping.get(&bi.chain_name_1) {
            Some(v) => v,
            None => return Err(String::from("No mapping for chain name")),
        };
        let ch_2 = match mapping.get(&bi.chain_name_2) {
            Some(v) => v,
            None => return Err(String::from("No mapping for chain name")),
        };

        let res_no_auth_1 = match _get_auth_res_no(&ch_1, bi.res_no_1) {
            Some(v) => v,
            None => return Err(String::from("Cannot get auth_res_no for res_no_1")),
        };
        let res_no_auth_2 = match _get_auth_res_no(&ch_2, bi.res_no_2) {
            Some(v) => v,
            None => return Err(String::from("Cannot get auth_res_no for res_no_2")),
        };
        txt += format!(
            "baseInteraction {} {} {} {} {} {} {}\n",
            ch_1.auth_name, res_no_auth_1, bi.edge_1,
            ch_2.auth_name, res_no_auth_2, bi.edge_2,
            bi.orientation
        ).as_str();
    }
    Ok(txt)
}

fn compounds_to_txt(compounds: &Vec<api::Compound>) -> Result<String, String> {
    let mut txt = String::new();
    for c in compounds.iter() {
        let ctype = match c.ctype {
            api::CompoundType::DNA => "DNA",
            api::CompoundType::Protein => "Protein",
            api::CompoundType::RNA => "RNA",
        };
        let res_no = if c.residues.len() > 0 {
            c.residues[0].auth_number
        } else {
            return Err(String::from("Compound does not have any residues"));
        };
        txt += format!("{} {} {} {}\n", ctype, c.chain.auth_name, res_no, c.sequence).as_str();
    }
    Ok(txt)
}

fn double_helices_to_txt(dhs: &Vec<api::DoubleHelix>, mapping: &AuthMapping) -> Result<String, String> {
    let mut txt = String::new();
    for dh in dhs.iter() {
        let ch_1 = match mapping.get(&dh.chain_name_1) {
            Some(v) => v,
            None => return Err(String::from("No mapping for chain name")),
        };
        let ch_2 = match mapping.get(&dh.chain_name_2) {
            Some(v) => v,
            None => return Err(String::from("No mapping for chain name")),
        };

        let first_res_no_auth_1 = match _get_auth_res_no(&ch_1, dh.first_res_no_1) {
            Some(v) => v,
            None => return Err(String::from("Cannot get auth_res_no for first_res_no_1")),
        };
        let last_res_no_auth_1 = match _get_auth_res_no(&ch_1, dh.last_res_no_1) {
            Some(v) => v,
            None => return Err(String::from("Cannot get auth_res_no last_res_no_1")),
        };
        let first_res_no_auth_2 = match _get_auth_res_no(&ch_2, dh.first_res_no_2) {
            Some(v) => v,
            None => return Err(String::from("Cannot get auth_res_no for first_res_no_1")),
        };
        let last_res_no_auth_2 = match _get_auth_res_no(&ch_2, dh.last_res_no_2) {
            Some(v) => v,
            None => return Err(String::from("Cannot get auth_res_no for last_res_no_2")),
        };
        txt += format!(
            "nucleicAcidDuplex {} {} {} {} {} {}\n",
            ch_1.auth_name, first_res_no_auth_1, last_res_no_auth_1,
            ch_2.auth_name, first_res_no_auth_2, last_res_no_auth_2
        ).as_str();
    }
    Ok(txt)
}

fn filename_to_txt(key: &str, value: &str) -> Result<String, String> {
    if value.len() < 1 {
        return Err(String::from("File name of zero length"));
    }
    if value.find("/").is_some() {
        return Err(format!("Invalid character in file name for command {}", key));
    }
    if value.find("\\").is_some() {
        return Err(format!("Invalid character in file name for command {}", key));
    }

    Ok(keyed_to_txt(key, value))
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

fn mobilizers_to_txt(mobilizers: &Vec<api::Mobilizer>, mapping: &AuthMapping) -> Result<String, String> {
    let mut txt = String::new();

    for m in mobilizers.iter() {
        let mut line = format!("mobilizer {}", m.bond_mobility);

        if m.chain.is_some() {
            let ch = match mapping.get(m.chain.as_ref().unwrap()) {
                Some(v) => v,
                None => return Err(String::from("No mapping for chain")),
            };

            line.push_str(format!(" {}", ch.auth_name).as_str());

            if m.first_residue.is_some() && m.last_residue.is_some() {
                let auth_first = match _get_auth_res_no(&ch, m.first_residue.unwrap()) {
                    Some(v) => v,
                    None => return Err(String::from("No auth_res_no for first_residue")),
                };
                let auth_last = match _get_auth_res_no(&ch, m.last_residue.unwrap()) {
                    Some(v) => v,
                    None => return Err(String::from("No auth_res_no for last_residue")),
                };
                line.push_str(format!(" {} {}", auth_first, auth_last).as_str());
            }
        }

        txt += (line + "\n").as_str();
    }

    Ok(txt)
}

fn ntcs_to_txt(ntcs: &api::NtCs, mapping: &AuthMapping) -> Result<String, String> {
    let mut txt = String::new();
    if ntcs.conformations.len() == 0 {
        return Ok(txt);
    }

    for ntc in ntcs.conformations.iter() {
        let ch = match mapping.get(&ntc.chain_name) {
            Some(v) => v,
            None => return Err(String::from("No mapping for chain name")),
        };
        let first_res_no_auth = match _get_auth_res_no(&ch, ntc.first_res_no) {
            Some(v) => v,
            None => return Err(String::from("Cannot get auth_res_no for first_res_no")),
        };
        let last_res_no_auth = match _get_auth_res_no(&ch, ntc.last_res_no) {
            Some(v) => v,
            None => return Err(String::from("Cannot get auth_res_no for last_res_no")),
        };
        txt += format!(
            "NtC {} {} {} {}\n",
            ch.auth_name, first_res_no_auth, last_res_no_auth, ntc.ntc
        ).as_str();
    }
    txt += format!("NtCForceScaleFactor {}\n", ntcs.force_scale_factor).as_str();
    Ok(txt)
}

fn common_commands_to_txt(common: &api::Commands, stage: i32) -> Result<String, String> {
    let mut txt = String::new();

    txt += keyed_to_txt(KEY_FIRST_STAGE, stage).as_str();
    txt += keyed_to_txt(KEY_LAST_STAGE, stage).as_str();

    txt += keyed_to_txt("reportingInterval", common.reporting_interval).as_str();
    txt += keyed_to_txt(KEY_NUM_REP_INTVLS, common.num_reporting_intervals).as_str();

    txt += keyed_to_txt(KEY_BASE_ITRS_SF, common.base_interaction_scale_factor).as_str();
    txt += keyed_to_txt("temperature", common.temperature).as_str();

    Ok(txt)
}

fn density_fit_commands_to_txt(common: &api::Commands, concrete: &api::DensityFitCommands, stage: i32) -> Result<String, String> {
    match common_commands_to_txt(common, stage) {
        Ok(mut txt) => {
            let auth_mapping = _mk_auth_mapping(&concrete.compounds);

            if concrete.set_default_MD_parameters {
                txt += "setDefaultMDParameters\n";
            }

            match filename_to_txt("loadSequencesFromPdb", &concrete.structure_file_name) {
                Ok(s) => txt += s.as_str(),
                Err(e) => return Err(e),
            };
            match filename_to_txt("density densityFileName", &concrete.density_map_file_name) {
                Ok(s) => txt += s.as_str(),
                Err(e) => return Err(e),
            };
            txt += match mobilizers_to_txt(&concrete.mobilizers, &auth_mapping) {
                Ok(v) => v,
                Err(e) => return Err(e),
            }.as_str();
            txt += match ntcs_to_txt(&concrete.ntcs, &auth_mapping) {
                Ok(v) => v,
                Err(e) => return Err(e),
            }.as_str();
            txt += "fitToDensity\n";

            Ok(txt)
        },
        Err(e) => Err(e),
    }
}

fn standard_commands_to_txt(common: &api::Commands, concrete: &api::StandardCommands, stage: i32) -> Result<String, String> {
    match common_commands_to_txt(common, stage) {
        Ok(mut txt) => {
            if concrete.set_default_MD_parameters {
                txt += "setDefaultMDParameters\n";
            }

            let auth_mapping = _mk_auth_mapping(&concrete.compounds);

            match compounds_to_txt(&concrete.compounds) {
                Ok(s) => txt += s.as_str(),
                Err(e) => return Err(e),
            };
            txt += match double_helices_to_txt(&concrete.double_helices, &auth_mapping) {
                Ok(v) => v,
                Err(e) => return Err(e),
            }.as_str();
            txt += match base_interactions_to_txt(&concrete.base_interactions, &auth_mapping) {
                Ok(v) => v,
                Err(e) => return Err(e),
            }.as_str();
            txt += match ntcs_to_txt(&concrete.ntcs, &auth_mapping) {
                Ok(v) => v,
                Err(e) => return Err(e),
            }.as_str();

            txt += match mobilizers_to_txt(&concrete.mobilizers, &auth_mapping) {
                Ok(v) => v,
                Err(e) => return Err(e),
            }.as_str();
            match advanced_params::to_txt(&concrete.adv_params) {
                Ok(s) => txt += s.as_str(),
                Err(e) => return Err(e.to_string()),
            }

            Ok(txt)
        },
        Err(e) => Err(e)
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

pub fn stages(commands: &api::Commands) -> Result<Stages, String> {
    if commands.last_stage < commands.first_stage {
        return Err(String::from("Last stage number cannot be lower than first stage"))
    }

    Ok(Stages { first: commands.first_stage, last: commands.last_stage })
}

pub fn write(path: &PathBuf, mapped: &api::Commands, stage: i32) -> Result<(), String> {
    let parsed = match &mapped.concrete {
        api::ConcreteCommands::DensityFit(v) => match density_fit_commands_to_txt(&mapped, &v, stage) {
            Ok(parsed) => parsed,
            Err(e) => return Err(format!("Invalid MMB commands for density fit job: {}", e.to_string())),
        },
        api::ConcreteCommands::Standard(v) => match standard_commands_to_txt(&mapped, &v, stage) {
            Ok(parsed) => parsed,
            Err(e) => return Err(format!("Invalid MMB commands for standard job: {}", e.to_string())),
        },
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
