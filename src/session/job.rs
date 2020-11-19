use nix::unistd::Pid;
use nix::sys::signal::{self, Signal};
use std::io::Read;
use std::path::PathBuf;
use std::process::{Child, Command};
use std::thread;
use std::time::Duration;
use file_lock::FileLock;
use serde_json;

use crate::mmb;

const CMDS_FILE_NAME: &'static str = "commands.txt";
const PGRS_FILE_NAME: &'static str = "progress.json";

#[derive(Clone)]
pub struct JobInfo {
    pub name: String,
    pub state: mmb::State,
    pub step: i32,
    pub total_steps: i32, 
    pub last_completed_stage: i32,
}

pub struct Job {
    pub name: String,
    commands: serde_json::Value,
    job_dir: PathBuf,
    cmds_path: PathBuf,
    mmb_exec_path: PathBuf,
    progress_path: PathBuf,
    mmb_process: Option<Child>,
}

fn get_last_completed_stage(path: &PathBuf) -> i32 {
    let dir_lister = std::fs::read_dir(path);
    if dir_lister.is_err() {
        return 0;
    }

    let mut last_stage = 0;
    for entry in dir_lister.unwrap() {
        if entry.is_err() {
            continue;
        }

        let p = entry.unwrap().path();
        if p.is_file() {
            let extn = p.extension();
            if extn.is_none() {
                continue;
            }

            if p.extension().unwrap() != "pdb" {
                continue;
            }

            let stem = p.file_stem();
            if stem.is_none() {
                continue;
            }

            let name = stem.unwrap().to_str();
            if name.is_none() {
                continue;
            }

            if !name.unwrap().starts_with("last.") {
                continue;
            }

            let segs = name.unwrap().split(".").collect::<Vec<&str>>();
            if segs.len() != 2 {
                continue;
            }

            match segs.get(1).unwrap().parse::<i32>() {
                Ok(n) => {
                    if n > last_stage {
                        last_stage = n;
                    }
                }
                Err(_) => {},
            }
        }
    }

    last_stage
}

fn check_process(proc: &mut Option<Child>) -> Result<mmb::State, String> {
    if proc.is_none() {
        return Ok(mmb::State::Unknown);
    }

    match proc.as_mut().unwrap().try_wait() {
        Ok(code) => {
            match code {
                Some(status) => {
                    if !status.success() {
                        return Ok(mmb::State::Failed);
                    }
                    Ok(mmb::State::Finished)
                },
                None => Ok(mmb::State::Running)
            }
        },
        Err(e) => Err(e.to_string()),
    }
}

fn read_mmb_progress(path: &PathBuf) -> Result<(mmb::State, i32, i32), String> {
    let path_str = path.to_str();
    if path_str.is_none() {
        return Err(String::from("Invalid progress file path"));
    }

    let locked = FileLock::lock(path_str.unwrap(), false, false);
    if locked.is_err() {
        return Err(String::from("Progress file is missing or inaccessible"));
    }

    let mut s = String::new();
    if locked.unwrap().file.read_to_string(&mut s).is_err() {
        return Err(String::from("Cannot read progress report file"));
    }

    let json: serde_json::Result<mmb::Progress> = serde_json::from_str(s.as_str());
    match json {
        Ok(pgrs) => Ok((pgrs.state, pgrs.step, pgrs.total_steps)),
        Err(e) => Err(e.to_string()),
    }
}

impl Job {
    pub fn commands(&self) -> serde_json::Value {
        self.commands.clone()
    }

    pub fn create(name: String, commands: serde_json::Value, mmb_exec_path: PathBuf, job_dir: PathBuf) -> Result<Job, String> {
        let mut cmds_path = PathBuf::new();
        cmds_path.push(&job_dir); cmds_path.push(CMDS_FILE_NAME);

        let mut progress_path = PathBuf::new();
        progress_path.push(&job_dir); progress_path.push(PGRS_FILE_NAME);

        match mmb::commands::write_commands(&cmds_path, &commands) {
            Ok(()) => Ok(Job{
                name,
                commands,
                job_dir,
                cmds_path,
                mmb_exec_path,
                progress_path,
                mmb_process: None,
            }),
            Err(e) => Err(e.to_string())
        }
    }

    pub fn info(&mut self) -> Result<JobInfo, String> {
        let proc_state = check_process(&mut self.mmb_process)?;

        if proc_state == mmb::State::Unknown {
            return Err(String::from("Unknown job state"));
        }

        match read_mmb_progress(&self.progress_path) {
            Ok((state, step, total_steps)) => {
                let mut info = JobInfo{
                    name: self.name.clone(),
                    state,
                    step,
                    total_steps,
                    last_completed_stage: get_last_completed_stage(&self.job_dir),
                };
                if proc_state == mmb::State::Running {
                    // MMB reports the job has finished but the MMB process is still running
                    // Wait until the MMB process actually terminates
                    info.state = mmb::State::Running;
                } else if info.state == mmb::State::Running &&
                          proc_state != mmb::State::Running {
                    // MMB reports that the job is running but its process has dies
                    // Report this as an error
                    info.state = mmb::State::Failed;
                }
                Ok(info)
            },
            Err(e) => {
                Ok(JobInfo{
                    name: self.name.clone(),
                    state: proc_state,
                    step: 0,
                    total_steps: 0,
                    last_completed_stage: get_last_completed_stage(&self.job_dir),
                })
            },
        }
    }

    pub fn last_completed_stage(&self) -> i32 {
        get_last_completed_stage(&self.job_dir)
    }

    pub fn resume(&mut self, commands: serde_json::Value) -> Result<Option<JobInfo>, String> {
        match mmb::commands::write_commands(&self.cmds_path, &commands) {
            Ok(_) => {
                self.commands = commands;
                match self.start() {
                    Ok(_) => match self.info() {
                        Ok(info) => Ok(Some(info)),
                        Err(_) => Ok(None),
                    },
                    Err(e) => Err(e),
                }
            }
            Err(e) => Err(e.to_string()),
        }
    }

    pub fn start(&mut self) -> Result<(), String> {
        // TODO: This is now tailored to the (unneccessary) runner script

        let proc = match Command::new(&self.mmb_exec_path)
            .current_dir(&self.job_dir)
            .arg(&self.cmds_path)
            .arg(&self.progress_path)
            .spawn() {
                Ok(proc) => proc,
                Err(_) => return Err(String::from("Failed to start MMB process"))
            };

        self.mmb_process = Some(proc);

        Ok(())
    }

    pub fn stop(&mut self) -> Result<JobInfo, String> {
        if self.mmb_process.is_none() {
            match self.info() {
                Ok(info) => return Ok(info),
                Err(e) => return Err(e),
            }
        }

        let pid = self.mmb_process.as_ref().unwrap().id();
        if signal::kill(Pid::from_raw(pid as i32), Signal::SIGTERM).is_err() {
            return Err(String::from("Failed to signal job process"));
        }
        
        let mut attempts = 0;
        while attempts < 10 {
            match self.mmb_process.as_mut().unwrap().try_wait() {
                Ok(_) => break,
                Err(_) => {},
            }
            attempts += 1;
            thread::sleep(Duration::from_micros(100));
        }

        if self.mmb_process.as_mut().unwrap().kill().is_err() {
            return Err(String::from("Failed to kill job process"));
        }

        match self.info() {
            Ok(info) => return Ok(info),
            Err(e) => return Err(e),
        }
    }
}

impl Drop for Job {
    fn drop(&mut self) {
        assert!(self.info().unwrap().state != mmb::State::Running);

        std::fs::remove_dir_all(&self.job_dir);
        println!("Job dropped");
    }
}
