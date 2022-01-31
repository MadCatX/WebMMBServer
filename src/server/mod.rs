pub mod api;
mod deserializers;
mod file_transfer_chunk;
mod request_handlers;
mod responders;
mod server;
mod session_cookie;
mod transfer_handlers;

use rocket;

pub const LOGSRC: &'static str = "server";

pub fn start() -> rocket::Rocket<rocket::Build> {
    server::start()
}
