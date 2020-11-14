use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use regex::Regex;

use crate::config::Config;
use crate::session::session::Session;

pub struct SessionManager {
    sessions: HashMap<String, Arc<Session>>,
    cfg: Arc<Config>,
    username_checker: Regex,
}

impl<'a> SessionManager {
    pub fn create(cfg: Arc<Config>) -> SessionManager {
        SessionManager {
            sessions: HashMap::new(),
            cfg,
            username_checker: Regex::new(r"^[0-9A-z_\-]+$").unwrap(),
        }
    }

    pub fn create_session(&mut self, username: String) -> Result<(), String>{
        if !self.username_checker.is_match(username.as_str()) {
            return Err(String::from("Invalid username"));
        }

        match self.sessions.get_mut(&username) {
            Some(s) => {
                s.set_login_state(true);
                Ok(())
            },
            None => {
                let mut jobs_dir = PathBuf::from(&self.cfg.jobs_dir);
                jobs_dir.push(&username);
                match Session::create(
                        username.clone(),
                        true,
                        PathBuf::from(self.cfg.mmb_exec_path.as_str()),
                        PathBuf::from(self.cfg.mmb_parameters_path.clone()),
                        jobs_dir
                    ) {
                    Ok(s) => {
                        self.sessions.insert(username, Arc::from(s));
                        Ok(())
                    },
                    Err(e) => Err(e),
                }
            },
        }
    }

    pub fn get_session(&self, username: String) -> Option<Arc<Session>> {
        match self.sessions.get(&username) {
            Some(s) => Some(s.clone()),
            None => None,
        }
    }
}
