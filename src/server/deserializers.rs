use std::io::{self, Read};
use rocket::{Data, Outcome::*};
use rocket::data::{FromData, Outcome, Transform, Transformed};
use rocket::http::Status;
use rocket::request::Request;
use serde_json::Result;

use crate::server::api;

const MAX_JSON_SIZE: u64 = 32 * 1024 * 1024;

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
/*
impl<'a> FromData<'a> for JobQuery {
    type Error = JsonParseError;
    type Owned = String;
    type Borrowed = str;

    fn transform(_: &Request, data: Data) -> Transform<Outcome<Self::Owned, Self::Error>> {
        transform_json(data)
    }

    fn from_data(_: &Request, outcome: Transformed<'a, Self>) -> Outcome<Self, Self::Error> {
        let s = outcome.borrowed()?;

        println!("{}", s);

        let parsed: Result<JobQuery> = serde_json::from_str(s);
        match parsed {
            Ok(parsed) => Success(parsed),
            Err(e) => Failure((Status::UnprocessableEntity, JsonParseError::Parse))
        }
    }
}

impl<'a> FromData<'a> for MmbCommands {
    type Error = JsonParseError;
    type Owned = String;
    type Borrowed = str;

    fn transform(_: &Request, data: Data) -> Transform<Outcome<Self::Owned, Self::Error>> {
        transform_json(data)
    }

    fn from_data(_: &Request, outcome: Transformed<'a, Self>) -> Outcome<Self, Self::Error> {
        let s = outcome.borrowed()?;

        println!("COMMANDS: {}", s);
        let parsed: Result<HashMap<String, Vec<String>>> = serde_json::from_str(s);
        match parsed {
            Ok(parsed) => Success(MmbCommands{tokens: parsed}),
            Err(e) => {
                println!("{}", e);
                return Failure((Status::UnprocessableEntity, JsonParseError::Parse))
            }
        }
    }
}
*/
