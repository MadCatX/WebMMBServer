use std::{str::FromStr, sync::Arc};
use rocket::http::Status;
use serde_json;
use uuid::Uuid;

use crate::server::api;
use crate::session;
use crate::session::session::Session;

pub fn chunk(s: Arc<Session>, chunk: api::TransferChunk) -> api::ApiResponse {
    let job_id = match Uuid::from_str(chunk.job_id.as_str()) {
        Ok(id) => id,
        Err(e) => return api::ApiResponse::fail(Status::BadRequest, e.to_string()),
    };

    let transfer_id = match Uuid::from_str(chunk.transfer_id.as_str()) {
        Ok(id) => id,
        Err(e) => return api::ApiResponse::fail(Status::BadRequest, e.to_string()),
    };

    match s.upload_chunk(&job_id, &transfer_id, chunk.challenge, chunk.data) {
        Ok(challenge) => {
            let chal_bytes = challenge.to_le_bytes();
            let resp = api::FileTranferAck{id: session::uuid_to_str(&transfer_id), challenge: chal_bytes};
            api::ApiResponse::ok(serde_json::to_value(resp).unwrap())
        },
        Err(e) => api::ApiResponse::fail(Status::BadRequest, e),
    }
}
