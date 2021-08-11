use std::io::{self, Read};
use rocket::{Data, Outcome::*};
use rocket::data::{FromData, Outcome, Transform, Transformed};
use rocket::http::Status;
use rocket::request::Request;
use serde_json::Result;

use crate::server::api;

const MAX_JSON_SIZE: u64 = 4096;
const MAX_CHUNK_SIZE: u64 = 8 * 1024 * 1024;

pub enum JsonParseError {
    Io(io::Error),
    Parse
}

fn transform_json(data: Data) -> Transform<Outcome<String, JsonParseError>> {
    let mut stream = data.open().take(MAX_JSON_SIZE);
        let mut string = String::with_capacity((MAX_JSON_SIZE / 8) as usize);
        let outcome = match stream.read_to_string(&mut string) {
            Ok(_) => Success(string),
            Err(e) => Failure((Status::InternalServerError, JsonParseError::Io(e)))
        };

        Transform::Borrowed(outcome)
}

impl<'a> FromData<'a> for api::ApiRequest {
    type Error = JsonParseError;
    type Owned = String;
    type Borrowed = str;

    fn transform(_: &Request, data: Data) -> Transform<Outcome<Self::Owned, Self::Error>> {
        transform_json(data)
    }

    fn from_data(_: &Request, outcome: Transformed<'a, Self>) -> Outcome<Self, Self::Error> {
        let s = outcome.borrowed()?;

        println!("{}", s);

        let parsed: Result<api::ApiRequest> = serde_json::from_str(s);
        match parsed {
            Ok(parsed) => Success(parsed),
            Err(e) => {
                println!("Bad ApiRequest{}", e.to_string());
                Failure((Status::BadRequest, JsonParseError::Parse))
            }
        }
    }
}

impl<'a> FromData<'a> for api::AuthRequest {
    type Error = JsonParseError;
    type Owned = String;
    type Borrowed = str;

    fn transform(_: &Request, data: Data) -> Transform<Outcome<Self::Owned, Self::Error>> {
        transform_json(data)
    }

    fn from_data(_: &Request, outcome: Transformed<'a, Self>) -> Outcome<Self, Self::Error> {
        let s = outcome.borrowed()?;

        println!("{}", s);

        let parsed: Result<api::AuthRequest> = serde_json::from_str(s);
        match parsed {
            Ok(parsed) => Success(parsed),
            Err(e) => {
                println!("Bad AuthRequest{}", e.to_string());
                Failure((Status::BadRequest, JsonParseError::Parse))
            }
        }
    }
}

impl<'a> FromData<'a> for api::FileTransferChunk {
    type Error = String;
    type Owned = Vec<u8>;
    type Borrowed = Vec<u8>;

    fn transform(_request: &Request, data: Data) -> Transform<Outcome<Self::Owned, Self::Error>> {
        let mut stream = data.open().take(MAX_CHUNK_SIZE);
        let mut buf = Vec::<u8>::with_capacity(MAX_CHUNK_SIZE as usize);
        let outcome = match stream.read_to_end(&mut buf) {
            Ok(_) => Success(buf),
            Err(e) => Failure((Status::InternalServerError, format!("Cannot read request data: {}", e.to_string()))),
        };

        Transform::Borrowed(outcome)
    }

    fn from_data(_: &Request, outcome: Transformed<'a, Self>) -> Outcome<Self, Self::Error> {
        let buf = outcome.borrowed()?;

        match api::FileTransferChunk::from_bytes(&buf) {
            Ok(chunk) => Success(chunk),
            Err(e) => Failure((Status::BadRequest, e)),
        }
    }
}
