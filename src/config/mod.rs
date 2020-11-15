use std::sync::Arc;
use std::io::prelude::*;
use serde::Deserialize;
use serde_json;

fn check_dir_exists(path: &str) {
    let p = std::path::Path::new(path);
    if !std::path::Path::is_dir(p) {
        panic!("Invalid configuration, {} does not exist or it is not a directory", path);
    }
}

fn check_file_exists(path: &str) {
    let p = std::path::Path::new(path);
    if !std::path::Path::is_file(p) {
        panic!("Invalid configuration, {} does not exist or it is not a file", path);
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

    pub root_dir: String,

    pub port: u16,
}

impl Config {
    fn load(cfg_path: &str) -> Config {
        let cfg: Config = match serde_json::from_str(read_config(cfg_path).as_str()) {
            Ok(cfg) => cfg,
            Err(e) => panic!("Failed to parse configuation file: {}", e.to_string()),
        };

        check_file_exists(cfg.mmb_exec_path.as_str());
        check_file_exists(cfg.mmb_parameters_path.as_str());
        check_dir_exists(cfg.root_dir.as_str());
        if cfg.port == 0 {
            panic!("Invalid port number");
        }

        cfg
    }
}

pub fn load(cfg_path: &str) -> Arc<Config> {
    Arc::from(Config::load(cfg_path))
}
