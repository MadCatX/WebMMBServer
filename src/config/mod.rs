use std::io::prelude::*;
use std::path::PathBuf;
use lazy_static::lazy_static;
use serde_derive::Deserialize;
use serde_json;

use crate::logging;
use crate::log_plain;

const LOGSRC: &'static str= "config";

#[derive(Deserialize)]
pub struct Config {
    pub mmb_exec_path: String,
    pub mmb_parameters_path: String,
    pub jobs_dir: String,
    pub examples_dir: String,

    pub root_dir: String,

    pub domain: String,
    pub port: u16,
    pub require_https: bool,
    pub use_pbs_offloading: bool,
    pub verbose_rocket_logging: bool,

    pub log_file: Option<String>,
}
lazy_static! {
    static ref CONFIG: Config = {
        load(PathBuf::from("./cfg.json"))
    };
}

fn check_dir_exists(path: &str) {
    let p = std::path::Path::new(path);
    if !std::path::Path::is_dir(p) {
        log_plain!(Critical, LOGSRC, &format!("Invalid configuration, {} does not exist or it is not a directory", path));
        panic!();
    }
}

fn check_file_exists(path: &str) {
    let p = std::path::Path::new(path);
    if !std::path::Path::is_file(p) {
        log_plain!(Critical, LOGSRC, &format!("Invalid configuration, {} does not exist or it is not a file", path));
        panic!();
    }
}

fn read_config(path: &PathBuf) -> String {
    let mut s = String::new();
    let fh = std::fs::File::open(path).expect("Failed to open configuration file");
    let mut reader = std::io::BufReader::new(fh);
    reader.read_to_string(&mut s).expect("Failed to read configuration file");

    s
}

fn load(cfg_path: PathBuf) -> Config {
    let cfg: Config = match serde_json::from_str(read_config(&cfg_path).as_str()) {
        Ok(cfg) => cfg,
        Err(e) => {
            log_plain!(Critical, LOGSRC, &format!("Failed to parse configuation file: {}", e.to_string().as_str()));
            panic!();
        }
    };

    check_file_exists(&cfg.mmb_exec_path);
    check_file_exists(&cfg.mmb_parameters_path);
    check_dir_exists(&cfg.examples_dir);
    check_dir_exists(&cfg.root_dir);
    if cfg.domain.len() < 1 {
        log_plain!(Critical, LOGSRC, "Invalid configuration - no domain name: {}");
        panic!();
    }
    if cfg.port == 0 {
        log_plain!(Critical, LOGSRC, "Invalid configuration - port number cannot be zero");
        panic!();
    }

    cfg
}

pub fn get() -> &'static Config {
    &CONFIG
}
