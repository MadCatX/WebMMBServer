pub mod commands;

pub const PARAMETERS_FILE: &'static str = "parameters.csv";
const TRAJECTORY_FILE: &'static str = "trajectory";

pub fn trajectory_file_name(stage: i32) -> String {
    format!("{}.{}.pdb", TRAJECTORY_FILE, stage)
}
