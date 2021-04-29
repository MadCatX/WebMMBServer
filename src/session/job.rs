use nix::unistd::Pid;
use nix::sys::signal::{self, Signal};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::thread;
use std::time::Duration;
use file_lock::FileLock;

use crate::mmb;
use crate::server::api;

const CMDS_FILE_NAME: &'static str = "commands.txt";
const PGRS_FILE_NAME: &'static str = "progress.json";
const DOUT_FILE_NAME: &'static str = "doutput.txt";
const LAST_FRAME_FILE_PREFIX: &'static str = "last";
const TRAJECTORY_FILE_PREFIX: &'static str = "trajectory";

#[derive(Clone)]
pub enum CommandsMode {
    Synthetic,
    Raw,
}

#[derive(Clone)]
pub struct JobInfo {
    pub name: String,
    pub state: mmb::State,
    pub step: i32,
    pub total_steps: i32,
    pub available_stages: Vec<i32>,
    pub current_stage: Option<i32>,
    pub created_on: u128,
    pub commands_mode: CommandsMode,
}

pub struct Job {
    pub name: String,
    commands: Option<api::JsonCommands>,
    raw_commands: Option<String>,
    job_dir: PathBuf,
    cmds_path: PathBuf,
    mmb_exec_path: PathBuf,
    progress_path: PathBuf,
    diag_output_path: PathBuf,
    current_stage: Option<i32>,
    mmb_process: Option<Child>,
    created_on: std::time::SystemTime,
}

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

fn clear_stages(path: &PathBuf, stage: i32) -> Result<(), String> {
    let dir_lister = match std::fs::read_dir(path) {
        Ok(v) => v,
        Err(e) => return Err(e.to_string()),
    };

    let traj_prefix = format!("{}.", TRAJECTORY_FILE_PREFIX);
    let last_prefix = format!("{}.", LAST_FRAME_FILE_PREFIX);
    for entry in dir_lister {
        if entry.is_err() {
            continue;
        }

        let p = entry.unwrap().path();
        if !p.is_file() {
            continue;
        }

        match p.extension() {
            Some(extn) => {
                if extn != "pdb" {
                    continue;
                }
            },
            None => continue,
        }

        let name = match p.file_stem() {
            Some(stem) => {
                match stem.to_str() {
                    Some(name) => name,
                    None => continue,
                }
            },
            None => continue,
        };

        if !(name.starts_with(traj_prefix.as_str()) || name.starts_with(last_prefix.as_str())) {
            continue;
        }

        let segs = name.split(".").collect::<Vec<&str>>();
        if segs.len() != 2 {
            continue;
        }

        let n = match segs.get(1).unwrap().parse::<i32>() {
            Ok(n) => n,
            Err(_) => continue,
        };

        if n >= stage {
            std::fs::remove_file(p);
        }
    }

    Ok(())
}

fn copy_job_dir(tgt: &PathBuf, src: &PathBuf) -> Result<(), String> {
    let dir_lister = match std::fs::read_dir(src) {
        Ok(v) => v,
        Err(e) => return Err(e.to_string()),
    };

    for entry in dir_lister {
        let e = match entry {
            Ok(v) => v,
            Err(e) => return Err(e.to_string()),
        };

        let path = e.path();

        if path.is_dir() {
            continue;
        }

        let name = match path.file_name() {
            Some(v) => v,
            None => return Err(String::from("No file name")),
        };

        if name == PGRS_FILE_NAME ||
           name == DOUT_FILE_NAME {
            continue;
        }

        let mut tgt_file = tgt.clone();
        tgt_file.push(name);

        match std::fs::copy(e.path(), tgt_file) {
            Ok(_) => (),
            Err(e) => return Err(e.to_string()),
        }
    }

    Ok(())
}

fn get_stages(path: &PathBuf, file_name: &str) -> Vec<i32> {
    let mut stages: Vec<i32> = Vec::new();
    let dir_lister = std::fs::read_dir(path);
    if dir_lister.is_err() {
        return stages;
    }

    let name_prefix = format!("{}.", file_name);
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

            if !name.unwrap().starts_with(name_prefix.as_str()) {
                continue;
            }

            let segs = name.unwrap().split(".").collect::<Vec<&str>>();
            if segs.len() != 2 {
                continue;
            }

            match segs.get(1).unwrap().parse::<i32>() {
                Ok(n) => {
                    stages.push(n);
                }
                Err(_) => (),
            }
        }
    }

    stages.sort_unstable();
    stages
}

fn process_stages(commands: &api::JsonCommands) -> Result<mmb::commands::Stages, String> {
    let stages = match mmb::commands::stages(&commands) {
        Ok(stages) => stages,
        Err(e) => return Err(e),
    };
    if stages.first != stages.last {
        return Err(String::from("Calculation spanning over multiple stages is not supported"));
    }

    Ok(stages)
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

fn remove_file(path: &Path) -> Result<(), String> {
    if path.exists() {
        match std::fs::remove_file(path) {
            Ok(_) => return Ok(()),
            Err(e) => return Err(e.to_string()),
        }
    }
    Ok(())
}

impl Job {
    fn launch(&self) -> Result<Child, std::io::Error> {
        Command::new(&self.mmb_exec_path)
            .current_dir(&self.job_dir)
            .arg("-C")
            .arg(&self.cmds_path)
            .arg("-progress")
            .arg(&self.progress_path)
            .arg("-output")
            .arg(&self.diag_output_path)
            .spawn()
    }

    fn prune_path(&self) -> Result<(), String> {
        match remove_file(&self.progress_path) {
            Ok(_) => (),
            Err(e) => return Err(e.to_string()),
        }
        match remove_file(&self.diag_output_path) {
            Ok(_) => (),
            Err(e) => return Err(e.to_string()),
        }

        Ok(())
    }

    pub fn available_stages(&self) -> Vec<i32> {
        get_stages(&self.job_dir, TRAJECTORY_FILE_PREFIX)
    }

    pub fn clone(name: String, mmb_exec_path: PathBuf, job_dir: PathBuf, src: &Job) -> Result<Job, String> {
        match copy_job_dir(&job_dir, &src.job_dir) {
            Ok(_) => (),
            Err(e) => return Err(e),
        };

        let mut cmds_path = PathBuf::new();
        cmds_path.push(&job_dir); cmds_path.push(CMDS_FILE_NAME);

        let mut progress_path = PathBuf::new();
        progress_path.push(&job_dir); progress_path.push(PGRS_FILE_NAME);

        let mut diag_output_path = PathBuf::new();
        diag_output_path.push(&job_dir); diag_output_path.push(DOUT_FILE_NAME);

        Ok(Job{
            name,
            commands: src.commands.clone(),
            raw_commands: src.raw_commands.clone(),
            job_dir,
            cmds_path,
            mmb_exec_path,
            progress_path,
            diag_output_path,
            current_stage: src.current_stage,
            mmb_process: None,
            created_on: std::time::SystemTime::now(),
        })
    }

    pub fn commands(&self) -> Option<api::JsonCommands> {
        self.commands.clone()
    }

    pub fn commands_mode(&self) -> CommandsMode {
        assert!(!(self.commands.is_some() && self.raw_commands.is_some()), "Synthetic and raw commands cannot be both specified at the same time");

        if self.commands.is_some() {
            return CommandsMode::Synthetic;
        }
        if self.raw_commands.is_some() {
            return CommandsMode::Raw;
        }

        panic!("Neither synthetic nor raw commands have been set");
    }

    pub fn commands_raw(&self) -> Option<String> {
        self.raw_commands.clone()
    }

    pub fn create(name: String, mmb_exec_path: PathBuf, job_dir: PathBuf, commands: Option<api::JsonCommands>, raw_commands: Option<String>) -> Result<Job, String> {
        assert!(!(commands.is_some() && raw_commands.is_some()), "Synthetic and raw commands cannot be both specified at the same time");

        let current_stage = if commands.is_some() {
            let stages = process_stages(commands.as_ref().unwrap())?;
            Some(stages.first)
        } else if raw_commands.is_some() {
            let parsed = mmb::commands::parse_raw(raw_commands.as_ref().unwrap())?;
            Some(parsed.first_stage)
        } else {
            None
        };

        let mut cmds_path = PathBuf::new();
        cmds_path.push(&job_dir); cmds_path.push(CMDS_FILE_NAME);

        let mut progress_path = PathBuf::new();
        progress_path.push(&job_dir); progress_path.push(PGRS_FILE_NAME);

        let mut diag_output_path = PathBuf::new();
        diag_output_path.push(&job_dir); diag_output_path.push(DOUT_FILE_NAME);

        Ok(Job{
            name,
            commands,
            raw_commands,
            job_dir,
            cmds_path,
            mmb_exec_path,
            progress_path,
            diag_output_path,
            current_stage,
            mmb_process: None,
            created_on: std::time::SystemTime::now(),
        })
    }

    pub fn info(&mut self) -> Result<JobInfo, String> {
        let state = self.state()?;

        match state {
            mmb::State::NotStarted =>
                return Ok(JobInfo{
                    name: self.name.clone(),
                    state: mmb::State::NotStarted,
                    step: 0,
                    total_steps: 0,
                    available_stages: self.available_stages(),
                    current_stage: self.current_stage,
                    created_on: match self.created_on.duration_since(std::time::UNIX_EPOCH) {
                        Ok(d) => d.as_millis(),
                        Err(_) => 0
                    },
                    commands_mode: self.commands_mode(),
                }),
            mmb::State::Unknown => return Err(String::from("Unknown job state")),
            _ => (),
        };

        match read_mmb_progress(&self.progress_path) {
            Ok((state, step, total_steps)) => {
                let mut info = JobInfo{
                    name: self.name.clone(),
                    state,
                    step,
                    total_steps,
                    available_stages: self.available_stages(),
                    current_stage: self.current_stage,
                    created_on: match self.created_on.duration_since(std::time::UNIX_EPOCH) {
                        Ok(d) => d.as_millis(),
                        Err(_) => 0
                    },
                    commands_mode: self.commands_mode(),
                };
                if state == mmb::State::Running {
                    // MMB reports the job has finished but the MMB process is still running
                    // Wait until the MMB process actually terminates
                    info.state = mmb::State::Running;
                } else if info.state == mmb::State::Running &&
                          state != mmb::State::Running {
                    // MMB reports that the job is running but its process has died
                    // Report this as an error
                    info.state = mmb::State::Failed;
                }
                Ok(info)
            },
            Err(e) => { // Error here may indicate that MMB has not created progress file yet. That is okay
                Ok(JobInfo{
                    name: self.name.clone(),
                    state,
                    step: 0,
                    total_steps: 0,
                    available_stages: self.available_stages(),
                    current_stage: self.current_stage,
                    created_on: 0,
                    commands_mode: self.commands_mode(),
                })
            },
        }
    }

    pub fn last_available_stage(&self) -> Option<i32> {
        match get_stages(&self.job_dir, TRAJECTORY_FILE_PREFIX).last() {
            Some(v) => Some(*v),
            None => None
        }
    }

    pub fn start(&mut self, commands: api::JsonCommands) -> Result<(), String> {
        if self.raw_commands.is_some() {
            return Err(String::from("Job created in raw commands mode cannot be run in synthetic commands mode"));
        }

        match self.prune_path() {
            Ok(_) => (),
            Err(e) => return Err(e),
        }

        self.commands = Some(commands);

        let stages = process_stages(self.commands.as_ref().unwrap())?;

        match mmb::commands::write(&self.cmds_path, self.commands.as_ref().unwrap(), stages.first) {
            Ok(_) => (),
            Err(e) => return Err(e.to_string()),
        };

        match clear_stages(&self.job_dir, stages.first) {
            Ok(()) => (),
            Err(e) => return Err(e),
        };

        let proc = match self.launch() {
            Ok(proc) => proc,
            Err(e) => return Err(format!("Failed to launch MMB process: {}", e)),
        };
        self.mmb_process = Some(proc);

        Ok(())
    }

    pub fn start_raw(&mut self, raw_commands: String) -> Result<(), String> {
        if self.commands.is_some() {
            return Err(String::from("Job created in synthetic commands mode cannot be run in raw commands mode"));
        }

        match self.prune_path() {
            Ok(_) => (),
            Err(e) => return Err(e),
        }


        let parsed = match mmb::commands::parse_raw(&raw_commands) {
            Ok(v) => v,
            Err(e) => return Err(e),
        };

        match mmb::commands::write_raw(&self.cmds_path, &raw_commands) {
            Ok(_) => (),
            Err(e) => return Err(e.to_string()),
        }

        match clear_stages(&self.job_dir, parsed.first_stage) {
            Ok(_) => (),
            Err(e) => return Err(e),
        }

        self.raw_commands = Some(raw_commands);

        let proc = match self.launch() {
            Ok(proc) => proc,
            Err(e) => return Err(format!("Failed to launch MMB process: {}", e)),
        };
        self.mmb_process = Some(proc);

        Ok(())
    }

    pub fn state(&mut self) -> Result<mmb::State, String> {
        let proc_state = check_process(&mut self.mmb_process)?;

        if proc_state == mmb::State::Unknown && !self.progress_path.exists() {
            return Ok(mmb::State::NotStarted);
        }
        Ok(proc_state)
    }

    pub fn stdout(&mut self) -> Result<String, String> {
        let state = self.state()?;
        if state == mmb::State::NotStarted {
            return Ok(String::new());
        }

        match std::fs::File::open(&self.diag_output_path) {
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
                    Err(e) => {
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

        match self.info() {
            Ok(info) => return Ok(info),
            Err(e) => return Err(e),
        }
    }
}

impl Drop for Job {
    fn drop(&mut self) {
        match self.mmb_process.as_mut() {
            Some(p) => assert!(p.try_wait().is_ok()),
            None => {},
        };

        std::fs::remove_dir_all(&self.job_dir);
        println!("Job dropped");
    }
}
