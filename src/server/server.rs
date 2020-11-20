use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use rocket::http::{Cookies, Status}; //FIXME: CookieJar???
use rocket::{get, post, routes, State};
use rocket::outcome::IntoOutcome;
use rocket::request::{self, FromRequest};
use rocket::Request;
use rocket::response::{NamedFile, Redirect};
use rocket::uri;

use crate::config::Config;
use crate::session;
use crate::session::session_manager::SessionManager;
use crate::server::api;
use crate::server::request_handlers;
use crate::server::responders::{PdbFile, WMSError};
use crate::session::session::Session;
use crate::server::session_cookie;

struct AppState {
    pub sm: RwLock<SessionManager>,
    pub jobs_dir: PathBuf,
    pub domain: String,
}

#[derive(Debug)]
struct User(String);

impl<'a, 'r> FromRequest<'a, 'r> for User {
    type Error = std::convert::Infallible;

    fn from_request(request: &'a Request<'r>) -> request::Outcome<User, Self::Error> {
        request.cookies()
            .get_private("auth")
            .and_then(|c| Some(String::from(c.value())))
            .map(|username| User(username))
            .or_forward(())
    }
}

fn get_session(cookies: &mut Cookies, state: State<AppState>) -> Option<Arc<Session>> {
    match session_cookie::get_session_username(cookies) {
        Some(session_id) => {
            state.sm.write().unwrap().get_session(&session_id)
        },
        None => {
            None
        },
    }
}

fn get_session_authorized(cookies: &mut Cookies, state: State<AppState>) -> Option<Arc<Session>> {
    match get_session(cookies, state) {
        Some(s) => {
            match s.is_logged_in() {
                true => Some(s),
                false => None,
            }
        },
        None => None,
    }
}

#[get("/auth")]
fn auth_already_authenticated(_user: User, mut cookies: Cookies, state: State<AppState>) -> Redirect {
    match get_session_authorized(&mut cookies, state) {
        Some(_) => Redirect::to(uri!(index_authorized)),
        None => {
            session_cookie::remove_session_cookie(&mut cookies);
            Redirect::to(uri!(auth_page))
        },
    }
}

#[get("/auth", rank = 2)]
fn auth_page() -> Result<NamedFile, WMSError> {
    match NamedFile::open(Path::new("assets/login.html")) {
        Ok(file) => Ok(file),
        Err(_) => Err(WMSError{ status: Status::NotFound }),
    }
}

#[post("/auth", data = "<auth>")]
fn auth_verify(auth: api::AuthRequest, mut cookies: Cookies, state: State<AppState>) -> Result<Redirect, api::AuthFailResponse> {
    match auth {
        api::AuthRequest::LogIn(data) => {
            if data.session_id == "" {
                let id = session::new_uuid();
                let c = session_cookie::make_auth_cookie(state.domain.clone(), session::uuid_to_str(&id));
                cookies.add_private(c);
                match state.sm.write().unwrap().create_session(&id) {
                    Ok(_) => Ok(Redirect::to(uri!(index_authorized))),
                    Err(e) => Err(api::AuthFailResponse{status: Status::InternalServerError, reason: e.to_string()}),
                }
            } else {
                let id = match session::str_to_uuid(data.session_id.as_str().trim()) {
                    Ok(id) => id,
                    Err(_) => return Err(api::AuthFailResponse{status: Status::BadRequest, reason: String::from("Invalid session ID")}),
                };
                match state.sm.write().unwrap().get_session(&id) {
                    Some(session) => {
                        let c = session_cookie::make_auth_cookie(state.domain.clone(), session::uuid_to_str(&id));
                        cookies.add_private(c);
                        session.set_login_state(true);
                        Ok(Redirect::to(uri!(index_authorized)))
                    },
                    None => Err(api::AuthFailResponse{status: Status::BadRequest, reason: String::from("No such session")}),
                }
            }
        },
        api::AuthRequest::LogOut(_) => {
            match get_session_authorized(&mut cookies, state) {
                Some(s) => {
                    s.set_login_state(false);
                },
                None => {},
            }
            session_cookie::remove_session_cookie(&mut cookies);
            Ok(Redirect::to(uri!(auth_page)))
        }
    }
}

#[get("/", rank = 2)]
fn index(mut cookies: Cookies, state: State<AppState>) -> Redirect {
    match get_session(&mut cookies, state) {
        Some(s) => {
            if s.is_logged_in() {
                return Redirect::to(uri!(index_authorized));
            } else {
                session_cookie::remove_session_cookie(&mut cookies);
                return Redirect::to(uri!(auth_page));
            }
        },
        None => Redirect::to(uri!(auth_page)),
    }
}

#[get("/")]
fn index_authorized(_user: User, mut cookies: Cookies, state: State<AppState>) -> Result<NamedFile, WMSError> {
    match get_session_authorized(&mut cookies, state) {
        Some(s) => {
            match NamedFile::open("assets/index.html") {
                Ok(f) => Ok(f),
                Err(_) => Err(WMSError{ status: Status::NotFound })
            }
        },
        None => {
            session_cookie::remove_session_cookie(&mut cookies);
            Err(WMSError{ status: Status::Forbidden })
        },
    }
}

#[get("/<file..>", rank = 3)]
fn static_files(file: PathBuf) -> Option<NamedFile> {
    NamedFile::open(Path::new("assets/").join(file)).ok()
}

#[post("/api", format = "application/json", data = "<req>")]
fn api(req: api::ApiRequest, mut cookies: Cookies, state: State<AppState>) -> Result<api::ApiResponse, WMSError> {
    let s = match get_session_authorized(&mut cookies, state) {
        Some(s) => s,
        None => return Err(WMSError{ status: Status::Forbidden }),
    };

    match req {
        api::ApiRequest::StartJob(v) => Ok(request_handlers::start_job(s, v.data)),
        api::ApiRequest::StopJob(v) => Ok(request_handlers::stop_job(s, v.data)),
        api::ApiRequest::ResumeJob(v) => Ok(request_handlers::resume_job(s, v.data)),
        api::ApiRequest::DeleteJob(v) => Ok(request_handlers::delete_job(s, v.data)),
        api::ApiRequest::JobStatus(v) => Ok(request_handlers::job_status(s, v.data)),
        api::ApiRequest::ListJobs(_) => Ok(request_handlers::list_jobs(s)),
        api::ApiRequest::JobCommands(v) => Ok(request_handlers::job_commands(s, v.data)),
        api::ApiRequest::SessionInfo(_) => Ok(request_handlers::session_info(s)),
    }
}

#[get("/structure/<session_id>/<job_id>/<stage>", rank = 1)]
fn structure(session_id: String, job_id: String, stage: String, state: State<AppState>) -> Result<PdbFile, WMSError> {
    let sid = match session::str_to_uuid(session_id.as_str()) {
        Ok(sid) => sid,
        Err(_) => return Err(WMSError{ status: Status::NotFound }),
    };
    let jid = match session::str_to_uuid(job_id.as_str()) {
        Ok(jid) => jid,
        Err(_) => return Err(WMSError{ status: Status::NotFound }),
    };

    if stage.to_lowercase() == "last" {
        match state.sm.read().unwrap().get_session(&sid) {
            Some(session) => {
                let stage_num = session.job_last_completed_stage(&jid);
                match stage_num {
                    Some(stage_num) => {
                        match session::trajectory_file_path(&state.jobs_dir, session_id.as_str(), job_id.as_str(), if stage_num == 0 { 1 } else { stage_num }) {
                            Ok(path) => return Ok(PdbFile{ path }),
                            Err(_) => return Err(WMSError{ status: Status::NotFound }),
                        }
                    },
                    None => return Err(WMSError{ status: Status::NotFound }),
                }
            },
            None => return Err(WMSError{ status: Status::NotFound }),
        }
    }

    let stage_num = stage.parse::<i32>();
    if stage_num.is_err() {
        return Err(WMSError{ status: Status::BadRequest });
    }

    match session::trajectory_file_path(&state.jobs_dir, session_id.as_str(), job_id.as_str(), stage_num.unwrap()) {
        Ok(path) => Ok(PdbFile{ path }),
        Err(_) => Err(WMSError{ status: Status::NotFound }),
    }
}

pub fn start(cfg: Arc<Config>) {
    let srv_cfg = rocket::config::Config::build(rocket::config::Environment::Staging)
        .root(cfg.root_dir.clone())
        .port(cfg.port)
        .finalize()
        .expect("Server configuration is invalid");

    rocket::custom(srv_cfg)
        .mount("/", routes![index, index_authorized, auth_page, auth_already_authenticated, auth_verify, static_files, api, structure])
        .manage( AppState{
            sm: RwLock::new(SessionManager::create(cfg.clone())),
            jobs_dir: PathBuf::from(cfg.jobs_dir.as_str()),
            domain: cfg.domain.clone(),
        })
        .launch();
}
