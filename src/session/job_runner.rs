use crate::mmb;
use std::path::{Path, PathBuf};

pub trait JobRunner {
    fn executor_state(&mut self) -> Result<mmb::State, String>;
    fn prune_job_dir(&self, job_dir: PathBuf) -> Result<(), String>;
    fn start(&mut self, job_dir: PathBuf, cmds_file_path: &Path, diag_file_path: &Path, progress_file_path: &Path) -> Result<(), String>;
    fn stop(&mut self) -> Result<(), String>;
}
