use file_lock::FileLock;
use nix::unistd::Pid;
use nix::sys::signal::{self, Signal};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::thread;
use std::time::Duration;

use crate::config;
use crate::mmb;
use super::job_runner::{JobRunner, Progress};

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

fn read_mmb_progress(path: &PathBuf) -> Result<Option<Progress>, String> {
    let path_str = path.to_str();
    if path_str.is_none() {
        return Err(String::from("Invalid progress file path"));
    }

    /* If the progress file does not exist, it could mean that MMB just has not created it yet */
    if !path.is_file() {
        return Ok(None);
    }

    let locked = FileLock::lock(path_str.unwrap(), false, false);
    if locked.is_err() {
        /* Error here may indicate that the progress file is locked by MMB */
        return Ok(None);
    }

    let mut s = String::new();
    if locked.unwrap().file.read_to_string(&mut s).is_err() {
        return Err(String::from("Cannot read progress report file"));
    }

    let json: serde_json::Result<mmb::Progress> = serde_json::from_str(s.as_str());
    match json {
        Ok(progress) => Ok(Some(Progress{
            state: progress.state,
            step: progress.step,
            total_steps: progress.total_steps,
        })),
        Err(e) => Err(e.to_string()),
    }
}

fn remove_file(path: &Path) -> Result<(), String> {
    if path.exists() {
        match std::fs::remove_file(path) {
            Ok(_) => return Ok(()),
            Err(e) => return Err(e.to_string()),
        }
    }
    Ok(())
}

pub struct LocalJobRunner {
    job_dir: PathBuf,
    cmds_file_path: PathBuf,
    progress_file_path: PathBuf,
    diag_output_file_path: PathBuf,
    mmb_process: Option<Child>,
}

impl JobRunner for LocalJobRunner {
    fn diagnostics(&mut self) -> Result<String, String> {
        let state = self.state()?;
        if state == mmb::State::NotStarted {
            return Ok(String::new());
        }

        match std::fs::File::open(&self.diag_output_file_path) {
            Ok(mut fh) => {
                let mut buf = String::new();
                match fh.read_to_string(&mut buf) {
                    Ok(_) => Ok(buf),
                    Err(e) => Err(e.to_string()),
                }
            },
            Err(e) => Err(e.to_string()),
        }
    }

    fn job_dir(&self) -> Result<&PathBuf, String> {
        Ok(&self.job_dir)
    }

    fn progress(&self) -> Result<Option<Progress>, String> {
        read_mmb_progress(&self.progress_file_path)
    }

    fn prune_job_dir(&self) -> Result<(), String> {
        match remove_file(&self.progress_file_path) {
            Ok(_) => (),
            Err(e) => return Err(e.to_string()),
        }
        match remove_file(&self.diag_output_file_path) {
            Ok(_) => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }

    fn start(&mut self) -> Result<(), String> {
        match Command::new(&config::get().mmb_exec_path)
            .current_dir(&self.job_dir)
            .arg("-C")
            .arg(&self.cmds_file_path)
            .arg("-progress")
            .arg(&self.progress_file_path)
            .arg("-output")
            .arg(&self.diag_output_file_path)
            .spawn() {
            Ok(child) => self.mmb_process = Some(child),
            Err(e) => return Err(e.to_string()),
        };

        Ok(())
    }

    fn state(&mut self) -> Result<mmb::State, String> {
        let proc_state = check_process(&mut self.mmb_process)?;

        if proc_state == mmb::State::Unknown && !self.progress_file_path.exists() {
            return Ok(mmb::State::NotStarted);
        }
        Ok(proc_state)
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
    pub fn create(job_dir: PathBuf, cmds_file_path: PathBuf) -> Result<LocalJobRunner, String> {
        let mut progress_file_path = PathBuf::new();
        progress_file_path.push(&job_dir); progress_file_path.push(mmb::PGRS_FILE_NAME);

        let mut diag_output_file_path = PathBuf::new();
        diag_output_file_path.push(&job_dir); diag_output_file_path.push(mmb::DOUT_FILE_NAME);

        Ok(LocalJobRunner{
            job_dir,
            cmds_file_path,
            progress_file_path,
            diag_output_file_path,
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
