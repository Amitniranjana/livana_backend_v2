use axum::{
    extract::State,
    http::{StatusCode, HeaderMap},
    Json,
};
use bcrypt::verify;
use jsonwebtoken::{encode, decode, Header, Validation, EncodingKey, DecodingKey};
use serde::{Deserialize, Serialize};
use std::{env, time::{SystemTime, UNIX_EPOCH}};
use axum_extra::extract::cookie::{Cookie, SameSite, CookieJar};

use crate::app_state::AppState;

#[derive(Deserialize)]
pub struct AdminLoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Serialize)]
pub struct AdminAuthResponse {
    pub success: bool,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
}

#[derive(Serialize)]
pub struct AdminMeResponse {
    pub email: String,
    pub role: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AdminClaims {
    pub sub: String,
    pub role: String,
    pub exp: usize,
}

// In-memory simple rate limiter: IP -> (attempts, reset_time)
use dashmap::DashMap;
use std::time::{Instant, Duration};

static RATE_LIMITS: std::sync::LazyLock<DashMap<String, (u32, Instant)>> = std::sync::LazyLock::new(DashMap::new);

pub async fn admin_login(
    State(state): State<AppState>,
    jar: CookieJar,
    headers: HeaderMap,
    Json(payload): Json<AdminLoginRequest>,
) -> Result<(CookieJar, Json<AdminAuthResponse>), (StatusCode, Json<AdminAuthResponse>)> {
    // 1. Extract IP for rate limiting
    let ip = headers
        .get("x-forwarded-for")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.split(',').next().unwrap_or("").trim().to_string())
        .unwrap_or_else(|| "127.0.0.1".to_string());

    // 2. Check rate limits (5 per 15 mins)
    let now = Instant::now();
    let window = Duration::from_secs(15 * 60);

    let mut allowed = true;
    if let Some(mut record) = RATE_LIMITS.get_mut(&ip) {
        if now.duration_since(record.1) > window {
            record.0 = 1;
            record.1 = now;
        } else {
            record.0 += 1;
            if record.0 > 5 {
                allowed = false;
            }
        }
    } else {
        RATE_LIMITS.insert(ip.clone(), (1, now));
    }

    if !allowed {
        return Err((
            StatusCode::TOO_MANY_REQUESTS,
            Json(AdminAuthResponse {
                success: false,
                message: "Too many login attempts. Please try again in 15 minutes.".into(),
                token: None,
            }),
        ));
    }

    // 3. Validate credentials
    let admin_email = env::var("ADMIN_EMAIL").unwrap_or_else(|_| "admin@livana.com".into());
    let admin_password_hash = env::var("ADMIN_PASSWORD").unwrap_or_else(|_| "".into());

    if payload.email != admin_email {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(AdminAuthResponse {
                success: false,
                message: "Invalid credentials".into(),
                token: None,
            }),
        ));
    }

    // Verify cost factor >= 12 as per requirements
    let parts: Vec<&str> = admin_password_hash.split('$').collect();
    if parts.len() >= 3 {
        if let Ok(cost) = parts[2].parse::<u32>() {
            if cost < 12 {
                log::warn!("ADMIN_PASSWORD hash has a cost factor lower than 12! Current cost: {}", cost);
            }
        }
    }

    let is_valid_password = verify(&payload.password, &admin_password_hash).unwrap_or(false);
    
    // For local dev if hash is missing or misconfigured, and we passed exactly empty hash, it fails.
    if !is_valid_password {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(AdminAuthResponse {
                success: false,
                message: "Invalid credentials".into(),
                token: None,
            }),
        ));
    }

    // 4. Generate JWT
    let exp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_secs() as usize
        + (24 * 3600); // 24 hours

    let claims = AdminClaims {
        sub: payload.email.clone(),
        role: "admin".into(),
        exp,
    };

    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(state.admin_jwt_secret.as_ref()),
    )
    .map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(AdminAuthResponse {
                success: false,
                message: "Token generation failed".into(),
                token: None,
            }),
        )
    })?;

    // 5. Set Cookie
    let cookie = Cookie::build(("admin_session", token))
        .path("/")
        .http_only(true)
        .secure(true)
        .same_site(SameSite::None)
        .build();

    let updated_jar = jar.add(cookie);

    Ok((
        updated_jar,
        Json(AdminAuthResponse {
            success: true,
            message: "Login successful".into(),
            token: Some(token),
        }),
    ))
}

pub async fn admin_logout(
    jar: CookieJar,
) -> (CookieJar, Json<AdminAuthResponse>) {
    // CookieJar::remove will create a removal cookie with negative max_age automatically
    let mut remove_cookie = Cookie::from("admin_session");
    remove_cookie.set_path("/");
    let updated_jar = jar.remove(remove_cookie);

    (
        updated_jar,
        Json(AdminAuthResponse {
            success: true,
            message: "Logged out successfully".into(),
            token: None,
        }),
    )
}

pub async fn admin_me(
    State(state): State<AppState>,
    jar: CookieJar,
    req: axum::extract::Request,
) -> Result<Json<AdminMeResponse>, (StatusCode, Json<AdminAuthResponse>)> {
    let session_cookie = jar.get("admin_session").map(|c| c.value().to_string());
    
    let auth_header = req.headers().get("authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .map(|s| s.to_string());

    let token = session_cookie.or(auth_header).ok_or_else(|| {
        (
            StatusCode::UNAUTHORIZED,
            Json(AdminAuthResponse {
                success: false,
                message: "No active session".into(),
                token: None,
            }),
        )
    })?;

    let token_data = decode::<AdminClaims>(
        &token,
        &DecodingKey::from_secret(state.admin_jwt_secret.as_ref()),
        &Validation::default(),
    )
    .map_err(|_| {
        (
            StatusCode::UNAUTHORIZED,
            Json(AdminAuthResponse {
                success: false,
                message: "Invalid or expired session".into(),
                token: None,
            }),
        )
    })?;

    Ok(Json(AdminMeResponse {
        email: token_data.claims.sub,
        role: token_data.claims.role,
    }))
}
