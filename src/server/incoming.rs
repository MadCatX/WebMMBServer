use std::net::IpAddr;
use rocket::Data;
use rocket::data::{FromData, Outcome, ToByteUnit};
use rocket::http::Status;
use rocket::request::Request;

use crate::server::{api, LOGSRC};
use crate::logging;

const MAX_JSON_SIZE: usize = 4 * 1024 * 1024;
const MAX_CHUNK_SIZE: usize = 8 * 1024 * 1024;

pub struct IncomingAuthRequest {
    pub payload: api::AuthRequest,
    pub remote_addr: Option<IpAddr>,
}

pub struct IncomingApiRequest {
    pub payload: api::ApiRequest,
    pub remote_addr: Option<IpAddr>,
}

#[rocket::async_trait]
impl<'a> FromData<'a> for IncomingApiRequest {
    type Error = String;

    async fn from_data(req: &'a Request<'_>, data: Data<'a>) -> Outcome<'a, Self> {
        let remote_addr = req.client_ip();

        let stream = data.open(ToByteUnit::bytes(MAX_JSON_SIZE));
        match stream.into_string().await {
            Ok(payload) => {
                match serde_json::from_str::<api::ApiRequest>(&payload) {
                    Ok(apiReq) => {
                        Outcome::Success(IncomingApiRequest{ payload: apiReq, remote_addr })
                    },
                    Err(e) => {
                        logging::incoming(logging::Priority::Warning, LOGSRC, remote_addr, &format!("Malformed api request: {}", e.to_string()));
                        Outcome::Failure((Status::BadRequest, String::from("Malformed request")))
                    }
                }
            },
            Err(e) => {
                logging::incoming(logging::Priority::Warning, LOGSRC, remote_addr, &format!("Cannot get api request message body: {}", e.to_string()));
                Outcome::Failure((Status::InternalServerError, String::from("Cannot process api request")))
            },
        }
    }
}

#[rocket::async_trait]
impl<'a> FromData<'a> for IncomingAuthRequest {
    type Error = String;

    async fn from_data(req: &'a Request<'_>, data: Data<'a>) -> Outcome<'a, Self> {
        let remote_addr = req.client_ip();

        let stream = data.open(ToByteUnit::bytes(MAX_JSON_SIZE));
        match stream.into_string().await {
            Ok(payload) => {
                match serde_json::from_str::<api::AuthRequest>(&payload) {
                    Ok(auth) => Outcome::Success(IncomingAuthRequest{ payload: auth, remote_addr }),
                    Err(e) => {
                        logging::incoming(logging::Priority::Warning, LOGSRC, remote_addr, &format!("Malformed authentication request: {}", e.to_string()));
                        Outcome::Failure((Status::BadRequest, String::from("Malformed request")))
                    }
                }
            },
            Err(e) => {
                logging::incoming(logging::Priority::Warning, LOGSRC, remote_addr, &format!("Cannot get authentication request message body: {}", e.to_string()));
                Outcome::Failure((Status::InternalServerError, String::from("Cannot process authentication request")))
            },
        }
    }
}

#[rocket::async_trait]
impl<'a> FromData<'a> for api::FileTransferChunk {
    type Error = String;

    async fn from_data(req: &'a Request<'_>, data: Data<'a>) -> Outcome<'a, Self> {
        let remote_addr = req.client_ip();

        let stream = data.open(ToByteUnit::bytes(MAX_CHUNK_SIZE));
        match stream.into_bytes().await {
            Ok(payload) => {
                if !payload.is_complete() {
                    logging::incoming(logging::Priority::Warning, LOGSRC, remote_addr, &format!("FileTransferChunk payload was capped prematurely at {} bytes", payload.n));
                    return Outcome::Failure((Status::BadRequest, String::from("Too long request")));
                } else {
                    match api::FileTransferChunk::from_bytes(&payload.value) {
                        Ok(chunk) => Outcome::Success(chunk),
                        Err(e) => {
                            logging::incoming(logging::Priority::Warning, LOGSRC, remote_addr, &format!("Malformed payload for FileTransferChunk: {}", e.to_string()));
                            Outcome::Failure((Status::BadRequest, String::from("Malformed file transfer chunk request")))
                        },
                    }
                }
            },
            Err(e) => {
                logging::incoming(logging::Priority::Warning, LOGSRC, remote_addr, &format!("Cannot get file transfer chunk request body: {}", e.to_string()));
                Outcome::Failure((Status::InternalServerError, String::from("Cannot process file transfer request")))
            },
        }
    }
}
