#![feature(proc_macro_hygiene, decl_macro)]
extern crate rocket;
extern crate uuid;

mod config;
mod mmb;
mod server;
mod session;

fn init() {
    let p = std::path::Path::new(config::get().jobs_dir.as_str());
    if !std::path::Path::is_dir(p) {
        let mut db = std::fs::DirBuilder::new();
        db.recursive(true);
        db.create(p).expect("Failed to create jobs directory");
    }
}

fn main() {
    config::load("./cfg.json");
    init();

    server::start();
}
