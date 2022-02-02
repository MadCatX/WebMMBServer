#![feature(proc_macro_hygiene, decl_macro)]
#[macro_use] extern crate rocket;
extern crate uuid;

mod config;
mod logging;
mod mmb;
mod pbs;
mod server;
mod session;

const LOGSRC: &'static str = "main";

fn init() {
    let p = std::path::Path::new(config::get().jobs_dir.as_str());
    if !std::path::Path::is_dir(p) {
        let mut db = std::fs::DirBuilder::new();
        db.recursive(true);
        match db.create(p) {
            Ok(()) => (),
            Err(e) => {
                log_plain!(Critical, LOGSRC, &format!("Failed to create working directory: {}", e.to_string()));
                panic!();
            },
        }
    }
}

#[rocket::launch]
fn liftoff() -> _ {
    logging::log_startup_message();
    init();

    server::start()
}
