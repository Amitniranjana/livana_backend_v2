// tmp script to generate a test JWT using the same secret as the server
use chrono::{Duration, Utc};
use jsonwebtoken::{EncodingKey, Header, encode};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct Claims {
    sub: String,
    exp: usize,
}

fn main() {
    let secret = std::env::var("JWT_SECRET").unwrap_or_else(|_| "supersecret".into());
    let exp = (Utc::now() + Duration::hours(1)).timestamp() as usize;
    let claims = Claims {
        sub: "00000000-0000-0000-0000-000000000001".to_string(),
        exp,
    };
    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .unwrap();
    println!("{}", token);
}
