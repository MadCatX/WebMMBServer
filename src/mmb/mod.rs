pub mod advanced_commands;
pub mod commands;
pub mod examples;

use serde::Deserialize;

pub const PARAMETERS_FILE: &'static str = "parameters.csv";
const TRAJECTORY_FILE: &'static str = "trajectory";

#[derive(Copy, Clone, Debug, PartialEq, Deserialize)]
pub enum State {
    Unknown,
    NotStarted,
    Running,
    Failed,
    Finished,
}

impl std::fmt::Display for State {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        std::fmt::Debug::fmt(self, f)
    }
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
