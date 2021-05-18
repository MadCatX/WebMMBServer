pub mod commands;
pub mod examples;
pub mod additional_files;
mod advanced_params;

use serde_derive::Deserialize;

pub const CMDS_FILE_NAME: &'static str = "commands.txt";
pub const DOUT_FILE_NAME: &'static str = "doutput.txt";
pub const LAST_FRAME_FILE_PREFIX: &'static str = "last";
pub const PARAMS_FILE_NAME: &'static str = "parameters.csv";
pub const PGRS_FILE_NAME: &'static str = "progress.json";
pub const TRAJECTORY_FILE_PREFIX: &'static str = "trajectory";

#[derive(Copy, Clone, Debug, PartialEq, Deserialize)]
pub enum State {
    Unknown,
    NotStarted,
    Queued,
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
    format!("{}.{}.pdb", TRAJECTORY_FILE_PREFIX, stage)
}
