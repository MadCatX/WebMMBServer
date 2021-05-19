use std::path::PathBuf;

use crate::mmb;

pub struct Progress {
    pub state: mmb::State,
    pub step: i32,
    pub total_steps: i32,
}

pub trait JobRunner {
    fn diagnostics(&mut self) -> Result<String, String>;
    fn job_dir(&self) -> Result<&PathBuf, String>;
    fn progress(&self) -> Result<Option<Progress>, String>;
    fn prune_job_dir(&self) -> Result<(), String>;
    fn start(&mut self) -> Result<(), String>;
    fn state(&mut self) -> Result<mmb::State, String>;
    fn stop(&mut self) -> Result<(), String>;
}
