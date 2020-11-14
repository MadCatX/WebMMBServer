use std::collections::HashMap;
use std::fs::DirBuilder;
use std::path::PathBuf;
use std::sync::RwLock;
use uuid::Uuid;

use crate::mmb;
use crate::session;
use crate::session::job;

struct SessionData {
    username: String,
    jobs: HashMap<Uuid, job::Job>,
    is_logged_in: bool,
}

pub struct Session {
    data: RwLock<SessionData>,
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
    pub fn create(username: String, is_logged_in: bool, mmb_exec_path: PathBuf, mmb_parameters_path: PathBuf, jobs_dir: PathBuf) -> Result<Session, String> {
        let mut db = DirBuilder::new();
        db.recursive(false);
        match db.create(&jobs_dir) {
            Ok(_) => {
                Ok(Session {
                    data: RwLock::new(
                        SessionData {
                            username,
                            jobs: HashMap::new(),
                            is_logged_in,
                        
                        }
                    ),
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
            if job.info().name == name {
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
                        let info = job.info();
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

        if job.info().state == job::State::Running {
            if job.stop().is_err() {
                return false;
            }
        }
        return true;
    }

    pub fn has_job(&self, id: &Uuid) -> bool {
        self.data.read().unwrap().jobs.contains_key(id)
    }

    pub fn is_logged_in(&self) -> bool {
        let data = self.data.read().unwrap();

        data.is_logged_in
    }

    pub fn list_jobs(&self) -> Vec<(Uuid, job::JobInfo)> {
        let data = self.data.read().unwrap();

        let mut list: Vec<(Uuid, job::JobInfo)> = Vec::new();
        for (id, job) in &data.jobs {
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

    pub fn job_info(&self, id: Uuid) -> Option<job::JobInfo> {
        let data = self.data.read().unwrap();

        match data.jobs.get(&id) {
            Some(job) => Some(job.info()),
            None => return None
        }
    }

    pub fn resume_job(&self, id: &Uuid, commands: serde_json::Value) -> Result<job::JobInfo, String> {
        let mut data = self.data.write().unwrap();

        match data.jobs.get_mut(&id) {
            Some(job) => {
                match job.resume(commands) {
                    Ok(info) => Ok(info),
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

    pub fn username(&self) -> String {
        let data = self.data.read().unwrap();

        data.username.clone()
    }
}
