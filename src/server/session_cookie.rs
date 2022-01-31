extern crate time;

use rocket::http::{Cookie, CookieJar, SameSite};
use uuid::Uuid;

pub const AUTH_NAME: &'static str = "webmmb_auth";

pub fn make_auth_cookie(domain: String, session_id: String, require_https: bool) -> Cookie<'static> {
    let now = time::OffsetDateTime::now_utc();
    let expire_on = now + time::Duration::days(1);

    Cookie::build(AUTH_NAME, session_id)
        .domain(domain)
        .expires(expire_on)
        .path("/")
        .same_site(SameSite::Strict)
        .secure(require_https)
        .finish()
}

pub fn get_session_id(jar: &CookieJar<'_>) -> Option<Uuid> {
    match jar.get_private(AUTH_NAME) {
        Some(c) => {
            match Uuid::parse_str(c.value()) {
                Ok(id) => Some(id),
                Err(_) => None,
            }
        },
        None => None,
    }
}

pub fn remove_session_cookie(jar: &CookieJar<'_>) {
    jar.remove_private(Cookie::named(AUTH_NAME));
}
