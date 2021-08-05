use std::{collections::HashMap, io::Write};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use file_lock::FileLock;
use uuid::Uuid;

use crate::config;
use crate::mmb;
use crate::server::api;

use super::job_runner::JobRunner;
use super::local_job_runner::LocalJobRunner;
use super::pbs_job_runner::PbsJobRunner;

#[derive(Clone)]
struct AdditionalFileInternal {
    size: u64,
}

struct FileTransfer {
    pub fh: std::fs::File,
    pub file_name: String,
    pub last_index: u32,
    pub last_activity: SystemTime,
}

struct Progress {
    pub state: mmb::State,
    pub step: i32,
    pub total_steps: i32,
}

#[derive(Clone)]
pub enum CommandsMode {
    None,
    Synthetic,
    Raw,
}

pub struct AdditionalFile {
    pub name: String,
    pub size: u64,
}

#[derive(Clone)]
pub struct JobInfo {
    pub name: String,
    pub state: mmb::State,
    pub available_stages: Vec<i32>,
    pub current_stage: Option<i32>,
    pub created_on: u128,
    pub commands_mode: CommandsMode,
    pub progress: Option<JobProgress>,
}

#[derive(Clone)]
pub struct JobProgress {
    pub step: i32,
    pub total_steps: i32,
}

pub struct Job {
    pub name: String,
    commands: Option<api::Commands>,
    raw_commands: Option<String>,
    job_dir: PathBuf,
    cmds_file_path: PathBuf,
    diag_file_path: PathBuf,
    progress_file_path: PathBuf,
    runner: Box<dyn JobRunner + Send + Sync>,
    current_stage: Option<i32>,
    created_on: SystemTime,
    file_transfers: HashMap<Uuid, FileTransfer>,
    additional_files: HashMap<String, AdditionalFileInternal>,
    file_transfer_timeout: Duration,
}

fn clear_stages(path: &PathBuf, stage: i32) -> Result<(), String> {
    let dir_lister = match std::fs::read_dir(path) {
        Ok(v) => v,
        Err(e) => return Err(e.to_string()),
    };

    let traj_prefix = format!("{}.", mmb::TRAJECTORY_FILE_PREFIX);
    let last_prefix = format!("{}.", mmb::LAST_FRAME_FILE_PREFIX);
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

        if name == mmb::PGRS_FILE_NAME ||
           name == mmb::DOUT_FILE_NAME {
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

fn mk_cmds_file_path(mut base_path: PathBuf) -> PathBuf {
    base_path.push(mmb::CMDS_FILE_NAME);
    base_path
}

fn mk_diag_file_path(mut base_path: PathBuf) -> PathBuf {
    base_path.push(mmb::DOUT_FILE_NAME);
    base_path
}

fn mk_progress_file_path(mut base_path: PathBuf) -> PathBuf {
    base_path.push(mmb::PGRS_FILE_NAME);
    base_path
}

fn mk_runner() -> Result<Box<dyn JobRunner + Sync + Send>, String> {
    if config::get().use_pbs_offloading {
        let runner = match PbsJobRunner::create() {
            Ok(runner) => runner,
            Err(e) => return Err(e),
        };
        Ok(Box::new(runner))
    } else {
        let runner = match LocalJobRunner::create() {
            Ok(runner) => runner,
            Err(e) => return Err(e),
        };
        Ok(Box::new(runner))
    }
}

fn process_stages(commands: &api::Commands) -> Result<mmb::commands::Stages, String> {
    let stages = match mmb::commands::stages(&commands) {
        Ok(stages) => stages,
        Err(e) => return Err(e),
    };
    if stages.first != stages.last {
        return Err(String::from("Calculation spanning over multiple stages is not supported"));
    }

    Ok(stages)
}

fn read_diagnostics(path: &Path) -> Result<String, String> {
    if !path.is_file() {
        return Ok(String::new());
    }

    match std::fs::File::open(path) {
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

fn read_mmb_progress(path: &Path) -> Result<Option<Progress>, String> {
    /* If the progress file does not exist, it could mean that MMB just has not created it yet */
    if !path.is_file() {
        return Ok(None);
    }

    let path_str = match path.to_str() {
        Some(s) => s,
        None => return Err(String::from("Cannot convert progress file path to str")),
    };

    let mut locked = match FileLock::lock(path_str, false, false) {
        Ok(locked) => locked,
        Err(e) => {
            /* Error here may indicate that the progress file is locked by MMB */
            return Ok(None);
        },
    };

    let mut s = String::new();
    let len = match locked.file.read_to_string(&mut s) {
        Ok(len) => len,
        Err(e) => return Err(format!("Cannot read progress report file: {}", e)),
    };
    if len == 0 {
        return Ok(None);
    }

    let json: serde_json::Result<mmb::Progress> = serde_json::from_str(s.as_str());
    match json {
        Ok(progress) => Ok(Some(Progress{
            state: progress.state,
            step: progress.step,
            total_steps: progress.total_steps,
        })),
        Err(e) => Err(format!("Cannot parse progress file: {}", e)),
    }
}

pub fn remove_file(path: &Path) -> Result<(), String> {
    if path.exists() {
        match std::fs::remove_file(path) {
            Ok(_) => return Ok(()),
            Err(e) => return Err(e.to_string()),
        }
    }
    Ok(())
}

impl Job {
    pub fn available_stages(&self) -> Vec<i32> {
        get_stages(&self.job_dir, mmb::TRAJECTORY_FILE_PREFIX)
    }

    pub fn cancel_upload(&mut self, transfer_id: &Uuid) -> Result<(), String> {
        match self.file_transfers.remove(&transfer_id) {
            Some(xfr) => {
                self.delete_file(&xfr.file_name);
                Ok(())
            },
            None => Err(String::from("No such transfer")),
        }
    }

    pub fn clone(name: String, job_dir: PathBuf, src: &Job) -> Result<Job, String> {
        if !src.file_transfers.is_empty() {
            return Err(String::from("Jobs with active file transfers cannot be cloned"));
        }

        match copy_job_dir(&job_dir, &src.job_dir) {
            Ok(_) => (),
            Err(e) => return Err(e),
        };

        let cmds_file_path = mk_cmds_file_path(job_dir.clone());
        let diag_file_path = mk_diag_file_path(job_dir.clone());
        let progress_file_path = mk_progress_file_path(job_dir.clone());

        let runner = match mk_runner() {
            Ok(runner) => runner,
            Err(e) => return Err(e),
        };

        Ok(Job{
            name,
            commands: src.commands.clone(),
            raw_commands: src.raw_commands.clone(),
            job_dir,
            cmds_file_path,
            diag_file_path,
            progress_file_path,
            runner,
            current_stage: src.current_stage,
            created_on: SystemTime::now(),
            file_transfers: HashMap::new(),
            additional_files: src.additional_files.clone(),
            file_transfer_timeout: src.file_transfer_timeout.clone(),
        })
    }

    pub fn commands(&self) -> Option<api::Commands> {
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

        CommandsMode::None
    }

    pub fn commands_raw(&self) -> Option<String> {
        self.raw_commands.clone()
    }

    pub fn create(name: String, job_dir: PathBuf, commands: Option<api::Commands>, raw_commands: Option<String>) -> Result<Job, String> {
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

        let cmds_file_path = mk_cmds_file_path(job_dir.clone());
        let diag_file_path = mk_diag_file_path(job_dir.clone());
        let progress_file_path = mk_progress_file_path(job_dir.clone());

        let runner = match mk_runner() {
            Ok(runner) => runner,
            Err(e) => return Err(e),
        };

        Ok(Job{
            name,
            commands,
            raw_commands,
            job_dir,
            cmds_file_path,
            diag_file_path,
            progress_file_path,
            runner,
            current_stage,
            created_on: std::time::SystemTime::now(),
            file_transfers: HashMap::new(),
            additional_files: HashMap::new(),
            file_transfer_timeout: Duration::new(30, 0),
        })
    }

    pub fn density_map_file_name(&self) -> Option<String> {
        match &self.commands {
            Some(cmds) => match &cmds.concrete {
                api::ConcreteCommands::DensityFit(v) => Some(v.density_map_file_name.clone()),
                _ => None
            },
            None => None,
        }
    }

    pub fn delete_additional_file(&mut self, file_name: String) -> Result<(), String> {
        match self.additional_files.remove(&file_name) {
            Some(_) => {
                self.delete_file(&file_name);
                Ok(())
            },
            None => Err(String::from("No such file")),
        }
    }

    pub fn diagnostics(&mut self) -> Result<String, String> {
        read_diagnostics(self.diag_file_path.as_path())
    }

    pub fn dir(&self) -> PathBuf {
        self.job_dir.clone()
    }

    pub fn finish_upload(&mut self, id: Uuid) -> Result<(), String> {
        if !self.file_transfers.contains_key(&id) {
            return Err(String::from("No such transfer"));
        }

        let xfr = self.file_transfers.remove(&id).unwrap();
        match xfr.fh.sync_all() {
            Ok(_) =>
                match xfr.fh.metadata() {
                    Ok(m) => {
                        self.additional_files.insert(xfr.file_name, AdditionalFileInternal{size: m.len()});
                        Ok(())
                    },
                    Err(e) => {
                        // TODO: Delete the file
                        Err(format!("Cannot get metadata: {}", e.to_string()))
                    },
                },
            Err(e) => Err(format!("Cannot write file: {}", e.to_string())),
        }
    }

    pub fn info(&mut self) -> Result<JobInfo, String> {
        let executor_state = self.runner.executor_state()?;
        let maybe_progress = read_mmb_progress(self.progress_file_path.as_path())?;

        match maybe_progress {
            Some(progress) => {
                let reported_state = {
                    if executor_state == mmb::State::Running {
                        /* MMB reports the job has finished but the MMB process is still running
                           Wait until the MMB process actually terminates */
                        mmb::State::Running
                    } else if progress.state == mmb::State::Running &&
                        executor_state != mmb::State::Running {
                            /* MMB reports that the job is running but its process has died
                               Report this as an error */
                        mmb::State::Failed
                    } else {
                        progress.state
                    }
                };

                Ok(JobInfo{
                    name: self.name.clone(),
                    state: reported_state,
                    available_stages: self.available_stages(),
                    current_stage: self.current_stage,
                    created_on: self.created_on.duration_since(SystemTime::UNIX_EPOCH).unwrap().as_millis(),
                    commands_mode: self.commands_mode(),
                    progress: Some(JobProgress{
                        step: progress.step,
                        total_steps: progress.total_steps,
                    }),
                })
            },
            None => {
                let reported_state = if executor_state == mmb::State::Unknown {
                    mmb::State::NotStarted
                    } else {
                        executor_state
                };

                Ok(JobInfo{
                    name: self.name.clone(),
                    state: reported_state,
                    available_stages: self.available_stages(),
                    current_stage: self.current_stage,
                    created_on: self.created_on.duration_since(SystemTime::UNIX_EPOCH).unwrap().as_millis(),
                    commands_mode: self.commands_mode(),
                    progress: None,
                })
            },
        }
    }

    pub fn init_upload(&mut self, file_name: String) -> Result<Uuid, String> {
        let id = Uuid::new_v4();

        if self.file_transfers.contains_key(&id) {
            return Err(String::from("File transfer with such id already exists"));
        }

        for v in self.file_transfers.values() {
            if v.file_name == file_name {
                return Err(String::from("Transfer with such file name already exists"));
            }
        }

        if mmb::additional_files::is_reserved_file_name(&file_name) {
            return Err(String::from("Filename is reserved"));
        }

        let mut path = self.job_dir.clone();
        path.push(&file_name);

        let fh = match std::fs::File::create(path) {
            Ok(fh) => fh,
            Err(e) => return Err(e.to_string()),
        };

        self.file_transfers.insert(
            id,
            FileTransfer{
                fh,
                file_name,
                last_index: u32::MAX,
                last_activity: SystemTime::now(),
            }
        );

        Ok(id)
    }

    pub fn last_available_stage(&self) -> Option<i32> {
        match get_stages(&self.job_dir, mmb::TRAJECTORY_FILE_PREFIX).last() {
            Some(v) => Some(*v),
            None => None
        }
    }

    pub fn list_additional_files(&self) -> Vec<AdditionalFile> {
        self.additional_files.iter().map(|(k, v)| { AdditionalFile{name: k.clone(), size: v.size} }).collect()
    }

    pub fn start(&mut self, commands: api::Commands) -> Result<(), String> {
        if self.raw_commands.is_some() {
            return Err(String::from("Job created in raw commands mode cannot be run in synthetic commands mode"));
        }

        self.commands = Some(commands);

        let stages = process_stages(self.commands.as_ref().unwrap())?;

        self.prune_job_dir(stages.first)?;
        match mmb::commands::write(&self.cmds_file_path, self.commands.as_ref().unwrap(), stages.first) {
            Ok(_) => (),
            Err(e) => return Err(e.to_string()),
        };

        self.runner.start(self.job_dir.clone(), self.cmds_file_path.as_path(), self.diag_file_path.as_path(), self.progress_file_path.as_path())
    }

    pub fn start_raw(&mut self, raw_commands: String) -> Result<(), String> {
        if self.commands.is_some() {
            return Err(String::from("Job created in synthetic commands mode cannot be run in raw commands mode"));
        }

        let parsed = match mmb::commands::parse_raw(&raw_commands) {
            Ok(v) => v,
            Err(e) => return Err(e),
        };

        self.prune_job_dir(parsed.first_stage)?;
        match mmb::commands::write_raw(&self.cmds_file_path, &raw_commands) {
            Ok(_) => (),
            Err(e) => return Err(e.to_string()),
        }
        self.raw_commands = Some(raw_commands);

        self.runner.start(self.job_dir.clone(), self.cmds_file_path.as_path(), self.diag_file_path.as_path(), self.progress_file_path.as_path())
    }

    pub fn stop(&mut self) -> Result<(), String> {
        self.runner.stop()
    }

    pub fn terminate_hung_uploads(&mut self) {
        let mut to_terminate = Vec::<Uuid>::new();

        for (k, xfr) in self.file_transfers.iter() {
            match SystemTime::now().duration_since(xfr.last_activity) {
                Ok(dt) => {
                    if dt > self.file_transfer_timeout {
                        to_terminate.push(*k);
                    }
                },
                Err(e) => panic!("{}", e.to_string().as_str()),
            }
        }

        for item in to_terminate.iter() {
            self.terminate_transfer(item);
        }
    }

    pub fn upload_chunk(&mut self, transfer_id: &Uuid, index: u32, chunk: Vec<u8>) -> Result<(), String> {
        match self.file_transfers.get_mut(transfer_id) {
            Some(xfr) => {
                let expected_index = xfr.last_index.wrapping_add(1);
                if index != expected_index {
                    return Err(String::from("Invalid chunk index"));
                }

                match xfr.fh.write(&chunk) {
                    Ok(_) => {
                        xfr.last_index = expected_index;
                        xfr.last_activity = SystemTime::now();
                        Ok(())
                    },
                    Err(e) => Err(format!("Failed to write file: {}", e.to_string())),
                }
            },
            None => Err(String::from("No such transfer")),
        }
    }

    fn delete_file(&self, file_path: &String) {
        let mut path = self.job_dir.clone();
        path.push(file_path);

        std::fs::remove_file(path);
    }

    fn prune_job_dir(&self, first_stage: i32) -> Result<(), String> {
        remove_file(self.cmds_file_path.as_path());
        remove_file(self.diag_file_path.as_path());
        remove_file(self.progress_file_path.as_path());
        clear_stages(&self.job_dir, first_stage)?;
        self.runner.prune_job_dir(self.job_dir.clone())
    }

    fn terminate_transfer(&mut self, id: &Uuid) {
        let file_name = self.file_transfers.remove(id).unwrap().file_name;

        let mut path = self.job_dir.clone();
        path.push(&file_name);
        std::fs::remove_file(path);

        println!("Terminating hung transfer of file \"{}\"", file_name);
    }
}

impl Drop for Job {
    fn drop(&mut self) {
        let xfr_ids: Vec<Uuid> = self.file_transfers.keys().map(|&id| id.clone()).collect();
        for id in xfr_ids {
            self.terminate_transfer(&id);
        }

        std::fs::remove_dir_all(&self.job_dir);
        println!("Job dropped");
    }
}
