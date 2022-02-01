use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use rocket;
use uuid::Uuid;

use crate::config;
use crate::logging;
use crate::session;
use crate::server::api as srvapi;
use crate::server::{request_handlers, session_cookie, transfer_handlers, LOGSRC};
use crate::server::responders::{DensityFile, PdbFile, WMSError};
use crate::session::session::Session;
use crate::session::session_manager::SessionManager;

struct AppState {
    pub sm: RwLock<SessionManager>,
    pub jobs_dir: PathBuf,
    pub examples_dir: PathBuf,
    pub domain: String,
    pub require_https: bool,
}

#[derive(Debug)]
struct SessionId(String);

fn check_str_is_uuid(s: String) -> Option<Uuid> {
    match session::str_to_uuid(s.as_str()) {
        Ok(uuid) => Some(uuid),
        Err(_) => None,
    }
}

#[rocket::async_trait]
impl<'a> rocket::request::FromRequest<'a> for SessionId {
    type Error = String;

    // REVIEW: Look into these Forwardings
    async fn from_request(request: &'a rocket::request::Request<'_>) -> rocket::request::Outcome<Self, Self::Error> {
        match request.guard::<&rocket::State<AppState>>().await {
            rocket::request::Outcome::Success(state) => {
                match request.cookies()
                    .get_private(session_cookie::AUTH_NAME)
                    .and_then(|c| check_str_is_uuid(String::from(c.value()))) {
                        Some(uuid) => {
                            match state.sm.write().unwrap().get_session(&uuid) {
                                Some(session) => {
                                    if session.is_logged_in() {
                                        return rocket::request::Outcome::Success(SessionId(session::uuid_to_str(&uuid)));
                                    }
                                    rocket::request::Outcome::Forward(())
                                },
                                None => rocket::request::Outcome::Forward(()),
                            }
                        }
                        None => rocket::request::Outcome::Forward(()),
                    }
            },
            _ => {
                logging::log(logging::Priority::Error, LOGSRC, &format!("Failed to get application state"));
                rocket::request::Outcome::Failure((rocket::http::Status::InternalServerError, String::from("Internal server error")))
            }
        }
    }
}

fn get_session(jar: &rocket::http::CookieJar<'_>, state: &AppState) -> Option<Arc<Session>> {
    match session_cookie::get_session_id(jar) {
        Some(session_id) => {
            state.sm.write().unwrap().get_session(&session_id)
        },
        None => {
            None
        },
    }
}

fn get_session_authorized(jar: &rocket::http::CookieJar<'_>, state: &AppState) -> Option<Arc<Session>> {
    match get_session(jar, state) {
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
fn auth_already_authenticated(_sid: SessionId, jar: &rocket::http::CookieJar<'_>, state: &rocket::State<AppState>) -> rocket::response::Redirect {
    match get_session_authorized(jar, &state) {
        Some(_) => rocket::response::Redirect::to(uri!(index_authorized)),
        None => {
            session_cookie::remove_session_cookie(jar);
            rocket::response::Redirect::to(uri!(auth_page))
        },
    }
}

#[get("/auth", rank = 2)]
async fn auth_page() -> Result<rocket::fs::NamedFile, WMSError> {
    match rocket::fs::NamedFile::open(Path::new("assets/login.html")).await {
        Ok(file) => Ok(file),
        Err(_) => Err(WMSError{ status: rocket::http::Status::NotFound }),
    }
}

#[post("/auth", data = "<auth>")]
fn auth_verify(auth: srvapi::AuthRequest, jar: &rocket::http::CookieJar<'_>, state: &rocket::State<AppState>) -> srvapi::AuthResponse {
    match auth {
        srvapi::AuthRequest::LogIn(data) => {
            if data.session_id == "" {
                let id = session::new_uuid();
                let c = session_cookie::make_auth_cookie(state.domain.clone(), session::uuid_to_str(&id), state.require_https);
                jar.add_private(c);
                match state.sm.write().unwrap().create_session(&id) {
                    Ok(_) => srvapi::AuthResponse{ status: rocket::http::Status::Ok, message: String::new() },
                    Err(e) => srvapi::AuthResponse{ status: rocket::http::Status::InternalServerError, message: e.to_string() },
                }
            } else {
                let id = match session::str_to_uuid(data.session_id.as_str().trim()) {
                    Ok(id) => id,
                    Err(_) => return srvapi::AuthResponse{ status: rocket::http::Status::BadRequest, message: String::from("Invalid session ID") },
                };
                match state.sm.write().unwrap().get_session(&id) {
                    Some(session) => {
                        let c = session_cookie::make_auth_cookie(state.domain.clone(), session::uuid_to_str(&id), state.require_https);
                        jar.add_private(c);
                        session.set_login_state(true);
                        srvapi::AuthResponse{ status: rocket::http::Status::Ok, message: String::new() }
                    },
                    None => srvapi::AuthResponse{status: rocket::http::Status::BadRequest, message: String::from("No such session")},
                }
            }
        },
        srvapi::AuthRequest::LogOut(_) => {
            match session_cookie::get_session_id(jar) {
                Some(sid) => state.sm.write().unwrap().destroy_session(&sid),
                None => (),
            };
            session_cookie::remove_session_cookie(jar);
            srvapi::AuthResponse{ status: rocket::http::Status::Ok, message: String::new() }
        }
    }
}

#[get("/", rank = 2)]
fn index(jar: &rocket::http::CookieJar<'_>, state: &rocket::State<AppState>) -> rocket::response::Redirect {
    match get_session(jar, &state) {
        Some(s) => {
            if s.is_logged_in() {
                return rocket::response::Redirect::to(uri!(index_authorized));
            } else {
                session_cookie::remove_session_cookie(jar);
                return rocket::response::Redirect::to(uri!(auth_page));
            }
        },
        None => rocket::response::Redirect::to(uri!(auth_page)),
    }
}

#[get("/")]
async fn index_authorized(_sid: SessionId, jar: &rocket::http::CookieJar<'_>, state: &rocket::State<AppState>) -> Result<rocket::fs::NamedFile, WMSError> {
    match get_session_authorized(jar, &state) {
        Some(_) => {
            match rocket::fs::NamedFile::open("assets/index.html").await {
                Ok(f) => Ok(f),
                Err(_) => Err(WMSError{ status: rocket::http::Status::NotFound })
            }
        },
        None => {
            session_cookie::remove_session_cookie(jar);
            Err(WMSError{ status: rocket::http::Status::Forbidden })
        },
    }
}

#[get("/<file..>", rank = 3)]
async fn static_files(file: PathBuf) -> Result<rocket::fs::NamedFile, WMSError> {
    match rocket::fs::NamedFile::open(Path::new("assets/").join(&file)).await {
        Ok(file) => Ok(file),
        Err(_) => {
            logging::log(logging::Priority::Warning, LOGSRC, &format!("Non-existent static asset {} requested", file.as_os_str().to_str().unwrap_or(logging::INV_FILE_NAME)));
            Err(WMSError{ status: rocket::http::Status::NotFound })
        }
    }
}

#[get("/additional_file/<session_id>/<job_id>/<file_name>")]
async fn additional_file(session_id: String, job_id: String, file_name: String, jar: &rocket::http::CookieJar<'_>, state: &rocket::State<AppState>) -> Result<rocket::fs::NamedFile, WMSError> {
    let s = match get_session_authorized(jar, &state) {
        Some(s) => s,
        None => return Err(WMSError{ status: rocket::http::Status::Forbidden }),
    };
    if session::str_to_uuid(session_id.as_str()).is_err() {
        return Err(WMSError{ status: rocket::http::Status::NotFound });
    }
    let jid = match session::str_to_uuid(job_id.as_str()) {
        Ok(jid) => jid,
        Err(_) => return Err(WMSError{ status: rocket::http::Status::NotFound }),
    };

    let mut path = s.job_dir(&jid).unwrap();
    path.push(file_name);
    match rocket::fs::NamedFile::open(path).await {
        Ok(f) => Ok(f),
        Err(_) => Err(WMSError{ status: rocket::http::Status::NotFound } ),
    }
}

#[rocket::post("/api", format = "application/json", data = "<req>")]
fn api(req: srvapi::ApiRequest, jar: &rocket::http::CookieJar<'_>, state: &rocket::State<AppState>) -> Result<srvapi::ApiResponse, WMSError> {
    let s = match get_session_authorized(jar, &state) {
        Some(s) => s,
        None => return Err(WMSError{ status: rocket::http::Status::Forbidden }),
    };

    match req {
        srvapi::ApiRequest::StartJob(v) => Ok(request_handlers::start_job(s, v.data)),
        srvapi::ApiRequest::StopJob(v) => Ok(request_handlers::stop_job(s, v.data)),
        srvapi::ApiRequest::CreateJob(v) => Ok(request_handlers::create_job(s, v.data)),
        srvapi::ApiRequest::DeleteJob(v) => Ok(request_handlers::delete_job(s, v.data)),
        srvapi::ApiRequest::CloneJob(v) => Ok(request_handlers::clone_job(s, v.data)),
        srvapi::ApiRequest::JobStatus(v) => Ok(request_handlers::job_status(s, v.data)),
        srvapi::ApiRequest::ListJobs(_) => Ok(request_handlers::list_jobs(s)),
        srvapi::ApiRequest::MmbOutput(v) => Ok(request_handlers::mmb_output(s, v.data)),
        srvapi::ApiRequest::JobCommands(v) => Ok(request_handlers::job_commands(s, v.data)),
        srvapi::ApiRequest::SessionInfo(_) => Ok(request_handlers::session_info(s)),
        srvapi::ApiRequest::ListExamples(_) => Ok(request_handlers::list_examples(state.examples_dir.clone())),
        srvapi::ApiRequest::ActivateExample(v) => Ok(request_handlers::activate_example(s, v.data, state.examples_dir.clone())),
        srvapi::ApiRequest::FileOperation(v) => Ok(request_handlers::file_operation(s, v.data)),
        srvapi::ApiRequest::ListAdditionalFiles(v) => Ok(request_handlers::list_additional_files(s, v.data)),
    }
}

#[get("/density/<session_id>/<job_id>", rank = 1)]
fn density(session_id: String, job_id: String, jar: &rocket::http::CookieJar<'_>, state: &rocket::State<AppState>) -> Result<DensityFile, WMSError> {
    let s = match get_session_authorized(jar, &state) {
        Some(s) => s,
        None => return Err(WMSError{ status: rocket::http::Status::Forbidden }),
    };
    if session::str_to_uuid(session_id.as_str()).is_err() {
        return Err(WMSError{ status: rocket::http::Status::NotFound });
    }
    let jid = match session::str_to_uuid(job_id.as_str()) {
        Ok(jid) => jid,
        Err(_) => return Err(WMSError{ status: rocket::http::Status::NotFound }),
    };

    match s.job_density_map_file_name(&jid) {
        Some(name) => {
            let mut path = s.job_dir(&jid).unwrap();
            path.push(name);

            Ok(DensityFile{ path })
        },
        None => Err(WMSError{ status: rocket::http::Status::NotFound }),
    }
}

#[post("/xfr", format = "application/octet-stream", data = "<req>")]
fn xfr(req: srvapi::FileTransferChunk, jar: &rocket::http::CookieJar<'_>, state: &rocket::State<AppState>) -> Result<srvapi::ApiResponse, WMSError> {
    let s = match get_session_authorized(jar, &state) {
        Some(s) => s,
        None => return Err(WMSError{status: rocket::http::Status::Forbidden}),
    };

    Ok(transfer_handlers::chunk(s, req))
}

#[get("/structure/<session_id>/<job_id>/<stage>", rank = 1)]
fn structure(session_id: String, job_id: String, stage: String, state: &rocket::State<AppState>) -> Result<PdbFile, WMSError> {
    let sid = match session::str_to_uuid(session_id.as_str()) {
        Ok(sid) => sid,
        Err(_) => return Err(WMSError{ status: rocket::http::Status::NotFound }),
    };
    let jid = match session::str_to_uuid(job_id.as_str()) {
        Ok(jid) => jid,
        Err(_) => return Err(WMSError{ status: rocket::http::Status::NotFound }),
    };

    if stage.to_lowercase() == "last" {
        match state.sm.read().unwrap().get_session(&sid) {
            Some(session) => {
                let stage_num = session.job_last_available_stage(&jid);
                match stage_num {
                    Some(stage_num) => {
                        match session::trajectory_file_path(&state.jobs_dir, session_id.as_str(), job_id.as_str(), if stage_num == 0 { 1 } else { stage_num }) {
                            Ok(path) => return Ok(PdbFile{ path }),
                            Err(_) => return Err(WMSError{ status: rocket::http::Status::NotFound }),
                        }
                    },
                    None => return Err(WMSError{ status: rocket::http::Status::NotFound }),
                }
            },
            None => return Err(WMSError{ status: rocket::http::Status::NotFound }),
        }
    }

    let stage_num = stage.parse::<i32>();
    if stage_num.is_err() {
        return Err(WMSError{ status: rocket::http::Status::BadRequest });
    }

    match session::trajectory_file_path(&state.jobs_dir, session_id.as_str(), job_id.as_str(), stage_num.unwrap()) {
        Ok(path) => Ok(PdbFile{ path }),
        Err(_) => Err(WMSError{ status: rocket::http::Status::NotFound }),
    }
}

pub fn start() -> rocket::Rocket<rocket::Build> {
    let cfg = config::get();

    let mut srv_cfg = rocket::config::Config::default();
    srv_cfg.port = cfg.port;
    srv_cfg.cli_colors = false;
    srv_cfg.log_level = match cfg.verbose_rocket_logging {
        true => rocket::config::LogLevel::Normal,
        false => rocket::config::LogLevel::Critical,
    };

    rocket::custom(srv_cfg)
        .mount("/",
               routes![
                   index,
                   index_authorized,
                   auth_page,
                   auth_already_authenticated,
                   auth_verify,
                   static_files,
                   api,
                   density,
                   structure,
                   xfr,
                   additional_file,
               ]
        )
        .manage(AppState{
            sm: RwLock::new(SessionManager::create()),
            jobs_dir: PathBuf::from(cfg.jobs_dir.as_str()),
            examples_dir: PathBuf::from(cfg.examples_dir.as_str()),
            domain: cfg.domain.clone(),
            require_https: cfg.require_https,
        })
}
