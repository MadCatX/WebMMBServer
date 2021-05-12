pub mod api;
mod deserializers;
mod file_transfer_chunk;
mod request_handlers;
mod responders;
mod server;
mod session_cookie;
mod transfer_handlers;

use std::sync::Arc;

use crate::config::Config;

pub fn start(cfg: Arc<Config>) {
    server::start(cfg);
}
