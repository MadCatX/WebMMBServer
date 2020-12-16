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

    fn add_job(&self, name: String) -> Result<Uuid, String> {
        let id = Uuid::new_v4();

        match prepare_job_dir(&self.jobs_dir, &id, &self.mmb_parameters_path) {
            Ok(job_dir) => {
                match job::Job::create(
                    name,
                    self.mmb_exec_path.clone(),
                    job_dir
                ) {
                    Ok(job) => {
                        let mut data = self.data.write().unwrap();
                        data.jobs.insert(id, job);
                        Ok(id)
                    },
                    Err(e) => Err(e),
                }
            },
            Err(e) => Err(e.to_string()),
        }
    }

    pub fn clone_job(&self, name: String, src_id: &Uuid) -> Result<Uuid, String> {
        if name.len() < 1 {
            return Err(String::from("Job must have a name"));
        }
        if self.has_job_by_name(&name) {
            return Err(String::from("Job with such name already exists"));
        }

        let id = Uuid::new_v4();
        let mut data = self.data.write().unwrap();
        let src_job = match data.jobs.get_mut(src_id) {
            Some(v) => v,
            None => return Err(String::from("No job to clone")),
        };

        match src_job.info() {
            Ok(info) => {
                if info.state == mmb::State::Running {
                    return Err(String::from("Running jobs cannot be cloned"));
                }
            },
            Err(e) => return Err(e),
        };

        match prepare_job_dir(&self.jobs_dir, &id, &self.mmb_parameters_path) {
            Ok(job_dir) => {
                match job::Job::clone(
                    name,
                    self.mmb_exec_path.clone(),
                    job_dir,
                    &src_job
                ) {
                    Ok(job) => {
                        data.jobs.insert(id, job);
                        Ok(id)
                    },
                    Err(e) => Err(e),
                }
            },
            Err(e) => Err(e.to_string()),
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

    pub fn is_logged_in(&self) -> bool {
        let data = self.data.read().unwrap();

        data.is_logged_in
    }

    pub fn job_name_to_id(&self, name: &String) -> Option<Uuid> {
        for (id, job) in &self.data.read().unwrap().jobs {
            if job.name == *name {
                return Some(*id);
            }
        }
        None
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
            Some(job) => job.commands(),
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
            Some(job) => job.last_available_stage(),
            None => None,
        }
    }

    pub fn job_stdout(&self, id: &Uuid) -> Option<Result<String, String>> {
        let mut data = self.data.write().unwrap();

        match data.jobs.get_mut(&id) {
            Some(job) => Some(job.stdout()),
            None => None
        }
    }

    pub fn set_login_state(&self, login_state: bool) {
        let mut data = self.data.write().unwrap();

        data.is_logged_in = login_state;
    }

    pub fn start_job(&self, name: String, commands: serde_json::Value) -> Result<(Uuid, job::JobInfo), String> {
        let ret = if !self.has_job_by_name(&name) {
            self.add_job(name)
        } else {
            match self.job_name_to_id(&name) {
                Some(id) => Ok(id),
                None => Err(String::from("No such job")),
            }
        };

        let id = match ret {
            Ok(id) => id,
            Err(e) => return Err(e),
        };

        let mut data = self.data.write().unwrap();
        let job = data.jobs.get_mut(&id).unwrap();
        match job.start(commands) {
            Ok(()) => {
                match job.info() {
                    Ok(info) => Ok((id, info)),
                    Err(e) => Err(e),
                }
            },
            Err(e) => Err(e),
        }
    }

    pub fn stop_job(&self, id: Uuid) -> Result<job::JobInfo, String> {
        let mut data = self.data.write().unwrap();

        match data.jobs.get_mut(&id) {
            Some(job) => return job.stop(),
            None => return Err(String::from("No such job")),
        }
    }
}
