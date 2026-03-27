use crate::utils::auth;

// Wrapper for password hashing that accepts a string slice and
// returns the hash as a `String`. The underlying implementation
// lives in `utils::auth` and returns a Result; here we propagate
// errors as a simple panic-safe fallback returning an empty string
// (handlers will treat empty as an error if needed).
pub fn hash_string(password: &str) -> String {
    match auth::hash_password(password) {
        Ok(h) => h,
        Err(_) => String::new(),
    }
}
