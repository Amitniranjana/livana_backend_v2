use axum::{
    Json, RequestPartsExt,
    extract::FromRequestParts,
    http::{StatusCode, request::Parts},
    response::{IntoResponse, Response},
};
use axum_extra::{
    TypedHeader,
    headers::{Authorization, authorization::Bearer},
};
use jsonwebtoken::{DecodingKey, Validation, decode};
use serde_json::json;

use crate::app_state::AppState;
use crate::utils::auth::Claims;

#[derive(Debug)]
pub struct AuthenticationUser {
    pub user_id: String,
}

impl FromRequestParts<AppState> for AuthenticationUser {
    type Rejection = Response;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        // Extract the "Authorization" header
        let TypedHeader(Authorization(bearer)) = parts
            .extract::<TypedHeader<Authorization<Bearer>>>()
            .await
            .map_err(|_| {
                let error_response = json!({
                    "success": false,
                    "message": "Missing or invalid authorization header",
                    "data": null
                });
                (StatusCode::UNAUTHORIZED, Json(error_response)).into_response()
            })?;

        let token = bearer.token();

        // 3. Decode the JWT
        let token_data = decode::<Claims>(
            token,
            &DecodingKey::from_secret(state.jwt_secret.as_bytes()),
            &Validation::default(),
        )
        .map_err(|e| {
            let error_response = json!({
                "success": false,
                "message": format!("Invalid token: {}", e),
                "data": null
            });
            (StatusCode::UNAUTHORIZED, Json(error_response)).into_response()
        })?;

        Ok(AuthenticationUser {
            user_id: token_data.claims.sub,
        })
    }
}
