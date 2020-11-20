use rocket::http::{Cookie, Cookies};
use uuid::Uuid;

const AuthName: &'static str = "auth";

pub fn make_auth_cookie(domain: String, session_id: String) -> Cookie<'static> {
    Cookie::build(AuthName, session_id)
        .domain(domain)
        .path("/")
        .finish()
}

pub fn get_session_username(cookies: &mut Cookies) -> Option<Uuid> {
    match cookies.get_private(AuthName) {
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
    cookies.remove_private(Cookie::named(AuthName));
}
