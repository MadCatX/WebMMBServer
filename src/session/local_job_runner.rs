use nix::unistd::Pid;
use nix::sys::signal::{self, Signal};
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::thread;
use std::time::Duration;

use crate::config;
use crate::mmb;
use super::job_runner;

fn check_process(proc: &mut Option<Child>) -> Result<mmb::State, String> {
    if proc.is_none() {
        return Ok(mmb::State::Unknown);
    }

    match proc.as_mut().unwrap().try_wait() {
        Ok(exit) => {
            match exit {
                Some(status) => {
                    if status.success() {
                        return Ok(mmb::State::Finished);
                    }
                    return Ok(mmb::State::Failed);
                },
                None => Ok(mmb::State::Running)
            }
        },
        Err(e) => Err(e.to_string()),
    }
}

pub struct LocalJobRunner {
    mmb_process: Option<Child>,
}

impl job_runner::JobRunner for LocalJobRunner {
    fn executor_state(&mut self) -> Result<mmb::State, String> {
        check_process(&mut self.mmb_process)
    }

    fn prune_job_dir(&self, _job_dir: PathBuf) -> Result<(), String> {
        /* Nothing specific to do for this Runner */
        Ok(())
    }

    fn start(&mut self, job_dir: PathBuf, cmds_file_path: &Path, diag_file_path: &Path, progress_file_path: &Path) -> Result<(), String> {
        match Command::new(&config::get().mmb_exec_path)
            .current_dir(&job_dir)
            .arg("-C")
            .arg(cmds_file_path)
            .arg("-progress")
            .arg(progress_file_path)
            .arg("-output")
            .arg(diag_file_path)
            .spawn() {
            Ok(child) => self.mmb_process = Some(child),
            Err(e) => return Err(e.to_string()),
        };

        Ok(())
    }

    fn stop(&mut self) -> Result<(), String> {
        if self.mmb_process.is_none() {
            return Ok(());
        }

        let pid = self.mmb_process.as_ref().unwrap().id();
        if signal::kill(Pid::from_raw(pid as i32), Signal::SIGTERM).is_err() {
            return Err(String::from("Failed to signal job process"));
        }

        let terminated = || -> bool {
            let mut attempts = 0;
            while attempts < 10 {
                match self.mmb_process.as_mut().unwrap().try_wait() {
                    Ok(ret) => match ret {
                        Some(_) => return true,
                        None => {
                            attempts += 1;
                        },
                    },
                    Err(_) => {
                        return false;
                    }
                };
                thread::sleep(Duration::from_millis(1000));
            }
            false
        }();

        if !terminated {
            if self.mmb_process.as_mut().unwrap().kill().is_err() {
                return Err(String::from("Failed to kill job process"));
            }
        }

        Ok(())
    }
}

impl LocalJobRunner {
    pub fn create() -> Result<LocalJobRunner, String> {
        Ok(LocalJobRunner{
            mmb_process: None,
        })
    }
}

impl Drop for LocalJobRunner {
    fn drop(&mut self) {
        match self.mmb_process.as_mut() {
            Some(p) => assert!(p.try_wait().is_ok()),
            None => {},
        };
    }
}
