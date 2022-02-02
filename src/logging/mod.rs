use std::collections::BTreeMap;
use std::fmt;
use std::fs::File;
use std::io::Write;
use std::net::IpAddr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use journald::*;
use lazy_static::lazy_static;
use time;

use crate::config;

pub const INV_FILE_NAME: &'static str = "<INVALID_FILE_NAME>";
pub const INV_FILE_PATH: &'static str = "<INVALID_FILE_PATH>";
pub const DELIM: &'static str = ";";

const LOGSRC: &'static str = "logger";

const PRIORITY: &'static str = "PRIORITY";
const SOURCE: &'static str = "SOURCE";

struct Logger {
    pub log_file: Option<File>,
}
lazy_static! {
    static ref LOGGER: Mutex<Logger> = Mutex::new(Logger{ log_file: None });
}

#[derive(Clone, Copy, PartialEq, PartialOrd)]
pub enum Priority {
    Debug = 7,
    Info = 6,
    Warning = 4,
    Error = 3,
    Critical = 2,
}
impl From<crate::config::LogLevel> for Priority {
    fn from(pri: crate::config::LogLevel) -> Self {
        match pri {
            crate::config::LogLevel::Debug => Priority::Debug,
            crate::config::LogLevel::Info => Priority::Info,
            crate::config::LogLevel::Warning => Priority::Warning,
        }
    }
}

macro_rules! priority_text {
    ($var:ident, $($item:ident),*) => {
        match $var {
            $(Priority::$item => stringify!($item),)*
        }
    };
}

impl fmt::Display for Priority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", *self as i32)
    }
}

fn addr_to_text(addr: &Option<IpAddr>) -> String {
    match addr {
        Some(addr) => addr.to_string(),
        None => String::from("NO_ADDRESS"),
    }
}

fn priority_to_text(pri: Priority) -> String {
    priority_text!(pri, Debug, Info, Warning, Error, Critical).to_uppercase()
}

fn make_journald_entry(pri: Priority, source: &str, message: &str) -> JournalEntry {
    let mut fields = BTreeMap::new();

    fields.insert(String::from(PRIORITY), pri.to_string());
    fields.insert(String::from(SOURCE), String::from(source));

    let mut entry = JournalEntry::from(&fields);
    entry.set_message(message);

    entry
}

fn make_text_entry(pri: Priority, source: &str, message: &str) -> String {
    let now = time::OffsetDateTime::now_utc();
    let time_str = format!(
        "{}-{:02}-{:02} {:02}:{:02}:{:02}.{:03} ({} UTC)",
        now.year(), now.month(), now.day(),
        now.hour(), now.minute(), now.second(), now.millisecond(),
        now.offset()
    );

    vec![priority_to_text(pri), time_str, source.to_string(), message.to_string()].join(DELIM)
}

fn write_to_file(mut entry: String, fh: &mut File) {
    entry.push('\n');
    if let Err(e) = fh.write_all(entry.as_bytes()) {
        write_to_journald(Priority::Debug, LOGSRC, &format!("Failed to write to logfile: {}", e.to_string()));
    }
}

fn write_to_journald(pri: Priority, source: &str, message: &str) {
    let entry = make_journald_entry(pri, source, message);
    journald::writer::submit(&entry);
}

fn write_to_stdout(entry: &str) {
    println!("{}", entry);
}

pub fn init(log_file_path: Option<PathBuf>) {
    let mut logger = Logger{ log_file: None };

    match log_file_path {
        Some(path) => match File::create(&path) {
            Ok(fh) => {
                _early(Priority::Debug, LOGSRC, &format!("Opened log file {}", path.to_str().unwrap_or(INV_FILE_PATH)));
                logger.log_file = Some(fh);
            },
            Err(e) => {
                _early(Priority::Error, LOGSRC, &format!("Failed to open log file {}: {}", path.to_str().unwrap_or(INV_FILE_PATH), e.to_string()));
                logger.log_file = None;
            }
        },
        None => logger.log_file = None,
    };

    *LOGGER.lock().unwrap() = logger;
}

pub fn _early(pri: Priority, source: &str, message: &str) {
    write_to_journald(pri, source, message);
    write_to_stdout(&make_text_entry(pri, source, message));
}

pub fn _incoming(pri: Priority, source: &str, remote_addr: Option<IpAddr>, message: &str, cfg: Arc<config::Config>) {
    let actual_msg = format!("{}{}{}", addr_to_text(&remote_addr), DELIM, message);
    _plain(pri, source, &actual_msg, cfg);
}

pub fn _plain(pri: Priority, source: &str, message: &str, cfg: Arc<config::Config>) {
    write_to_journald(pri, source, message);

    let text = make_text_entry(pri, source, message);
    if cfg.log_to_stdout.get() {
        write_to_stdout(&text);
    }
    if let Some(fh) = LOGGER.lock().as_mut().unwrap().log_file.as_mut() {
        write_to_file(text.clone(), fh);
    }
}

#[macro_export]
macro_rules! log_early {
    ($pri:ident, $source:ident, $($segment:expr),*) => {
        {
            let msg = vec![$(String::from($segment),)*].join(logging::DELIM);
            logging::_early(logging::Priority::$pri, $source, &msg);
        }
    };
}

#[macro_export]
macro_rules! log_incoming {
    ($pri:ident, $source:ident, $remote_addr:expr, $($segment:expr),*) => {
        {
            let cfg = crate::config::get();
            if logging::Priority::from(cfg.log_level) >= logging::Priority::$pri {
                let msg = vec![$(String::from($segment),)*].join(logging::DELIM);
                logging::_incoming(logging::Priority::$pri, $source, $remote_addr, &msg, cfg);
            }
        }
    };
}

#[macro_export]
macro_rules! log_plain {
    ($pri:ident, $source:ident, $($segment:expr),*) => {
        {
            let cfg = crate::config::get();
            if logging::Priority::from(cfg.log_level) >= logging::Priority::$pri {
                let msg = vec![$(String::from($segment),)*].join(logging::DELIM);
                logging::_plain(logging::Priority::$pri, $source, &msg, cfg);
            }
        }
    };
}
