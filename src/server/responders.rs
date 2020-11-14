use std::io::Cursor;
use std::path::PathBuf;
use rocket::http::{ContentType, Status};
use rocket::request::Request;
use rocket::response::{self, Response, Responder};
use serde_json;

use crate::server::api;

pub struct ApiResponse {
    is_ok: bool,
    pub ok_data: Option<serde_json::Value>,
    pub fail_data: Option<(Status, String)>,
}

impl ApiResponse {
    pub fn ok(data: serde_json::Value) -> ApiResponse {
        ApiResponse{is_ok: true, ok_data: Some(data), fail_data: None}
    }

    pub fn fail(status: Status, message: String) -> ApiResponse {
        ApiResponse{is_ok: false, ok_data: None, fail_data: Some((status, message))}
    }
}

impl<'a> Responder<'a> for ApiResponse {
    fn respond_to(self, _: &Request) -> response::Result<'a> {
        if self.is_ok {
            let payload = api::OkResponse{ success: true, data: self.ok_data.unwrap()};
            match serde_json::to_string(&payload) {
                Ok(json) => {
                    Ok(Response::build()
                        .status(Status::Ok)
                        .header(ContentType::JSON)
                        .sized_body(Cursor::new(json))
                        .finalize())
                },
                Err(_) => Err(Status::InternalServerError)
            }
        } else {
            let (status, message) = self.fail_data.unwrap();
            let payload = api::ErrorResponse{ success: false, message };
            match serde_json::to_string(&payload) {
                Ok(json) => {
                    Ok(Response::build()
                        .status(status)
                        .header(ContentType::JSON)
                        .sized_body(Cursor::new(json))
                        .finalize())
                },
                Err(_) => Err(Status::InternalServerError)
            }
        }
    }
}

pub struct PdbFile {
    pub path: PathBuf,
}

impl<'a> Responder<'a> for PdbFile {
    fn respond_to(self, _: &Request) -> response::Result<'a> {
        let fh = match std::fs::File::open(self.path) {
            Ok(fh) => fh,
            Err(_) => return Err(Status::NotFound),
        };

        Ok(Response::build()
            .status(Status::Ok)
            .raw_header("Content-Type", "chemical/pdb")
            .sized_body(fh)
            .finalize()
        )
    }
}

#[derive(Debug)]
pub struct WMSError {
    pub status: Status,
}

impl<'a> Responder<'a> for WMSError {
    fn respond_to(self, _: &Request) -> response::Result<'a> {
        Err(self.status)
    }
}
