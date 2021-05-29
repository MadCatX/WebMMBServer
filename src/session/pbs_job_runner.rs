use std::ffi::OsStr;
use std::io::Write;
use std::fs::File;
use std::process::Command;
use std::path::{Path, PathBuf};

use crate::config;
use crate::mmb;
use crate::pbs;
use super::job_runner;

fn mk_stderr_file_path(mut base_path: PathBuf) -> PathBuf {
    base_path.push("job_stderr.txt");
    base_path
}

fn mk_stdout_file_path(mut base_path: PathBuf) -> PathBuf {
    base_path.push("job_stdout.txt");
    base_path
}

impl job_runner::JobRunner for PbsJobRunner {
    fn executor_state(&mut self) -> Result<mmb::State, String> {
        if self.job_no.is_none() {
            return Ok(mmb::State::NotStarted);
        }

        match pbs::get_job_info(self.job_no.unwrap()) {
            Ok(info) => match info.state {
                pbs::JobState::Queued => Ok(mmb::State::Queued),
                pbs::JobState::Held => Ok(mmb::State::Failed),
                pbs::JobState::Running | pbs::JobState::Exiting => Ok(mmb::State::Running),
                pbs::JobState::Finished => Ok(mmb::State::Finished),
                pbs::JobState::Unknown => Ok(mmb::State::Unknown), /* Unknown state can mean that the job has already finished and been removed from the queue log */
            },
            Err(e) => Err(e),
        }
    }

    fn prune_job_dir(&self, job_dir: PathBuf) -> Result<(), String> {
        std::fs::remove_file(mk_stderr_file_path(job_dir.clone()));
        std::fs::remove_file(mk_stdout_file_path(job_dir.clone()));
        Ok(())
    }

    fn start(&mut self, job_dir: PathBuf, cmds_file_path: &Path, diag_file_path: &Path, progress_file_path: &Path) -> Result<(), String> {
        let starter_file_path = match self.write_starter_file(job_dir.clone(), cmds_file_path, diag_file_path, progress_file_path) {
            Ok(path) => path,
            Err(e) => return Err(e),
        };
        let stdout_file = mk_stdout_file_path(job_dir.clone());
        let stderr_file = mk_stderr_file_path(job_dir.clone());

        let cmdout = match Command::new("qsub")
                                   .args(&[
                                       OsStr::new("-o"),
                                       stdout_file.as_os_str(),
                                       OsStr::new("-e"),
                                       stderr_file.as_os_str(),
                                       starter_file_path.as_os_str()
                                   ])
                                   .current_dir(job_dir.as_path())
                                   .output() {
            Ok(cmdout) => cmdout,
            Err(e) => return Err(e.to_string()),
        };

        if !cmdout.status.success() {
            return Err(String::from("Failed to enqueue job"));
        }

        let stdout = match String::from_utf8(cmdout.stdout) {
            Ok(stdout) => stdout,
            Err(e) => return Err(e.to_string()),
        };

        let parts: Vec<&str> = stdout.split('.').collect();
        if parts.len() < 2 {
            return Err(String::from("Invalid PBS job name"));
        }

        self.job_no = match parts[0].parse::<u32>() {
            Ok(no) => Some(no),
            Err(e) => {
                println!("{}", String::from_utf8(cmdout.stderr).unwrap());
                return Err(e.to_string());
            },
        };

        match pbs::get_job_info(self.job_no.unwrap()) {
            Ok(info) => {
                self.exec_node = Some(info.exec_node);
            },
            Err(e) => return Err(e),
        };

        Ok(())
    }

    fn stop(&mut self) -> Result<(), String> {
        if self.job_no.is_none() {
            return Ok(());
        }

        let ret = match Command::new("qdel")
                                .args(&[self.job_no.unwrap().to_string()])
                                .status() {
            Ok(ret) => ret,
            Err(e) => return Err(e.to_string()),
        };

        match ret.success() {
            true => Ok(()),
            false => Err(String::from("Failed to remove job from queue")),
        }
    }
}

pub struct PbsJobRunner {
    job_no: Option<u32>,
    exec_node: Option<String>,
}

impl PbsJobRunner {
    pub fn create() -> Result<PbsJobRunner, String> {
        Ok(
            PbsJobRunner{
                job_no: None,
                exec_node: None,
            }
        )
    }

    fn write_starter_file(&self, job_dir: PathBuf, cmds_file_path: &Path, diag_file_path: &Path, progress_file_path: &Path) -> Result<PathBuf, String> {
        let mut starter_path = job_dir.clone();
        starter_path.push("starter.sh");

        let mut fh = match File::create(starter_path.clone()) {
            Ok(fh) => fh,
            Err(e) => return Err(e.to_string()),
        };

        let script = format!(
            "#!/bin/sh\n\
             cd \"$PBS_O_WORKDIR\" || exit 1\n\
             {} -C {} -output {} -progress {}",
            config::get().mmb_exec_path,
            cmds_file_path.display(),
            diag_file_path.display(),
            progress_file_path.display()
        );
        fh.write_all(script.as_bytes());

        Ok(starter_path)
    }
}
