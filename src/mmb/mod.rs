pub mod commands;

use serde::Deserialize;

pub const PARAMETERS_FILE: &'static str = "parameters.csv";
const TRAJECTORY_FILE: &'static str = "trajectory";

#[derive(Copy, Clone, PartialEq, Deserialize)]
pub enum State {
    Unknown,
    NotStarted,
    Running,
    Failed,
    Finished,
}

#[derive(Clone, Deserialize)]
pub struct Progress {
    pub state: State,
    pub step: i32,
    pub total_steps: i32,
}

pub fn trajectory_file_name(stage: i32) -> String {
    format!("{}.{}.pdb", TRAJECTORY_FILE, stage)
}
