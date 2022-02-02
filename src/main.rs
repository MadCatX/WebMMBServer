#![feature(proc_macro_hygiene, decl_macro)]
#[macro_use] extern crate rocket;
extern crate uuid;

use std::path::{Path, PathBuf};
use clap::{Arg, ArgMatches, App};

mod config;
mod logging;
mod mmb;
mod pbs;
mod server;
mod session;

const LOGSRC: &'static str = "main";

fn arguments() -> ArgMatches<'static> {
    App::new("WebMMB server")
        .arg(Arg::with_name("config_file")
            .long("config_file")
            .value_name("CONFIG_FILE")
            .help("Path to configuration file")
            .takes_value(true))
        .get_matches()
}

fn check_and_prepare() {
    let cfg = config::get();

    let mut is_ok = true;
    if !Path::new(&cfg.mmb_exec_path).is_file() {
        log_plain!(Critical, LOGSRC, "Configuration contains invalid path to MMB executable");
        is_ok = false;
    }
    if !Path::new(&cfg.mmb_parameters_path).is_file() {
        log_plain!(Critical, LOGSRC, "Configuration contains invalid path to MMB parameters file");
        is_ok = false;
    }
    if !Path::new(&cfg.examples_dir).is_dir() {
        log_plain!(Critical, LOGSRC, "Configuration contains invalid path to examples directory");
        is_ok = false;
    }
    if !Path::new(&cfg.root_dir).is_dir() {
        log_plain!(Critical, LOGSRC, "Configuration contains invalid path to server root directory");
        is_ok = false;
    }

    if !is_ok {
        panic!();
    }

    let p = Path::new(cfg.jobs_dir.as_str());
    if !Path::is_dir(p) {
        let mut db = std::fs::DirBuilder::new();
        db.recursive(true);
        match db.create(p) {
            Ok(()) => (),
            Err(e) => {
                log_plain!(Critical, LOGSRC, &format!("Failed to create working directory: {}", e.to_string()));
                panic!();
            },
        }
    }
}

fn init_config(path: &str) {
    config::load(PathBuf::from(path));
}

fn init_logging() {
    let cfg = config::get();

    let log_file_path = match &cfg.log_file {
        Some(path) => Some(PathBuf::from(&path)),
        None => None,
    };

    logging::init(log_file_path);
}

#[rocket::launch]
fn liftoff() -> _ {
    log_early!(Info, LOGSRC, "WebMMB server starting up");

    let args = arguments();

    init_config(args.value_of("config_file").unwrap_or("/etc/webmmb_server/config.json"));
    init_logging();

    check_and_prepare();

    server::start()
}
