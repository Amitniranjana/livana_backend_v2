use argon2::password_hash::{PasswordHash, SaltString, rand_core::OsRng};
use argon2::{Argon2, PasswordHasher, PasswordVerifier};
use chrono::{Duration, Utc};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use std::error::Error;

pub fn hash_password(password: &str) -> Result<String, String> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let password_hash = argon2.hash_password(password.as_bytes(), &salt);

    match password_hash {
        Ok(ph) => Ok(ph.to_string()),
        Err(e) => Err(e.to_string()),
    }
}

pub fn verify_password(hash: &str, password: &str) -> bool {
    let parsed = PasswordHash::new(hash);
    if let Err(_) = parsed {
        return false;
    }
    let parsed = parsed.unwrap();
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub exp: usize,
}

pub fn create_jwt(subject: &str, secret: &str, hours_valid: i64) -> Result<String, Box<dyn Error>> {
    let exp = (Utc::now() + Duration::hours(hours_valid)).timestamp() as usize;
    let claims = Claims {
        sub: subject.to_owned(),
        exp,
    };
    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| Box::new(e) as Box<dyn Error>)?;
    Ok(token)
}

#[allow(dead_code)]
pub fn decode_jwt(token: &str, secret: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
    let data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )?;
    Ok(data.claims)
}
