extern crate time;

use std::ops::Add;
use rocket::http::{Cookie, Cookies, SameSite};
use uuid::Uuid;

pub const AUTH_NAME: &'static str = "webmmb_auth";

pub fn make_auth_cookie(domain: String, session_id: String) -> Cookie<'static> {
    let now = time::now();
    let expire_on = now.add(time::Duration::days(1));

    Cookie::build(AUTH_NAME, session_id)
        .domain(domain)
        .expires(expire_on)
        .path("/")
        .same_site(SameSite::Strict)
        .secure(true)
        .finish()
}

pub fn get_session_id(cookies: &mut Cookies) -> Option<Uuid> {
    match cookies.get_private(AUTH_NAME) {
        Some(c) => {
            match Uuid::parse_str(c.value()) {
                Ok(id) => Some(id),
                Err(_) => None,
            }
        },
        None => None,
    }
}

pub fn remove_session_cookie(cookies: &mut Cookies) {
    cookies.remove_private(Cookie::named(AUTH_NAME));
}
