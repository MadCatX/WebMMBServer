use std::collections::HashMap;
use std::fs::DirBuilder;
use std::path::PathBuf;
use std::sync::RwLock;
use uuid::Uuid;

use crate::mmb;
use crate::session;
use crate::session::job;

struct SessionData {
    jobs: HashMap<Uuid, job::Job>,
    is_logged_in: bool,
}

pub struct Session {
    data: RwLock<SessionData>,
    id: Uuid,
    mmb_exec_path: PathBuf,
    mmb_parameters_path: PathBuf,
    jobs_dir: PathBuf,
}

fn prepare_job_dir(root: &PathBuf, id: &Uuid, params: &PathBuf) -> Result<PathBuf, String> {
    let mut db = DirBuilder::new();
    db.recursive(false);

    let mut path = PathBuf::new();
    path.push(root); path.push(session::uuid_to_str(id));
    match db.create(&path) {
        Ok(_) => {
            let mut param_dst_path = path.clone();
            param_dst_path.push(mmb::PARAMETERS_FILE);
            match std::fs::copy(params, param_dst_path) {
                Ok(_) => Ok(path),
                Err(e) => Err(e.to_string()),
            }
        },
        Err(e) => Err(e.to_string()),
    }
}

impl Session {
    pub fn create(id: Uuid, is_logged_in: bool, mmb_exec_path: PathBuf, mmb_parameters_path: PathBuf, jobs_dir: PathBuf) -> Result<Session, String> {
        let mut db = DirBuilder::new();
        db.recursive(false);
        match db.create(&jobs_dir) {
            Ok(_) => {
                Ok(Session {
                    data: RwLock::new(
                        SessionData {
                            jobs: HashMap::new(),
                            is_logged_in,
                        
                        }
                    ),
                    id,
                    mmb_exec_path,
                    mmb_parameters_path,
                    jobs_dir,
                })
            },
            Err(e) => Err(e.to_string()),
        }
    }

    pub fn add_job(&self, name: String, commands: serde_json::Value) -> Result<(Uuid, job::JobInfo), String> {
        let mut data = self.data.write().unwrap();
        for (_, job) in &data.jobs {
            if job.name == name {
                return Err(String::from("Job with the same name already exists in this session"));
            }
        }

        let id = Uuid::new_v4();

        match prepare_job_dir(&self.jobs_dir, &id, &self.mmb_parameters_path) {
            Ok(job_dir) => {
                let mut job = match job::Job::create(
                    name,
                    commands,
                    self.mmb_exec_path.clone(),
                    job_dir
                ) {
                    Ok(job) => job,
                    Err(e) => return Err(e)
                };

                match job.start() {
                    Ok(_) => {
                        let info = match job.info() {
                            Ok(info) => info,
                            Err(_) => {
                                job::JobInfo{
                                    name: job.name.clone(),
                                    state: mmb::State::Running,
                                    step: 0,
                                    total_steps: 0,
                                    last_available_stage: 0,
                                    last_completed_stage: 0,
                                }
                            }
                        };
                        data.jobs.insert(id, job);
                        Ok((id, info))
                    },
                    Err(e) => Err(e),
                }
            },
            Err(e) => Err(e),
        }
    }

    pub fn delete_job(&self, id: &Uuid) -> bool {
        let mut job = {
            let mut data = self.data.write().unwrap();
            match data.jobs.remove(id) {
                Some(job) => job,
                None => return false,
            }
        };

        match job.info() {
            Ok(info) => {
                if info.state == mmb::State::Running {
                    if job.stop().is_err() {
                        return false;
                    }
                }
                true
            },
            Err(_) => false,
        }
    }

    pub fn has_job(&self, id: &Uuid) -> bool {
        self.data.read().unwrap().jobs.contains_key(id)
    }

    pub fn id(&self) -> Uuid {
        self.id.clone()
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

    pub fn job_commands(&self, id: Uuid) -> Option<serde_json::Value> {
        let data = self.data.read().unwrap();

        match data.jobs.get(&id) {
            Some(job) => Some(job.commands()),
            None => None,
        }
    }

    pub fn job_info(&self, id: Uuid) -> Option<Result<job::JobInfo, String>> {
        let mut data = self.data.write().unwrap();

        match data.jobs.get_mut(&id) {
            Some(job) => Some(job.info()),
            None => return None
        }
    }

    pub fn job_last_available_stage(&self, id: &Uuid) -> Option<i32> {
        let data = self.data.read().unwrap();

        match data.jobs.get(&id) {
            Some(job) => Some(job.last_available_stage()),
            None => None
        }
    }

    pub fn job_last_completed_stage(&self, id: &Uuid) -> Option<i32> {
        let data = self.data.read().unwrap();

        match data.jobs.get(&id) {
            Some(job) => Some(job.last_completed_stage()),
            None => None
        }
    }

    pub fn job_stdout(&self, id: &Uuid) -> Option<Result<String, String>> {
        let data = self.data.read().unwrap();

        match data.jobs.get(&id) {
            Some(job) => Some(job.stdout()),
            None => None
        }
    }

    pub fn resume_job(&self, id: &Uuid, commands: serde_json::Value) -> Result<job::JobInfo, String> {
        let mut data = self.data.write().unwrap();

        match data.jobs.get_mut(&id) {
            Some(job) => {
                match job.resume(commands) {
                    Ok(info) => {
                        match info {
                            Some(info) => Ok(info),
                            None => {
                                Ok(job::JobInfo{
                                    name: job.name.clone(),
                                    state: mmb::State::Running,
                                    step: 0,
                                    total_steps: 0,
                                    last_available_stage: 0,
                                    last_completed_stage: 0,
                                })
                            },
                        }
                    },
                    Err(e) => Err(e),
                }
            },
            None => Err(String::from("No job to continue")),
        }
    }

    pub fn set_login_state(&self, login_state: bool) {
        let mut data = self.data.write().unwrap();

        data.is_logged_in = login_state;
    }

    pub fn stop_job(&self, id: Uuid) -> Result<job::JobInfo, String> {
        let mut data = self.data.write().unwrap();

        match data.jobs.get_mut(&id) {
            Some(job) => return job.stop(),
            None => return Err(String::from("No such job")),
        }
    }
}
