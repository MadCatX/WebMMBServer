use std::collections::HashMap;
use std::fs::DirBuilder;
use std::path::PathBuf;
use std::sync::RwLock;
use uuid::Uuid;

use crate::config;
use crate::logging;
use crate::log_plain;
use crate::mmb;
use crate::server::api;
use crate::session;
use crate::session::{job, JobError};

const LOGSRC: &'static str = "session";

struct SessionData {
    jobs: HashMap<Uuid, job::Job>,
    is_logged_in: bool,
}

pub struct Session {
    data: RwLock<SessionData>,
    id: Uuid,
    jobs_dir: PathBuf,
}

fn prepare_job_dir(root: &PathBuf, id: &Uuid) -> Result<PathBuf, String> {
    let mut db = DirBuilder::new();
    db.recursive(false);

    let mut path = PathBuf::new();
    path.push(root); path.push(session::uuid_to_str(id));
    match db.create(&path) {
        Ok(_) => {
            let mut param_dst_path = path.clone();
            param_dst_path.push(mmb::PARAMS_FILE_NAME);
            match std::fs::copy(&config::get().mmb_parameters_path, param_dst_path) {
                Ok(_) => Ok(path),
                Err(e) => Err(e.to_string()),
            }
        },
        Err(e) => Err(e.to_string()),
    }
}

impl Session {
    pub fn cancel_upload(&self, job_id: &Uuid, transfer_id: &Uuid) -> Result<(), String> {
        let mut data = self.data.write().unwrap();

        match data.jobs.get_mut(job_id) {
            Some(job) => job.cancel_upload(transfer_id),
            None => Err(String::from("No such job")),
        }
    }

    pub fn create(id: Uuid, is_logged_in: bool, jobs_dir: PathBuf) -> Result<Session, String> {
        let mut db = DirBuilder::new();
        db.recursive(false);
        match db.create(&jobs_dir) {
            Ok(_) => {
                let data = RwLock::new(SessionData{
                        jobs: HashMap::new(),
                        is_logged_in,
                    }
                );

                Ok(Session{
                    data,
                    id,
                    jobs_dir,
                })
            },
            Err(e) => Err(e.to_string()),
        }
    }

    pub fn create_job(&self, name: String, synthetic_commands: Option<api::Commands>, raw_commands: Option<String>) -> Result<Uuid, JobError> {
        assert!(!(synthetic_commands.is_some() && raw_commands.is_some()));

        if name.len() < 1 {
            return Err(JobError::BadInput(String::from("Job must have a name")));
        }
        if self.has_job_by_name(&name) {
            return Err(JobError::BadInput(format!("Job with name {} already exists", name)));
        }

        let id = Uuid::new_v4();

        match prepare_job_dir(&self.jobs_dir, &id) {
            Ok(job_dir) => {
                match job::Job::create(name, job_dir, synthetic_commands, raw_commands) {
                    Ok(job) => {
                        let mut data = self.data.write().unwrap();
                        data.jobs.insert(id, job);
                        Ok(id)
                    },
                    Err(e) => Err(e),
                }
            },
            Err(e) => {
                log_plain!(Error, LOGSRC, &format!("Failed to create job directory: {}", e));
                Err(JobError::InternalError)
            },
        }
    }

    pub fn clone_job(&self, name: String, src_id: &Uuid) -> Result<Uuid, JobError> {
        if name.len() < 1 {
            return Err(JobError::BadInput(String::from("Job must have a name")));
        }
        if self.has_job_by_name(&name) {
            return Err(JobError::BadInput(format!("Job with name {} already exists", name)));
        }

        let id = Uuid::new_v4();
        let mut data = self.data.write().unwrap();
        let src_job = match data.jobs.get_mut(src_id) {
            Some(v) => v,
            None => return Err(JobError::BadInput(String::from("No job to clone"))),
        };

        match src_job.info() {
            Ok(info) => {
                if info.state == mmb::State::Running {
                    return Err(JobError::BadInput(String::from("Running jobs cannot be cloned")));
                }
            },
            Err(e) => {
                log_plain!(Error, LOGSRC, &format!("Failed to get job info for job ID {} that was to be cloned: {}", id, e));
                return Err(JobError::InternalError);
            }
        };

        match prepare_job_dir(&self.jobs_dir, &id) {
            Ok(job_dir) => {
                match job::Job::clone(name, job_dir, &src_job) {
                    Ok(job) => {
                        data.jobs.insert(id, job);
                        Ok(id)
                    },
                    Err(e) => Err(e),
                }
            },
            Err(e) => {
                log_plain!(Error, LOGSRC, &format!("Failed to create job directory: {}", e));
                Err(JobError::InternalError)
            },
        }
    }

    pub fn delete_job(&self, id: &Uuid) -> bool {
        let mut data = self.data.write().unwrap();

        let job = match data.jobs.get_mut(id) {
            Some(job) => job,
            None => return false,
        };

        match job.info() {
            Ok(info) => {
                if info.state == mmb::State::Running {
                    return false;
                }
            },
            Err(e) => {
                log_plain!(Error, LOGSRC, &format!("Cannot get info for job ID {}: {}", id, e));
                return false;
            },
        };

        data.jobs.remove(id);
        return true;
    }

    pub fn delete_additional_file(&self, id: &Uuid, file_name: String) -> Result<(), String> {
        let mut data = self.data.write().unwrap();

        match data.jobs.get_mut(id) {
            Some(job) => job.delete_additional_file(file_name),
            None => Err(String::from("No such job")),
        }
    }

    pub fn finish_upload(&self, job_id: Uuid, transfer_id: Uuid) -> Result<(), String> {
        let mut data = self.data.write().unwrap();

        match data.jobs.get_mut(&job_id) {
            Some(job) => job.finish_upload(transfer_id),
            None => Err(String::from("No such job"))
        }
    }

    pub fn has_job(&self, id: &Uuid) -> bool {
        self.data.read().unwrap().jobs.contains_key(id)
    }

    pub fn has_job_by_name(&self, name: &String) -> bool {
        let data = self.data.read().unwrap();
        for (_, job) in &data.jobs {
            if job.name == *name {
                return true;
            }

        }
        return false;
    }

    pub fn id(&self) -> Uuid {
        self.id.clone()
    }

    pub fn init_upload(&self, id: &Uuid, file_name: String) -> Result<Uuid, String> {
        let mut data = self.data.write().unwrap();

        match data.jobs.get_mut(id) {
            Some(job) => job.init_upload(file_name),
            None => Err(String::from("No such job"))
        }
    }

    pub fn is_logged_in(&self) -> bool {
        let data = self.data.read().unwrap();

        data.is_logged_in
    }

    pub fn list_jobs(&self) -> Vec<(Uuid, Result<job::JobInfo, String>)> {
        let mut data = self.data.write().unwrap();

        let mut list: Vec<(Uuid, Result<job::JobInfo, String>)> = Vec::new();
        for (id, job) in &mut data.jobs {
            list.push((*id, job.info()));
        }

        list
    }

    pub fn list_job_additional_files(&self, id: &Uuid) -> Result<Vec<job::AdditionalFile>, String> {
        let data = self.data.read().unwrap();

        match data.jobs.get(id) {
            Some(job) => Ok(job.list_additional_files()),
            None => Err(String::from("No such job")),
        }
    }

    pub fn job_commands(&self, id: Uuid) -> Result<Option<api::Commands>, String> {
        let data = self.data.read().unwrap();

        match data.jobs.get(&id) {
            Some(job) => Ok(job.commands()),
            None => Err(String::from("Unknown job id")),
        }
    }

    pub fn job_commands_mode(&self, id: Uuid) -> Option<api::JobCommandsMode> {
        let data = self.data.read().unwrap();

        match data.jobs.get(&id) {
            Some(job) => Some(job.commands_mode()),
            None => None,
        }
    }

    pub fn job_commands_raw(&self, id: Uuid) -> Result<Option<String>, String> {
        let data = self.data.read().unwrap();

        match data.jobs.get(&id) {
            Some(job) => Ok(job.commands_raw()),
            None => Err(String::from("Unknown job id")),
        }
    }

    pub fn job_density_map_file_name(&self, id: &Uuid) -> Option<String> {
        let data = self.data.read().unwrap();

        match data.jobs.get(&id) {
            Some(job) => job.density_map_file_name(),
            None => None,
        }
    }

    pub fn job_diagnostics(&self, id: &Uuid) -> Option<Result<String, String>> {
        let mut data = self.data.write().unwrap();

        match data.jobs.get_mut(&id) {
            Some(job) => Some(job.diagnostics()),
            None => None,
        }
    }

    pub fn job_dir(&self, id: &Uuid) -> Option<PathBuf> {
        let mut data = self.data.write().unwrap();

        match data.jobs.get_mut(&id) {
            Some(job) => Some(job.dir()),
            None => None,
        }
    }

    pub fn job_info(&self, id: Uuid) -> Result<job::JobInfo, String> {
        let mut data = self.data.write().unwrap();

        match data.jobs.get_mut(&id) {
            Some(job) => job.info(),
            None => Err(String::from("No such job")),
        }
    }

    pub fn job_last_available_stage(&self, id: &Uuid) -> Option<i32> {
        let data = self.data.read().unwrap();

        match data.jobs.get(&id) {
            Some(job) => job.last_available_stage(),
            None => None,
        }
    }

    pub fn retire_ended_jobs(&self) {
        let mut data = self.data.write().unwrap();
        for (id, job) in data.jobs.iter_mut() {
            if let Err(e) = job.check_retire() {
                log_plain!(Error, LOGSRC, &format!("Cannot determine if job {} can be retired: {}", id, e.to_string()));
            }
        }
    }

    pub fn set_login_state(&self, login_state: bool) {
        let mut data = self.data.write().unwrap();

        data.is_logged_in = login_state;
    }

    pub fn start_job(&self, id: &Uuid, commands: api::Commands) -> Result<(), JobError> {
        if !self.has_job(id) {
            return Err(JobError::BadInput(format!("Job with id {} does not exist", id)));
        }

        let mut data = self.data.write().unwrap();
        let job = data.jobs.get_mut(&id).unwrap();
        job.start(commands)
    }

    pub fn start_job_raw(&self, id: &Uuid, raw_commands: String) -> Result<(), JobError> {
        if !self.has_job(id) {
            return Err(JobError::BadInput(format!("Job with id {} does not exist", id)));
        }

        let mut data = self.data.write().unwrap();
        let job = data.jobs.get_mut(&id).unwrap();
        job.start_raw(raw_commands)
    }

    pub fn stop_job(&self, id: Uuid) -> Result<(), String> {
        let mut data = self.data.write().unwrap();

        match data.jobs.get_mut(&id) {
            Some(job) => job.stop(),
            None => Err(String::from("No such job")),
        }
    }

    pub fn terminate_hung_uploads(&self) {
        let mut data = self.data.write().unwrap();

        // @nocheckin: This needs to be handled
        for job in data.jobs.values_mut() {
            job.terminate_hung_uploads();
        }
    }

    pub fn upload_chunk(&self, job_id: &Uuid, transfer_id: &Uuid, index: u32, chunk: Vec<u8>) -> Result<(), String> {
        let mut data = self.data.write().unwrap();

        match data.jobs.get_mut(job_id) {
            Some(job) => job.upload_chunk(transfer_id, index, chunk),
            None => return Err(String::from("No such job")),
        }
    }
}
