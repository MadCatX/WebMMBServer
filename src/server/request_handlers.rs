use std::{path::PathBuf, str::FromStr};
use std::sync::Arc;
use rocket::http::Status;
use serde_json;
use uuid::Uuid;

use crate::{mmb, session::uuid_to_str};
use crate::session;
use crate::session::JobError;
use crate::session::session::Session;
use crate::server::api;
use crate::server::api::ApiResponse;

const EMPTY: api::Empty = api::Empty{};
const INTR_SERV_ERR: &'static str = "Internal server error";
const NO_CMDS: &'static str = "No commands";

fn job_info_to_api(id: &Uuid, info: session::job::JobInfo) -> api::JobInfo {
    api::JobInfo{
        id: session::uuid_to_str(id),
        name: info.name,
        state: mmb_state_to_job_state(info.state),
        first_stage: info.first_stage,
        last_stage: info.last_stage,
        created_on: info.created_on.to_string(),
        commands_mode: info.commands_mode,
        progress: match info.progress {
            Some(progress) => {
                Some(api::JobProgress{
                    step: step_to_str(progress.step),
                    total_steps: progress.total_steps,
                })
            },
            None => None,
        }
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
        mmb::State::Queued => api::JobState::Queued,
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

    match session.create_job(parsed.id, Some(cmds_json), None) {
        Ok(id) => {
            let resp = api::JobCreated{id: session::uuid_to_str(&id)};
            ApiResponse::ok(serde_json::to_value(resp).unwrap())
        },
        Err(e) => match e {
            JobError::BadInput(msg) => ApiResponse::fail(Status::BadRequest, msg),
            JobError::InternalError => ApiResponse::fail(Status::InternalServerError, String::from(INTR_SERV_ERR)),
        },
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

    if session.has_job_by_name(&parsed.name) {
        return ApiResponse::fail(Status::BadRequest, format!("Job named {} already exists", parsed.name));
    }

    if parsed.name.len() < 1 {
        return ApiResponse::fail(Status::BadRequest, String::from("Job must have a name"));
    }

    match session.clone_job(parsed.name, &src_id) {
        Ok(id) => {
            let resp = api::JobCreated{id: session::uuid_to_str(&id)};
            ApiResponse::ok(serde_json::to_value(resp).unwrap())
        },
        Err(e) => match e {
            JobError::BadInput(msg) => ApiResponse::fail(Status::BadRequest, msg),
            JobError::InternalError => ApiResponse::fail(Status::InternalServerError, String::from(INTR_SERV_ERR)),
        }
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

    if parsed.name.len() < 1 {
        return ApiResponse::fail(Status::BadRequest, String::from("Job must have a name"));
    }

    match session.create_job(parsed.name, None, None) {
        Ok(id) => {
            let resp = api::JobCreated{id: session::uuid_to_str(&id)};
            ApiResponse::ok(serde_json::to_value(resp).unwrap())
        },
        Err(e) => match e {
            JobError::BadInput(msg) => ApiResponse::fail(Status::BadRequest, msg),
            JobError::InternalError => ApiResponse::fail(Status::InternalServerError, String::from(INTR_SERV_ERR)),
        },
    }
}

pub fn delete_job(session: Arc<Session>, data: serde_json::Value) -> ApiResponse {
    let id = match handle_simple_rq_data(data) {
        Ok(id) => id,
        Err(e) => return ApiResponse::fail(Status::BadRequest,e),
    };

    match session.delete_job(&id) {
        true => ApiResponse::ok(serde_json::to_value(EMPTY).unwrap()),
        false => ApiResponse::fail(Status::BadRequest, String::from("No job to delete")),
    }
}

pub fn file_operation(session: Arc<Session>, data: serde_json::Value) -> ApiResponse {
    let parsed: serde_json::Result<api::FileOperationRqData> = serde_json::from_value(data);

    match parsed {
        Ok(data) => {
            let job_id = match Uuid::from_str(data.job_id.as_str()) {
                Ok(id) => id,
                Err(e) => return ApiResponse::fail(Status::BadRequest, e.to_string()),
            };

            match data.req_type {
                api::FileOperationRequestType::InitUpload => {
                    if data.file_name.len() < 1 {
                        return ApiResponse::fail(Status::BadRequest, String::from("No file name"));
                    }
                    if data.file_name.contains("/") || data.file_name.contains("\\") {
                        return ApiResponse::fail(Status::BadRequest, String::from("Invalid file name"));
                    }
                    match session.init_upload(&job_id, data.file_name) {
                        Ok(id) => {
                            let resp = api::FileTransferAck{id: uuid_to_str(&id)};
                            match serde_json::to_value(resp) {
                                Ok(v) => return ApiResponse::ok(v),
                                Err(_) => return ApiResponse::fail(Status::InternalServerError, String::from("Cannot convert UUID to string")),
                            }
                        },
                        Err(e) => return ApiResponse::fail(Status::BadRequest, e)
                    }
                },
                api::FileOperationRequestType::FinishUpload => {
                    let transfer_id = match Uuid::from_str(data.transfer_id.as_str()) {
                        Ok(id) => id,
                        Err(e) => return ApiResponse::fail(Status::BadRequest, e.to_string()),
                    };

                    match session.finish_upload(job_id, transfer_id) {
                        Ok(()) => return ApiResponse::ok(serde_json::to_value(EMPTY).unwrap()),
                        Err(e) => return ApiResponse::fail(Status::BadRequest, e),
                    }
                },
                api::FileOperationRequestType::Delete => {
                    match session.delete_additional_file(&job_id, data.file_name) {
                        Ok(()) => return ApiResponse::ok(serde_json::to_value(EMPTY).unwrap()),
                        Err(e) => return ApiResponse::fail(Status::BadRequest, e),
                    }
                },
                api::FileOperationRequestType::CancelUpload => {
                    let transfer_id = match Uuid::from_str(data.transfer_id.as_str()) {
                        Ok(id) => id,
                        Err(e) => return ApiResponse::fail(Status::BadRequest, e.to_string()),
                    };

                    match session.cancel_upload(&job_id, &transfer_id) {
                        Ok(()) => return ApiResponse::ok(serde_json::to_value(EMPTY).unwrap()),
                        Err(e) => return ApiResponse::fail(Status::BadRequest, e),
                    }
                },
            }
        },
        Err(_) => ApiResponse::fail(Status::BadRequest, String::from("Invalid upload file request")),
    }
}

pub fn list_additional_files(session: Arc<Session>, data: serde_json::Value) -> ApiResponse {
    let id = match handle_simple_rq_data(data) {
        Ok(id) => id,
        Err(e) => return ApiResponse::fail(Status::BadRequest, e),
    };

    match session.list_job_additional_files(&id) {
        Ok(l) => {
            let mut list = Vec::<api::AdditionalFile>::new();
            for file in l {
                list.push(
                    api::AdditionalFile{
                        name: file.name,
                        size: file.size.to_string(),
                    }
                );
            }

            ApiResponse::ok(serde_json::to_value(list).unwrap())
        }
        Err(e) => ApiResponse::fail(Status::BadRequest, e),
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

    let mut jobs: api::JobList = Vec::new();
    for (id, info) in list {
        match info {
            Ok(info) => jobs.push(job_info_to_api(&id, info)),
            Err(e) => return ApiResponse::fail(Status::InternalServerError, e),
        };
    }

    ApiResponse::ok(serde_json::to_value(jobs).unwrap())
}

pub fn job_commands(session: Arc<Session>, data: serde_json::Value) -> ApiResponse {
    let id = match handle_simple_rq_data(data) {
        Ok(id) => id,
        Err(e) => return ApiResponse::fail(Status::BadRequest, e),
    };

    let mode = match session.job_commands_mode(id) {
        Some(mode) => mode,
        None => return ApiResponse::fail(Status::BadRequest, String::from("Unknown job id")),
    };

    match mode {
        api::JobCommandsMode::None => {
            let resp = api::JobCommands::None(api::JobCommandsNone{});
            ApiResponse::ok(serde_json::to_value(resp).unwrap())
        }
        api::JobCommandsMode::Synthetic => {
            match session.job_commands(id) {
                Ok(commands) => match commands {
                    Some(commands) => {
                        let resp = api::JobCommands::Synthetic(api::JobCommandsSynthetic{ commands });
                        ApiResponse::ok(serde_json::to_value(resp).unwrap())
                    },
                    None => ApiResponse::fail(Status::InternalServerError, String::from(NO_CMDS)),
                },
                Err(e) => ApiResponse::fail(Status::InternalServerError, e),
            }
        },
        api::JobCommandsMode::Raw => {
            match session.job_commands_raw(id) {
                Ok(commands) => match commands {
                    Some(commands) => {
                        let resp = api::JobCommands::Raw(api::JobCommandsRaw{ commands });
                        ApiResponse::ok(serde_json::to_value(resp).unwrap())
                    },
                    None => ApiResponse::fail(Status::InternalServerError, String::from(NO_CMDS)),
                },
                Err(e) => ApiResponse::fail(Status::InternalServerError, e),
            }
        },
    }
}

pub fn job_status(session: Arc<Session>, data: serde_json::Value) -> ApiResponse {
    let id = match handle_simple_rq_data(data) {
        Ok(id) => id,
        Err(e) => return ApiResponse::fail(Status::BadRequest, e),
    };

    match session.job_info(id) {
        Ok(info) => {
            let resp = job_info_to_api(&id, info);
            ApiResponse::ok(serde_json::to_value(resp).unwrap())
        },
        Err(e) => ApiResponse::fail(Status::BadRequest, e),
    }
}

pub fn mmb_output(session: Arc<Session>, data: serde_json::Value) -> ApiResponse {
    let id = match handle_simple_rq_data(data) {
        Ok(id) => id,
        Err(e) => return ApiResponse::fail(Status::BadRequest, e),
    };

    match session.job_diagnostics(&id) {
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
    let parsed: serde_json::Result<api::StartJobRqData> = serde_json::from_value(data);
    let start_data = match parsed {
        Ok(data) => data,
        Err(_) => return ApiResponse::fail(Status::BadRequest, String::from("Invalid start job request")),
    };

    let id = match Uuid::parse_str(&start_data.id) {
        Ok(v) => v,
        Err(_) => return ApiResponse::fail(Status::BadRequest, String::from("Malformed job id")),
    };

    match start_data.commands {
        api::JobCommandsNotNone::Synthetic(commands) => {
            match session.start_job(&id, commands.commands) {
                Ok(()) => ApiResponse::ok(serde_json::to_value(EMPTY).unwrap()),
                Err(e) => match e {
                    JobError::BadInput(msg) => ApiResponse::fail(Status::BadRequest, msg),
                    JobError::InternalError => ApiResponse::fail(Status::InternalServerError, String::from(INTR_SERV_ERR)),
                },
            }
        },
        api::JobCommandsNotNone::Raw(commands) => {
            match session.start_job_raw(&id, commands.commands) {
                Ok(()) => ApiResponse::ok(serde_json::to_value(EMPTY).unwrap()),
                Err(e) => match e {
                    JobError::BadInput(msg) => ApiResponse::fail(Status::BadRequest, msg),
                    JobError::InternalError => ApiResponse::fail(Status::InternalServerError, String::from(INTR_SERV_ERR)),
                },
            }
        },
    }
}

pub fn stop_job(session: Arc<Session>, data: serde_json::Value) -> ApiResponse {
    let id = match handle_simple_rq_data(data) {
        Ok(id) => id,
        Err(e) => return ApiResponse::fail(Status::BadRequest, e),
    };

    match session.stop_job(id) {
        Ok(_) => ApiResponse::ok(serde_json::to_value(EMPTY).unwrap()),
        Err(e) => ApiResponse::fail(Status::BadRequest, e),
    }
}
