pub mod job;
pub mod session;
pub mod session_manager;

use std::path::PathBuf;
use uuid::Uuid;

use crate::mmb;

pub fn trajectory_file_path(base: &PathBuf, username: &str, id_str: &str, stage: i32) -> Result<PathBuf, ()> {
    if uuid::Uuid::parse_str(id_str).is_err() {
        return Err(());
    };

    let traj_file = mmb::trajectory_file_name(stage);
    let mut path = PathBuf::new();
    path.push(base); 
    path.push(username);
    path.push(id_str);
    path.push(traj_file);

    Ok(path)
}

pub fn uuid_to_str(uuid: &Uuid) -> String {
    uuid.to_hyphenated().to_string()
}
