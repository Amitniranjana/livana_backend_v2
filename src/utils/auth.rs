use argon2::{Argon2, PasswordHasher, PasswordVerifier};
use argon2::password_hash::{SaltString, rand_core::OsRng, PasswordHash};
use jsonwebtoken::{encode, EncodingKey, Header, decode, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use chrono::{Utc, Duration};
use anyhow::Result;

pub fn hash_password(password: &str) -> Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let hash = argon2.hash_password(password.as_bytes(), &salt)?.to_string();
    Ok(hash)
}

pub fn verify_password(hash: &str, password: &str) -> bool {
    let parsed = PasswordHash::new(hash);
    if let Err(_) = parsed { return false; }
    Argon2::default().verify_password(password.as_bytes(), parsed.unwrap()).is_ok()
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub exp: usize,
}

pub fn create_jwt(subject: &str, secret: &str, hours_valid: i64) -> Result<String> {
    let exp = (Utc::now() + Duration::hours(hours_valid)).timestamp() as usize;
    let claims = Claims { sub: subject.to_owned(), exp };
    let token = encode(&Header::default(), &claims, &EncodingKey::from_secret(secret.as_bytes()))?;
    Ok(token)
}

pub fn decode_jwt(token: &str, secret: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
    let data = decode::<Claims>(token, &DecodingKey::from_secret(secret.as_bytes()), &Validation::default())?;
    Ok(data.claims)
}
