use std::sync::Arc;
use rocket::http::Status;
use serde_json;
use uuid::Uuid;

use crate::session;
use crate::session::session::Session;
use crate::session::job;
use crate::server::api;
use crate::server::responders::ApiResponse;

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

fn job_state_to_str(st: job::State) -> String {
    match st {
        job::State::NotStarted => String::from("not_started"),
        job::State::Running => String::from("running"),
        job::State::Finished => String::from("finished"),
        job::State::Failed => String::from("failed"),
    }
}

fn step_to_str(step: i32) -> String {
    if step == 0 {
        return String::from("preparing");
    }
    step.to_string()
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

pub fn list_jobs(session: Arc<Session>) -> ApiResponse {
    let list = session.list_jobs();

    let mut jobs: Vec<api::JobInfo> = Vec::new();
    for (id, info) in list {
        jobs.push(api::JobInfo{
            id: session::uuid_to_str(&id),
            name: info.name,
            status: job_state_to_str(info.state),
            step: step_to_str(info.step),
            total_steps: info.total_steps,
            last_completed_stage: info.last_completed_stage,
        });
    }

    ApiResponse::ok(serde_json::to_value(jobs).unwrap())
}

pub fn job_commands(session: Arc<Session>, data: serde_json::Value) -> ApiResponse {
    let id = match handle_simple_rq_data(data) {
        Ok(id) => id,
        Err(e) => return ApiResponse::fail(Status::BadRequest, e),
    };

    match session.job_commands(id) {
        Some(commands) => ApiResponse::ok(serde_json::to_value(commands).unwrap()),
        None => ApiResponse::fail(Status::BadRequest, String::from("Unknown job id")),
    }
}

pub fn job_status(session: Arc<Session>, data: serde_json::Value) -> ApiResponse {
    let id = match handle_simple_rq_data(data) {
        Ok(id) => id,
        Err(e) => return ApiResponse::fail(Status::BadRequest, e),
    };   

    match session.job_info(id) {
        Some(info) => {
            let resp = api::JobInfo{
                id: session::uuid_to_str(&id),
                name: info.name,
                status: job_state_to_str(info.state),
                step: step_to_str(info.step),
                total_steps: info.total_steps,
                last_completed_stage: info.last_completed_stage,
            };

            ApiResponse::ok(serde_json::to_value(resp).unwrap())
        },
        None => ApiResponse::fail(Status::BadRequest,String::from("Unknown job id")),
    }
}

pub fn resume_job(session: Arc<Session>, data: serde_json::Value) -> ApiResponse {
    println!("resume_job handler");
    let parsed: serde_json::Result<api::ResumeJobRqData> = serde_json::from_value(data);
    if parsed.is_err() {
        return ApiResponse::fail(Status::BadRequest, String::from("Invalid start job request"));
    }

    let res_data = parsed.unwrap();

    let id = match Uuid::parse_str(res_data.id.as_str()) {
        Ok(id) => id,
        Err(_) => return ApiResponse::fail(Status::BadRequest, String::from("Invalid job ID")),
    };

    if !session.has_job(&id) {
        return ApiResponse::fail(Status::BadRequest, String::from("No job to continue"));
    }

    match session.resume_job(&id, res_data.commands) {
        Ok(info) => { 
            let data = api::JobInfo{
                id: session::uuid_to_str(&id),
                name: info.name,
                status: job_state_to_str(job::State::Running),
                step: step_to_str(0),
                total_steps: info.total_steps,
                last_completed_stage: info.last_completed_stage,
            };
            ApiResponse::ok(serde_json::to_value(data).unwrap())
        },
        Err(e) => ApiResponse::fail(Status::BadRequest, e),
    }
}

pub fn session_info(session: Arc<Session>) -> ApiResponse {
    let username = session.username();

    let info = api::SessionInfo{ username };
    ApiResponse::ok(serde_json::to_value(info).unwrap())
}

pub fn start_job(session: Arc<Session>, data: serde_json::Value) -> ApiResponse {
    println!("start_job hander: {}", data);
    let parsed: serde_json::Result<api::StartJobRqData> = serde_json::from_value(data);
    if parsed.is_err() {
        return ApiResponse::fail(Status::BadRequest, String::from("Invalid start job request"));
    }

    let start_data = parsed.unwrap();
    match session.add_job(start_data.name, start_data.commands) {
        Ok((id, info)) => { 
            let data = api::JobInfo{
                id: session::uuid_to_str(&id),
                name: info.name,
                status: job_state_to_str(job::State::Running),
                step: step_to_str(0),
                total_steps: info.total_steps,
                last_completed_stage: info.last_completed_stage,
            };
            ApiResponse::ok(serde_json::to_value(data).unwrap())
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
            let info = session.job_info(id).unwrap();
            let out = api::JobInfo{
                id: session::uuid_to_str(&id),
                name: info.name,
                status: job_state_to_str(info.state),
                step: step_to_str(0), // FIXME
                total_steps: 0, // FIXME
                last_completed_stage: info.last_completed_stage,
            };
            ApiResponse::ok(serde_json::to_value(out).unwrap())
        },
        Err(e) => ApiResponse::fail(Status::BadRequest, e),
    }
}
