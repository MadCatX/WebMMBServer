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
use crate::server::responders::{ApiResponse, PdbFile, WMSError};
use crate::session::session::Session;
use crate::server::session_cookie;

struct AppState {
    pub sm: RwLock<SessionManager>,
    pub jobs_dir: PathBuf,
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
        Some(username) => {
            state.sm.write().unwrap().get_session(username)
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
        None => Redirect::to(uri!(auth_page)),
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
fn auth_verify(auth: api::AuthRequest, mut cookies: Cookies, state: State<AppState>) -> Result<Redirect, WMSError> {
    match auth {
        api::AuthRequest::LogIn(v) => {
            if v.username.len() < 1 {
                return Err(WMSError{ status: Status::BadRequest });
            }

            let c = session_cookie::make_auth_cookie(v.username.clone());
            cookies.add_private(c);
            match state.sm.write().unwrap().create_session(v.username) {
                Ok(_) => Ok(Redirect::to(uri!(index_authorized))),
                Err(e) => {
                    println!("{}", e);
                    Err(WMSError{ status: Status::BadRequest })
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
            match cookies.get_private("auth") {
                Some(c) => cookies.remove_private(c),
                None => {},
            }
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
                return Redirect::to(uri!(auth_page));
            }
        },
        None => {
            println!("No session");
            Redirect::to(uri!(auth_page))
        },
    }
}

#[get("/")]
fn index_authorized(_user: User, mut cookies: Cookies, state: State<AppState>) -> Result<NamedFile, WMSError> {
    match get_session_authorized(&mut cookies, state) {
        Some(s) => {
            let file = NamedFile::open("assets/index.html");
            match file {
                Ok(f) => Ok(f),
                Err(_) => Err(WMSError{ status: Status::NotFound })
            }
        },
        None => Err(WMSError{ status: Status::Forbidden }),
    }
}

#[get("/<file..>", rank = 3)]
fn static_files(file: PathBuf) -> Option<NamedFile> {
    NamedFile::open(Path::new("assets/").join(file)).ok()
}

#[post("/api", format = "application/json", data = "<req>")]
fn api(req: api::ApiRequest, mut cookies: Cookies, state: State<AppState>) -> Result<ApiResponse, WMSError> {
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

#[get("/structure/<username>/<id_str>/<stage>", rank = 1)]
fn structure(username: String, id_str: String, stage: i32, state: State<AppState>) -> Result<PdbFile, WMSError> {
    match session::trajectory_file_path(&state.jobs_dir, username.as_str(), id_str.as_str(), stage) {
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
        })
        .launch();
}
