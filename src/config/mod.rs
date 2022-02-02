use std::io::prelude::*;
use std::path::{Path, PathBuf};
use rand;
use rand::Rng;
use std::sync::Mutex;
use base64;
use lazy_static::lazy_static;
use serde_derive::Deserialize;
use serde_json;

use crate::logging;
use crate::log_early;
use crate::log_plain;

const LOGSRC: &'static str = "config";

#[derive(Clone, Deserialize)]
pub struct LogToStdOut(bool);
impl LogToStdOut {
    pub fn get(&self) -> bool { self.0 }
}
impl Default for LogToStdOut {
    fn default() -> Self { LogToStdOut(false) }
}

#[derive(Clone, Deserialize)]
pub struct Config {
    pub mmb_exec_path: String,
    pub mmb_parameters_path: String,

    pub jobs_dir: String,
    pub examples_dir: String,
    pub root_dir: String,

    pub domain: String,
    pub port: u16,
    #[serde(default = "oneshot_secret_key")]
    pub secret_key: String,
    pub require_https: bool,
    pub use_pbs_offloading: bool,
    pub verbose_rocket_logging: bool,

    pub log_file: Option<String>,
    #[serde(default)]
    pub log_to_stdout: LogToStdOut,
}
lazy_static! {
    static ref CONFIG: Mutex<Config> = Mutex::new(
        Config{
            mmb_exec_path: String::from("/usr/bin/MMB"),
            mmb_parameters_path: String::from("/usr/share/include/MMB/parameters.csv"),
            jobs_dir: String::from("/tmp/webmmb_server"),
            examples_dir: String::from("/srv/www/webmmb_server/examples"),
            root_dir: String::from("/srv/www/webmmb_server/"),
            secret_key: oneshot_secret_key(),
            domain: String::from("localhost"),
            port: 443,
            require_https: true,
            use_pbs_offloading: false,
            verbose_rocket_logging: false,
            log_file: Some(String::from("/var/log/webmmb_server.log")),
            log_to_stdout: LogToStdOut(true),
        }
    );
}

fn oneshot_secret_key() -> String {
    let mut rng = rand::thread_rng();
    let mut random_blob = Vec::from(rng.gen::<[u8; 32]>());
    random_blob.append(&mut Vec::from(rng.gen::<[u8; 32]>()));

    base64::encode(random_blob)
}

fn read_config(path: &PathBuf) -> String {
    match std::fs::File::open(path) {
        Ok(fh) => {
            let mut s = String::new();
            let mut reader = std::io::BufReader::new(fh);
            match reader.read_to_string(&mut s) {
                Ok(_) => s,
                Err(e) => {
                    log_early!(Critical, LOGSRC, &format!("Failed to read configuration file: {}", e.to_string()));
                    panic!();
                },
            }
        },
        Err(e) => {
            log_early!(Critical, LOGSRC, &format!("Failed to open configuration file: {}", e.to_string()));
            panic!();
        },
    }
}

pub fn get() -> Config {
    CONFIG.lock().unwrap().clone()
}

pub fn load(cfg_path: PathBuf) {
    if !Path::new(&cfg_path).is_file() {
        log_early!(Warning, LOGSRC, "No configuration files. Continuing with defaults but this will most likely fail.");
        return;
    }

    let cfg: Config = match serde_json::from_str(read_config(&cfg_path).as_str()) {
        Ok(cfg) => cfg,
        Err(e) => {
            log_plain!(Critical, LOGSRC, &format!("Failed to parse configuation file: {}", e.to_string()));
            panic!();
        }
    };

    if cfg.domain.len() < 1 {
        log_plain!(Critical, LOGSRC, "Invalid configuration - no domain name: {}");
        panic!();
    }
    if cfg.port == 0 {
        log_plain!(Critical, LOGSRC, "Invalid configuration - port number cannot be zero");
        panic!();
    }

    *CONFIG.lock().unwrap() = cfg;
}
