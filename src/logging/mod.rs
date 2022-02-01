use std::collections::BTreeMap;
use std::fmt;
use std::net::IpAddr;
use journald::*;
use time;

pub const INV_FILE_NAME: &'static str = "<INVALID_FILE_NAME>";
pub const INV_FILE_PATH: &'static str = "<INVALID_FILE_PATH>";

const PRIORITY: &'static str = "PRIORITY";
const SOURCE: &'static str = "SOURCE";

const DELIM: &'static str = ";";

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

fn make_log_entry(pri: Priority, source: &str, message: &str) -> JournalEntry {
    let mut fields = BTreeMap::new();

    fields.insert(String::from(PRIORITY), pri.to_string());
    fields.insert(String::from(SOURCE), String::from(source));

    let mut entry = JournalEntry::from(&fields);
    entry.set_message(message);

    entry
}

fn log_file(pri: Priority, source: &str, message: &str) {
    let now = time::PrimitiveDateTime::now();
    let time_str = format!(
        "{}-{:02}-{:02} {:02}:{:02}:{:02}.{:03}",
        now.year(), now.month(), now.day(),
        now.hour(), now.minute(), now.second(), now.millisecond()
    );

    let tokens = vec![priority_to_text(pri), time_str, source.to_string(), message.to_string()];

    println!("{}", tokens.into_iter().fold(String::new(), |a, b| a + &b + DELIM));
}

fn log_journald(pri: Priority, source: &str, message: &str) {
    let entry = make_log_entry(pri, source, message);
    journald::writer::submit(&entry);
}

pub fn incoming(pri: Priority, source: &str, remote: Option<IpAddr>, message: &str) {
    let actual_msg = format!("({}){}{}", addr_to_text(&remote), DELIM, message);
    log(pri, source, &actual_msg);
}

pub fn log(pri: Priority, source: &str, message: &str) {
    log_journald(pri, source, message);
    log_file(pri, source, message);
}

pub fn log_startup_message() {
    let entry = make_log_entry(Priority::Info, "core", "WebMMB server is starting up...");

    match journald::writer::submit(&entry) {
        Ok(()) => (),
        Err(e) => panic!("Cannot log startup message. Refusing to continue with no logging available.\nError reported: {}", e.to_string())
    }
}
