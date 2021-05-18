pub mod job;
pub mod session;
pub mod session_manager;

mod local_job_runner;
mod job_runner;

use std::path::PathBuf;
use uuid::Uuid;

use crate::mmb;

pub fn trajectory_file_path(base: &PathBuf, session_id: &str, job_id: &str, stage: i32) -> Result<PathBuf, ()> {
    let traj_file = mmb::trajectory_file_name(stage);
    let mut path = PathBuf::new();
    path.push(base);
    path.push(session_id);
    path.push(job_id);
    path.push(traj_file);

    Ok(path)
}

pub fn new_uuid() -> Uuid {
    Uuid::new_v4()
}

pub fn str_to_uuid(s: &str) -> Result<Uuid, uuid::Error> {
    return Uuid::parse_str(s);
}

pub fn uuid_to_str(uuid: &Uuid) -> String {
    uuid.to_hyphenated().to_string()
}
