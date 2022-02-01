use std::collections::BTreeMap;
use std::fmt;
use journald::*;

pub const INV_FILE_NAME: &'static str = "<INVALID_FILE_NAME>";
pub const INV_FILE_PATH: &'static str = "<INVALID_FILE_PATH>";

const PRIORITY: &'static str = "PRIORITY";
const SOURCE: &'static str = "SOURCE";

#[derive(Clone, Copy, PartialEq)]
pub enum Priority {
    Debug = 7,
    Info = 6,
    Warning = 4,
    Error = 3,
    Critical = 2,
}

impl fmt::Display for Priority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", *self as i32)
    }
}

fn make_log_entry(pri: Priority, source: &str, message: &str) -> JournalEntry {
    let mut fields = BTreeMap::new();

    fields.insert(String::from(PRIORITY), pri.to_string());
    fields.insert(String::from(SOURCE), String::from(source));

    let mut entry = JournalEntry::from(&fields);
    entry.set_message(message);

    entry
}

pub fn log(pri: Priority, source: &str, message: &str) {
    let entry = make_log_entry(pri, source, message);
    journald::writer::submit(&entry);
}

pub fn log_startup_message() {
    let entry = make_log_entry(Priority::Info, "core", "WebMMB server is starting up...");

    match journald::writer::submit(&entry) {
        Ok(()) => (),
        Err(e) => panic!("Cannot log startup message. Refusing to continue with no logging available.\nError reported: {}", e.to_string())
    }
}
