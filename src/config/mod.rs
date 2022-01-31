use std::io::prelude::*;
use serde_derive::Deserialize;
use serde_json;

use crate::logging;

const LOGSRC: &'static str= "config";

static mut CONFIG: ConfigContainer = ConfigContainer{
    config: Config{
        mmb_exec_path: String::new(),
        mmb_parameters_path: String::new(),
        jobs_dir: String::new(),
        examples_dir: String::new(),
        root_dir: String::new(),
        domain: String::new(),
        port: 0,
        require_https: false,
        use_pbs_offloading: false,
        verbose_rocket_logging: true,
    },
    is_empty: true,
};

fn check_dir_exists(path: &str) {
    let p = std::path::Path::new(path);
    if !std::path::Path::is_dir(p) {
        logging::log(logging::Priority::Critical, LOGSRC, &format!("Invalid configuration, {} does not exist or it is not a directory", path));
        panic!();
    }
}

fn check_file_exists(path: &str) {
    let p = std::path::Path::new(path);
    if !std::path::Path::is_file(p) {
        logging::log(logging::Priority::Critical, LOGSRC, &format!("Invalid configuration, {} does not exist or it is not a file", path));
        panic!();
    }
}

fn read_config(path: &str) -> String {
    let mut s = String::new();
    let fh = std::fs::File::open(path).expect("Failed to open configuration file");
    let mut reader = std::io::BufReader::new(fh);
    reader.read_to_string(&mut s).expect("Failed to read configuration file");

    s
}

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
}

struct ConfigContainer {
    config: Config,
    is_empty: bool,
}

impl ConfigContainer {
    fn is_empty(&self) -> bool {
        self.is_empty
    }

    fn load(cfg_path: &str) {
        let cfg: Config = match serde_json::from_str(read_config(cfg_path).as_str()) {
            Ok(cfg) => cfg,
            Err(e) => {
                logging::log(logging::Priority::Critical, LOGSRC, &format!("Failed to parse configuation file: {}", e.to_string().as_str()));
                panic!();
            }
        };

        check_file_exists(cfg.mmb_exec_path.as_str());
        check_file_exists(cfg.mmb_parameters_path.as_str());
        check_dir_exists(cfg.examples_dir.as_str());
        check_dir_exists(cfg.root_dir.as_str());
        if cfg.domain.len() < 1 {
            logging::log(logging::Priority::Critical, LOGSRC, "Invalid configuration - no domain name: {}");
            panic!();
        }
        if cfg.port == 0 {
            logging::log(logging::Priority::Critical, LOGSRC, "Invalid configuration - port number cannot be zero");
            panic!();
        }

        unsafe {
            CONFIG.config = cfg;
            CONFIG.is_empty = false;
        }
    }
}

pub fn get() -> &'static Config {
    unsafe {
        if CONFIG.is_empty() {
            panic!("Server configuration was accessed before it was initialized");
        }

        &CONFIG.config
    }
}

pub fn load(cfg_path: &str) {
    unsafe {
        if !CONFIG.is_empty() {
            panic!("Attemped to load server configuration after it has been already loaded");
        }
    }

    ConfigContainer::load(cfg_path)
}
