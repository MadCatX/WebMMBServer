use nix::unistd::Pid;
use nix::sys::signal::{self, Signal};
use std::path::PathBuf;
use std::fmt;
use std::process::{Child, Command};
use std::sync::{Arc, RwLock};
use std::thread;
use std::thread::JoinHandle;
use std::time::Duration;
use serde_json;

use crate::mmb;

const CMDS_FILE_NAME: &'static str = "commands.txt";

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum State {
    NotStarted,
    Running,
    Failed,
    Finished
}

impl fmt::Display for State {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[derive(Clone)]
pub struct JobInfo {
    pub name: String,
    pub state: State,
    pub step: i32,
    pub total_steps: i32, 
    pub last_completed_stage: i32,
}

struct JobData {
    info: JobInfo, 
    mmb_process: Option<Child>,
}

pub struct Job {
    data: Arc<RwLock<JobData>>,
    commands: serde_json::Value,
    job_dir: PathBuf,
    cmds_path: PathBuf,
    mmb_exec_path: PathBuf,
    watcher: Option<JoinHandle<()>>,
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

impl Job {
    pub fn commands(&self) -> serde_json::Value {
        self.commands.clone()
    }

    pub fn create(name: String, commands: serde_json::Value, mmb_exec_path: PathBuf, job_dir: PathBuf) -> Result<Job, String> {
        let mut cmds_path = PathBuf::new();
        cmds_path.push(&job_dir); cmds_path.push(CMDS_FILE_NAME);

        match mmb::commands::write_commands(&cmds_path, &commands) {
            Ok(total_steps) => Ok(Job{
                data: Arc::new(
                    RwLock::new(JobData{
                        info: JobInfo{
                            name,
                            state: State::NotStarted,
                            step: 0, // FIXME
                            total_steps, // FIXME
                            last_completed_stage: 0,
                        },
                        mmb_process: None,
                    })
                ),
                commands,
                job_dir,
                cmds_path,
                mmb_exec_path,
                watcher: None,
            }),
            Err(e) => Err(e.to_string())
        }
    }

    pub fn info(&self) -> JobInfo {
        self.data.read().unwrap().info.clone()
    }

    pub fn resume(&mut self, commands: serde_json::Value) -> Result<JobInfo, String> {
        match mmb::commands::write_commands(&self.cmds_path, &commands) {
            Ok(_) => {
                self.commands = commands;
                match self.start() {
                    Ok(_) => Ok(self.info()),
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
            .spawn() {
                Ok(proc) => proc,
                Err(_) => return Err(String::from("Failed to start MMB process"))
            };

        let mut data = self.data.write().unwrap();
        data.info.state = State::Running;
        data.mmb_process = Some(proc);

        let thr_data = Arc::clone(&self.data);
        let thr_work_path = self.job_dir.clone();
        self.watcher = Some(thread::spawn(move || {
            loop {
                {
                    let mut d = thr_data.write().unwrap();
                    match &mut d.mmb_process {
                        Some(proc) => {
                            match proc.try_wait().unwrap() {
                                Some(status) => {
                                    if status.success() {
                                        d.info.state = State::Finished;
                                        println!("Job finished");
                                    } else {
                                        d.info.state = State::Failed;
                                        println!("Job failed");
                                    }

                                    d.info.last_completed_stage = get_last_completed_stage(&thr_work_path);
                                    return;
                                },
                                None => {}
                            }
                        },
                        None => panic!("No process handle")
                    }
                }
                thread::sleep(Duration::from_millis(1000));
            };
        }));

        Ok(())
    }

    pub fn stop(&mut self) -> Result<JobInfo, String> {
        {
            let mut data = self.data.write().unwrap();

            match data.info.state {
                State::Finished => return Ok(data.info.clone()),
                State::Failed => return Ok(data.info.clone()),
                _ => {},
            }

            let pid = data.mmb_process.as_ref().unwrap().id();
            if signal::kill(Pid::from_raw(pid as i32), Signal::SIGTERM).is_err() {
                return Err(String::from("Failed to signal job process"));
            }
        
            let mut attempts = 0;
            while attempts < 10 {
                match data.mmb_process.as_mut().unwrap().try_wait() {
                    Ok(_) => break,
                    Err(_) => {},
                }
                attempts += 1;
                thread::sleep(Duration::from_micros(100));
            }

            if data.mmb_process.as_mut().unwrap().kill().is_err() {
                return Err(String::from("Failed to kill job process"));
            }
        }

        if let Some(watcher) = self.watcher.take() {
            watcher.join().expect("Failed to join watcher thread");
        }

        let data = self.data.read().unwrap();
        Ok(data.info.clone())
    }
}

impl Drop for Job {
    fn drop(&mut self) {
        let data = self.data.write().unwrap();

        assert!(data.info.state != State::Running);

        std::fs::remove_dir_all(&self.job_dir);
        println!("Job dropped");
    }
}