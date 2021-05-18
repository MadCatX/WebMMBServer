use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use std::thread;
use uuid::Uuid;

use crate::config;
use crate::session;
use crate::session::session::Session;

pub struct SessionManager {
    sessions: HashMap<Uuid, Arc<Session>>,
    session_watchdogs: HashMap<Uuid, thread::JoinHandle<()>>,
}

impl<'a> SessionManager {
    pub fn create() -> SessionManager {
        SessionManager {
            sessions: HashMap::new(),
            session_watchdogs: HashMap::new(),
        }
    }

    pub fn create_session(&mut self, session_id: &Uuid) -> Result<(), String>{
        match self.sessions.get_mut(&session_id) {
            Some(s) => {
                s.set_login_state(true);
                Ok(())
            },
            None => {
                let mut jobs_dir = PathBuf::from(config::get().jobs_dir.clone());
                jobs_dir.push(session::uuid_to_str(&session_id));
                match Session::create(
                        session_id.clone(),
                        true,
                        jobs_dir
                    ) {
                    Ok(s) => {
                        let session_handle = Arc::from(s);
                        self.sessions.insert(*session_id, session_handle.clone());
                        self.session_watchdogs.insert(
                            *session_id,
                            thread::spawn(move || {
                                while session_handle.is_logged_in() {
                                    thread::sleep(Duration::new(10, 0));
                                    session_handle.terminate_hung_uploads();
                                }

                                println!("Session watchdog exited");
                            })
                        );

                        Ok(())
                    },
                    Err(e) => Err(e),
                }
            },
        }
    }

    pub fn destroy_session(&mut self, session_id: &Uuid) {
        let sess = match self.get_session(session_id) {
            Some(sess) => sess,
            None => return,
        };

        sess.set_login_state(false);

        self.session_watchdogs.remove(session_id);
    }

    pub fn get_session(&self, session_id: &Uuid) -> Option<Arc<Session>> {
        match self.sessions.get(session_id) {
            Some(s) => Some(s.clone()),
            None => None,
        }
    }
}
