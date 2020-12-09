use rocket::http::{Cookie, Cookies, SameSite};
use uuid::Uuid;

const AUTH_NAME: &'static str = "auth";

pub fn make_auth_cookie(domain: String, session_id: String) -> Cookie<'static> {
    Cookie::build(AUTH_NAME, session_id)
        .domain(domain)
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
