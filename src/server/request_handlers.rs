use std::path::PathBuf;
use std::sync::Arc;
use rocket::http::Status;
use serde_json;
use uuid::Uuid;

use crate::mmb;
use crate::session;
use crate::session::session::Session;
use crate::server::api;
use crate::server::api::ApiResponse;

fn empty_job_info() -> api::JobInfo {
    api::JobInfo{
        id: String::new(),
        name: String::new(),
        state: api::JobState::NotStarted,
        step: step_to_str(0),
        total_steps: 0,
        available_stages: Vec::new(),
        current_stage: None,
        created_on: 0.to_string(),
        commands_mode: api::JobCommandsMode::Synthetic,
    }
}

fn job_commands_pruned(mut commands: api::JsonCommands) -> api::JsonCommands {
    commands.extra_files.clear();
    commands
}

fn job_info_to_api(id: &Uuid, info: session::job::JobInfo) -> api::JobInfo {
    api::JobInfo{
        id: session::uuid_to_str(id),
        name: info.name,
        state: mmb_state_to_job_state(info.state),
        step: step_to_str(info.step),
        total_steps: info.total_steps,
        available_stages: info.available_stages,
        current_stage: info.current_stage,
        created_on: info.created_on.to_string(),
        commands_mode: match info.commands_mode {
            session::job::CommandsMode::Synthetic => api::JobCommandsMode::Synthetic,
            session::job::CommandsMode::Raw => api::JobCommandsMode::Raw,
            session::job::CommandsMode::None => api::JobCommandsMode::None,
        },
    }
}

fn handle_simple_rq_data(data: serde_json::Value) -> Result<Uuid, String> {
    let parsed: serde_json::Result<api::SimpleJobRqData> = serde_json::from_value(data);
    if parsed.is_err() {
        return Err(String::from("Malformed data"));
    }

    let req = parsed.unwrap();
    match Uuid::parse_str(&req.id) {
        Ok(id) => Ok(id),
        Err(_) => Err(String::from("Malformed job id")),
    }
}

fn mmb_state_to_job_state(s: mmb::State) -> api::JobState {
    match s {
        mmb::State::NotStarted => api::JobState::NotStarted,
        mmb::State::Running => api::JobState::Running,
        mmb::State::Finished => api::JobState::Finished,
        mmb::State::Failed => api::JobState::Failed,
        mmb::State::Unknown => panic!("mmb::State::Unknown shall never be returned"),
    }
}

fn step_to_str(step: i32) -> String {
    if step == 0 {
        return String::from("preparing");
    }
    step.to_string()
}

pub fn activate_example(session: Arc<Session>, data: serde_json::Value, path: PathBuf) -> ApiResponse {
    let parsed = match serde_json::from_value::<api::SimpleJobRqData>(data) {
        Ok(v) => v,
        Err(e) => return ApiResponse::fail(Status::BadRequest, e.to_string()),
    };

    let example_cmds = match mmb::examples::example_data(path, &parsed.id) {
        Ok(v) => v,
        Err(e) => return ApiResponse::fail(Status::BadRequest, e.to_string()),
    };

    let cmds_json = match serde_json::from_str(example_cmds.as_str()) {
        Ok(v) => v,
        Err(e) => return ApiResponse::fail(Status::InternalServerError, e.to_string()),
    };

    let id = match session.create_job(parsed.id, Some(cmds_json), None) {
        Ok(v) => v,
        Err(e) => return ApiResponse::fail(Status::InternalServerError, e),
    };

    match session.job_info(id) {
        Some(v) => match v {
            Ok(info) => {
                let resp = job_info_to_api(&id, info);
                ApiResponse::ok(serde_json::to_value(resp).unwrap())
            },
            Err(e) => ApiResponse::fail(Status::InternalServerError, e),
        },
        None => ApiResponse::fail(Status::InternalServerError, format!("Job id {} is unknown", id)),
    }
}

pub fn clone_job(session: Arc<Session>, data: serde_json::Value) -> ApiResponse {
    let parsed = match serde_json::from_value::<api::CloneJobRqData>(data) {
        Ok(v) => v,
        Err(e) => return ApiResponse::fail(Status::BadRequest, e.to_string()),
    };
    let src_id = match Uuid::parse_str(&parsed.id) {
        Ok(v) => v,
        Err(e) => return ApiResponse::fail(Status::BadRequest, e.to_string()),
    };

    let id = match session.clone_job(parsed.name, &src_id) {
        Ok(v) => v,
        Err(e) => return ApiResponse::fail(Status::InternalServerError, e),
    };

    match session.job_info(id) {
        Some(v) => match v {
            Ok(info) => {
                let resp = job_info_to_api(&id, info);
                ApiResponse::ok(serde_json::to_value(resp).unwrap())
            },
            Err(e) => ApiResponse::fail(Status::InternalServerError, e),
        },
        None => ApiResponse::fail(Status::InternalServerError, format!("Job id {} is unknown", id)),
    }
}

pub fn create_job(session: Arc<Session>, data: serde_json::Value) -> ApiResponse {
    let parsed = match serde_json::from_value::<api::CreateJobRqData>(data) {
        Ok(v) => v,
        Err(e) => return ApiResponse::fail(Status::BadRequest, e.to_string()),
    };

    if session.has_job_by_name(&parsed.name) {
        return ApiResponse::fail(Status::BadRequest, format!("Job named {} already exists", parsed.name));
    }

    match session.create_job(parsed.name, None, None) {
        Ok(id) => {
            match session.job_info(id) {
                Some(v) => match v {
                    Ok(info) => {
                        let resp = job_info_to_api(&id, info);
                        return ApiResponse::ok(serde_json::to_value(resp).unwrap());
                    },
                    Err(e) => return ApiResponse::fail(Status::InternalServerError, e),
                },
                None => ApiResponse::fail(Status::InternalServerError, format!("Job id {} is unknown", id)),
            }
        },
        Err(e) => ApiResponse::fail(Status::InternalServerError, e),
    }
}

pub fn delete_job(session: Arc<Session>, data: serde_json::Value) -> ApiResponse {
    let id = match handle_simple_rq_data(data) {
        Ok(id) => id,
        Err(e) => return ApiResponse::fail(Status::BadRequest,e),
    };

    if session.delete_job(&id) {
        let resp = api::Empty{};
        return ApiResponse::ok(serde_json::to_value(resp).unwrap());
    } else {
        return ApiResponse::fail(Status::BadRequest, String::from("No job to delete"));
    }
}

pub fn list_examples(path: PathBuf) -> ApiResponse {
    let list = match mmb::examples::get_examples(path) {
        Ok(v) => v,
        Err(e) => return ApiResponse::fail(Status::InternalServerError, e),
    };

    let resp_list: api::ExampleList = list.into_iter().map(|item| api::ExampleListItem{ name: item.name, description: item.description }).collect();

    ApiResponse::ok(serde_json::to_value(resp_list).unwrap())
}

pub fn list_jobs(session: Arc<Session>) -> ApiResponse {
    let list = session.list_jobs();

    let mut jobs: Vec<api::JobListItem> = Vec::new();
    for (id, info) in list {
        match info {
            Ok(info) => {
                let item = api::JobListItem{
                    ok: true,
                    info: job_info_to_api(&id, info),
                };
                jobs.push(item);
            },
            Err(_) => {
                let item = api::JobListItem{
                    ok: false,
                    info: empty_job_info(),
                };
                jobs.push(item);
            },
        };
    }

    ApiResponse::ok(serde_json::to_value(jobs).unwrap())
}

pub fn job_commands(session: Arc<Session>, data: serde_json::Value) -> ApiResponse {
    let id = match handle_simple_rq_data(data) {
        Ok(id) => id,
        Err(e) => return ApiResponse::fail(Status::BadRequest, e),
    };

    match session.job_commands(id) {
        Ok(v) => {
            match v {
                Some(commands) => {
                    let pruned = job_commands_pruned(commands);
                    let resp = api::JobCommands{is_empty: false, commands: Some(pruned)};
                    ApiResponse::ok(serde_json::to_value(resp).unwrap())
                },
                None => {
                    let resp = api::JobCommands{is_empty: true, commands: None};
                    ApiResponse::ok(serde_json::to_value(resp).unwrap())
                }
            }
        },
        Err(e) => ApiResponse::fail(Status::BadRequest, e),
    }
}

pub fn job_commands_raw(session: Arc<Session>, data: serde_json::Value) -> ApiResponse {
    let id = match handle_simple_rq_data(data) {
        Ok(id) => id,
        Err(e) => return ApiResponse::fail(Status::BadRequest, e),
    };

    match session.job_commands_raw(id) {
        Ok(v) => {
            match v {
                Some(commands) => {
                    let resp = api::JobCommandsRaw{is_empty: false, commands: Some(commands)};
                    ApiResponse::ok(serde_json::to_value(resp).unwrap())
                },
                None => {
                    let resp = api::JobCommandsRaw{is_empty: true, commands: None};
                    ApiResponse::ok(serde_json::to_value(resp).unwrap())
                }
            }
        },
        Err(e) => ApiResponse::fail(Status::BadRequest, e),
    }
}

pub fn job_status(session: Arc<Session>, data: serde_json::Value) -> ApiResponse {
    let id = match handle_simple_rq_data(data) {
        Ok(id) => id,
        Err(e) => return ApiResponse::fail(Status::BadRequest, e),
    };

    match session.job_info(id) {
        Some(info) => {
            match info {
                Ok(info) => {
                    let resp = job_info_to_api(&id, info);
                    ApiResponse::ok(serde_json::to_value(resp).unwrap())
                },
                Err(e) => ApiResponse::fail(Status::InternalServerError, e),
            }
        },
        None => ApiResponse::fail(Status::BadRequest, String::from("Unknown job id")),
    }
}

pub fn mmb_output(session: Arc<Session>, data: serde_json::Value) -> ApiResponse {
    let id = match handle_simple_rq_data(data) {
        Ok(id) => id,
        Err(e) => return ApiResponse::fail(Status::BadRequest, e),
    };

    match session.job_stdout(&id) {
        Some(ret) => match ret {
            Ok(txt) => ApiResponse::ok(serde_json::to_value(txt).unwrap()),
            Err(e) => ApiResponse::fail(Status::InternalServerError, e),
        },
        None => ApiResponse::fail(Status::BadRequest, String::from("Unknown job id")),
    }
}

pub fn session_info(session: Arc<Session>) -> ApiResponse {
    let id = session::uuid_to_str(&session.id());

    let info = api::SessionInfo{ id };
    ApiResponse::ok(serde_json::to_value(info).unwrap())
}

pub fn start_job(session: Arc<Session>, data: serde_json::Value) -> ApiResponse {
    println!("start_job hander: {}", data);
    let parsed: serde_json::Result<api::StartJobRqData> = serde_json::from_value(data);
    if parsed.is_err() {
        return ApiResponse::fail(Status::BadRequest, String::from("Invalid start job request"));
    }

    let start_data = parsed.unwrap();
    let id = match Uuid::parse_str(&start_data.id) {
        Ok(v) => v,
        Err(_) => return ApiResponse::fail(Status::BadRequest, String::from("Malformed job id")),
    };
    match session.start_job(&id, start_data.commands) {
        Ok((id, info)) => {
            let resp = job_info_to_api(&id, info);
            ApiResponse::ok(serde_json::to_value(resp).unwrap())
        },
        Err(e) => ApiResponse::fail(Status::BadRequest, e),
    }
}

pub fn start_job_raw(session: Arc<Session>, data: serde_json::Value) -> ApiResponse {
    let parsed: serde_json::Result<api::StartJobRawRqData> = serde_json::from_value(data);
    if parsed.is_err() {
        return ApiResponse::fail(Status::BadRequest, String::from("Invalid start raw job request"));
    }

    let start_data_raw = parsed.unwrap();
    let id = match Uuid::parse_str(&start_data_raw.id) {
        Ok(v) => v,
        Err(_) => return ApiResponse::fail(Status::BadRequest, String::from("Malformed job id")),
    };
    match session.start_job_raw(&id, start_data_raw.commands) {
        Ok((id, info)) => {
            let resp = job_info_to_api(&id, info);
            ApiResponse::ok(serde_json::to_value(resp).unwrap())
        },
        Err(e) => ApiResponse::fail(Status::BadRequest, e),
    }
}

pub fn stop_job(session: Arc<Session>, data: serde_json::Value) -> ApiResponse {
    let id = match handle_simple_rq_data(data) {
        Ok(id) => id,
        Err(e) => return ApiResponse::fail(Status::BadRequest, e),
    };

    match session.stop_job(id) {
        Ok(_) => {
            match session.job_info(id) {
                Some(info) => {
                    match info {
                        Ok(info) => {
                            let resp = job_info_to_api(&id, info);
                            ApiResponse::ok(serde_json::to_value(resp).unwrap())
                        },
                        Err(e) => ApiResponse::fail(Status::InternalServerError, e),
                    }
                },
                None => ApiResponse::fail(Status::BadRequest, String::from("Unknown job id")),
            }
        },
        Err(e) => ApiResponse::fail(Status::BadRequest, e),
    }
}

pub fn upload_file(session: Arc<Session>, data: serde_json::Value) -> ApiResponse {
    let parsed: serde_json::Result<api::UploadFileData> = serde_json::from_value(data);
    if parsed.is_err() {
        return ApiResponse::fail(Status::BadRequest, String::from("Invalid upload file request"));
    }

    let data = parsed.unwrap();
    println!("Type: {}\nData: {}\n", data.req_type, data.data);

    let empty = api::Empty{};
    ApiResponse::ok(serde_json::to_value(empty).unwrap())
}

impl std::fmt::Display for api::UploadFileRequestType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        return match self {
            api::UploadFileRequestType::Start => write!(f, "Start"),
            api::UploadFileRequestType::Continue => write!(f, "Continue"),
            api::UploadFileRequestType::Finish => write!(f, "Finish"),
        }
    }
}
