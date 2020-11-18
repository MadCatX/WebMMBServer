use rocket::http::{Cookie, Cookies};
use uuid::Uuid;

pub fn make_auth_cookie(session_id: String) -> Cookie<'static> {
    Cookie::build("auth", session_id)
        .domain("localhost")
        .path("/")
        .finish()
}

pub fn get_session_username(cookies: &mut Cookies) -> Option<Uuid> {
    match cookies.get_private("auth") {
        Some(c) => {
            match Uuid::parse_str(c.value()) {
                Ok(id) => Some(id),
                Err(_) => None,
            }
        },
        None => None,
    }
}
