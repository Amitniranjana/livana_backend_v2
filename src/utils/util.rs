use std::error::Error;

use argon2::{
    password_hash::{
        rand_core::OsRng,
         PasswordHasher, SaltString,
    },
    Argon2
};


pub fn hash_string(password: String) -> Result<String, Box<dyn Error>> {
    let salt = SaltString::generate(&mut OsRng);
    // Argon2 with default params (Argon2id v19)
    let argon2 = Argon2::default();
    let password_hash = argon2.hash_password(password.as_bytes(), &salt);

    match password_hash {
        Ok(pass) => Ok(pass.to_string()),
        Err(e) => Err(Box::new(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("Password hashing failed: {}", e),
        ))),
    }
}