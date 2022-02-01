use std::io::{Cursor, Read};
use std::path::PathBuf;
use rocket::http::{ContentType, Status};
use rocket::request::Request;
use rocket::response::{self, Response, Responder};
use serde_json;

use crate::logging;
use crate::server::{api, LOGSRC};

impl api::ApiResponse {
    pub fn ok(data: serde_json::Value) -> api::ApiResponse {
        api::ApiResponse{is_ok: true, ok_data: Some(data), fail_data: None}
    }

    pub fn fail(status: Status, message: String) -> api::ApiResponse {
        api::ApiResponse{is_ok: false, ok_data: None, fail_data: Some((status, message))}
    }
}

impl<'a, 'b: 'a> Responder<'a, 'b> for api::ApiResponse {
    fn respond_to(self, _: &'a Request<'_>) -> response::Result<'b> {
        if self.is_ok {
            let payload = api::OkResponse{ success: true, data: self.ok_data.unwrap()};
            match serde_json::to_string(&payload) {
                Ok(json) => {
                    Ok(Response::build()
                        .status(Status::Ok)
                        .header(ContentType::JSON)
                        .sized_body(json.len(), Cursor::new(json))
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
                        .sized_body(json.len(), Cursor::new(json))
                        .finalize())
                },
                Err(_) => Err(Status::InternalServerError)
            }
        }
    }
}

impl<'a, 'b: 'a> Responder<'a, 'b> for api::AuthResponse {
    fn respond_to(self, _: &'a Request<'_>) -> response::Result<'b> {
        Ok(Response::build()
            .status(self.status)
            .header(ContentType::Plain)
            .sized_body(self.message.len(), Cursor::new(self.message))
            .finalize())
    }
}

pub struct DensityFile {
    pub path: PathBuf,
}

impl<'a, 'b: 'a> Responder<'a, 'b> for DensityFile {
    fn respond_to(self, _: &'a Request<'_>) -> response::Result<'b> {
        let file_name = match &self.path.file_name() {
            Some(name) => name.to_string_lossy(),
            None => return Err(Status::NotFound),
        };
        let mut fh = match std::fs::File::open(&self.path) {
            Ok(fh) => fh,
            Err(_) => return Err(Status::NotFound),
        };

        let mut payload = Vec::<u8>::new();
        if let Err(e) = fh.read_to_end(&mut payload) {
            logging::log(logging::Priority::Error, LOGSRC, &format!("Cannot read DensityFile {}: {}", file_name, e.to_string()));
            return Err(Status::InternalServerError);
        }

        Ok(Response::build()
            .status(Status::Ok)
            .raw_header("Content-Type", "application/octet-stream")
            .raw_header("Content-Disposition", format!("attachment; filename=\"{}\"", file_name))
            .sized_body(payload.len(), Cursor::new(payload))
            .finalize()
        )
    }
}

pub struct PdbFile {
    pub path: PathBuf,
}

impl<'a, 'b: 'a> Responder<'a, 'b> for PdbFile {
    fn respond_to(self, _: &'a Request<'_>) -> response::Result<'b> {
        let mut fh = match std::fs::File::open(&self.path) {
            Ok(fh) => fh,
            Err(_) => return Err(Status::NotFound),
        };

        let mut payload = Vec::<u8>::new();
        if let Err(e) = fh.read_to_end(&mut payload) {
            logging::log(logging::Priority::Error, LOGSRC, &format!("Cannot read PdbFile {}: {}", self.path.as_os_str().to_str().unwrap_or(logging::INV_FILE_PATH), e.to_string()));
            return Err(Status::InternalServerError);
        }

        Ok(Response::build()
            .status(Status::Ok)
            .raw_header("Content-Type", "chemical/pdb")
            .sized_body(payload.len(), Cursor::new(payload))
            .finalize()
        )
    }
}


#[derive(Debug)]
pub struct WMSError {
    pub status: Status,
}

impl<'a, 'b: 'a> Responder<'a, 'b> for WMSError {
    fn respond_to(self, _: &'a Request<'_>) -> response::Result<'b> {
        Err(self.status)
    }
}
