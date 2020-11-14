use rocket::http::{Cookie, Cookies};

pub fn make_auth_cookie(username: String) -> Cookie<'static> {
    Cookie::build("auth", username)
        .domain("localhost")
        .path("/")
        .finish()
}

pub fn get_session_username(cookies: &mut Cookies) -> Option<String> {
    match cookies.get_private("auth") {
        Some(c) => Some(String::from(c.value())),
        None => None,
    }
}
