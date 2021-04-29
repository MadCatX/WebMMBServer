pub mod api;
mod deserializers;
mod request_handlers;
mod responders;
mod server;
mod session_cookie;

use std::sync::Arc;

use crate::config::Config;

pub fn start(cfg: Arc<Config>) {
    server::start(cfg);
}
