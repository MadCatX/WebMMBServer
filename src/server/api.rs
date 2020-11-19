use rocket::http::Status;
use serde::{Deserialize, Serialize};
use serde_json;

/* Requests */

#[derive(Deserialize)]
pub struct ApiRequestData {
    pub data: serde_json::Value,
}

#[derive(Deserialize)]
#[serde(tag = "req_type")]
pub enum ApiRequest {
    StartJob(ApiRequestData),
    StopJob(ApiRequestData),
    ResumeJob(ApiRequestData),
    DeleteJob(ApiRequestData),
    JobStatus(ApiRequestData),
    ListJobs(ApiRequestData),
    JobCommands(ApiRequestData),
    SessionInfo(ApiRequestData),
}

#[derive(Deserialize)]
pub struct AuthRequestData {
    pub session_id: String,
}

#[derive(Deserialize)]
#[serde(tag = "auth_type")]
pub enum AuthRequest {
    LogIn(AuthRequestData),
    LogOut(AuthRequestData),
}

#[derive(Deserialize)]
pub struct SimpleJobRqData {
    pub id: String,
}

#[derive(Deserialize)]
pub struct ResumeJobRqData {
    pub id: String,
    pub commands: serde_json::Value,
}
#[derive(Deserialize)]
pub struct StartJobRqData {
    pub name: String,
    pub commands: serde_json::Value,
}

/* Responses */

pub struct ApiResponse {
    pub is_ok: bool,
    pub ok_data: Option<serde_json::Value>,
    pub fail_data: Option<(Status, String)>,
}

#[derive(Debug)]
pub struct AuthFailResponse {
    pub status: Status,
    pub reason: String,
}

#[derive(Serialize)]
pub enum JobState {
    NotStarted,
    Running,
    Finished,
    Failed,
}

#[derive(Serialize)]
pub struct ErrorResponse {
    pub success: bool,
    pub message: String,
}

#[derive(Serialize)]
pub struct OkResponse {
    pub success: bool,
    pub data: serde_json::Value,
}

#[derive(Serialize)]
pub struct Empty {
}

#[derive(Serialize)]
pub struct JobInfo {
    pub id: String,
    pub name: String,
    pub state: JobState,
    pub step: String,
    pub total_steps: i32,
    pub last_completed_stage: i32,
}

#[derive(Serialize)]
pub struct JobListItem {
    pub ok: bool,
    pub info: JobInfo,
}

#[derive(Serialize)]
pub struct SessionInfo {
    pub id: String,
}
