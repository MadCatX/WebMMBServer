use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::io::Write;
use std::net::IpAddr;
use std::path::PathBuf;
use std::sync::Mutex;
use journald::*;
use lazy_static::lazy_static;
use time;

pub const INV_FILE_NAME: &'static str = "<INVALID_FILE_NAME>";
pub const INV_FILE_PATH: &'static str = "<INVALID_FILE_PATH>";
pub const DELIM: &'static str = ";";

const LOGSRC: &'static str = "logger";

const PRIORITY: &'static str = "PRIORITY";
const SOURCE: &'static str = "SOURCE";

struct LoggerConfig {
    pub log_file: Option<fs::File>,
}
lazy_static! {
    static ref CONFIG: Mutex<Option<LoggerConfig>> = Mutex::new(None);
}

#[derive(Clone, Copy, PartialEq)]
pub enum Priority {
    Debug = 7,
    Info = 6,
    Warning = 4,
    Error = 3,
    Critical = 2,
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
    let text = priority_text!(pri, Debug, Info, Warning, Error, Critical);
    text.to_uppercase()
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

fn write_to_file(mut entry: String) {
    if let Some(cfg) = CONFIG.lock().unwrap().as_mut() {
        if let Some(fh) = cfg.log_file.as_mut() {
            entry.push('\n');
            if let Err(e) = fh.write_all(entry.as_bytes()) {
                write_to_journald(Priority::Debug, LOGSRC, &format!("Failed to write to logfile: {}", e.to_string()));
            }
        }
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
    let mut cfg = CONFIG.lock().unwrap();
    if cfg.is_some() {
        early(Priority::Critical, LOGSRC, "Attempted to initialize already initialized logger");
        panic!();
    }

    let log_file = match log_file_path {
        Some(path) => match fs::File::create(&path) {
            Ok(fh) => {
                early(Priority::Debug, LOGSRC, &format!("Opened log file {}", path.to_str().unwrap_or(INV_FILE_PATH)));
                Some(fh)
            },
            Err(e) => {
                early(Priority::Error, LOGSRC, &format!("Failed to open log file {}: {}", path.to_str().unwrap_or(INV_FILE_PATH), e.to_string()));
                None
            }
        },
        None => None,
    };

    cfg.replace(LoggerConfig{ log_file });
}

pub fn early(pri: Priority, source: &str, message: &str) {
    write_to_journald(pri, source, message);
    write_to_stdout(&make_text_entry(pri, source, message));
}

pub fn incoming(pri: Priority, source: &str, remote_addr: Option<IpAddr>, message: &str) {
    let actual_msg = format!("{}{}{}", addr_to_text(&remote_addr), DELIM, message);
    plain(pri, source, &actual_msg);
}

pub fn plain(pri: Priority, source: &str, message: &str) {
    write_to_journald(pri, source, message);
    write_to_file(make_text_entry(pri, source, message));
}

#[macro_export]
macro_rules! log_early {
    ($pri:ident, $source:ident, $($segment:expr),*) => {
        {
            let msg = vec![$(String::from($segment),)*].join(logging::DELIM);
            logging::early(logging::Priority::$pri, $source, &msg);
        }
    };
}

#[macro_export]
macro_rules! log_incoming {
    ($pri:ident, $source:ident, $remote_addr:expr, $($segment:expr),*) => {
        {
            let msg = vec![$(String::from($segment),)*].join(logging::DELIM);
            logging::incoming(logging::Priority::$pri, $source, $remote_addr, &msg);
        }
    };
}

#[macro_export]
macro_rules! log_plain {
    ($pri:ident, $source:ident, $($segment:expr),*) => {
        {
            let msg = vec![$(String::from($segment),)*].join(logging::DELIM);
            logging::plain(logging::Priority::$pri, $source, &msg);
        }
    };
}
