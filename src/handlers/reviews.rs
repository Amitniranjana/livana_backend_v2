use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use uuid::Uuid;
use validator::Validate;

use crate::{
    app_state::AppState,
    dtos::{
        response::ApiResponse,
        reviews::{CreateReviewDto, CreateReviewResponseDto, ReviewDto},
    },
    utils::{api_error::ApiError, auth_extractor::AuthenticationUser},
};

/// 2.1 Add Review (POST /api/v1/reviews)
pub async fn add_review(
    State(app_state): State<AppState>,
    auth: AuthenticationUser,
    Json(payload): Json<CreateReviewDto>,
) -> Result<impl IntoResponse, ApiError> {
    // 1. Validation
    if let Err(errors) = payload.validate() {
        let mut err_map = std::collections::HashMap::new();
        for (field, field_errs) in errors.field_errors() {
            let messages: Vec<String> = field_errs
                .iter()
                .filter_map(|e| e.message.as_ref().map(|s| s.to_string()))
                .collect();
            err_map.insert(field.to_string(), messages);
        }
        return Err(ApiError::ValidationError(err_map));
    }

    let auth_user_id_uuid = Uuid::parse_str(&auth.user_id).unwrap_or_default();

    // 2. Enforce role: USER (Only users should be able to review associates)
    let user_role: (String,) = sqlx::query_as("SELECT user_role FROM users WHERE id = $1")
        .bind(auth_user_id_uuid)
        .fetch_one(&app_state.db)
        .await
        .map_err(|_| ApiError::Unauthorized("Invalid user".to_string()))?;

    if user_role.0 != "USER" && user_role.0 != "user" {
        return Err(ApiError::Forbidden(
            "Only users can add reviews".to_string(),
        ));
    }

    // 3. Create Review
    let review_id = Uuid::new_v4();
    let now = chrono::Utc::now();

    sqlx::query(
        r#"
        INSERT INTO reviews (id, user_id, associate_id, rating, comment, created_at)
        VALUES ($1, $2, $3, $4, $5, $6)
        "#,
    )
    .bind(review_id)
    .bind(auth_user_id_uuid)
    .bind(payload.associate_id)
    .bind(payload.rating as i32) // Postgres usually maps integer types; casting u8 to i32
    .bind(&payload.comment)
    .bind(now)
    .execute(&app_state.db)
    .await
    .map_err(|e| ApiError::InternalServerError(format!("Failed to add review: {}", e)))?;

    let response = ApiResponse {
        success: true,
        message: "Review added successfully".to_string(),
        data: CreateReviewResponseDto {
            review_id,
            created_at: now,
        },
    };

    Ok((StatusCode::CREATED, Json(response)))
}

/// 2.2 Get Reviews (GET /api/v1/associates/{id}/reviews)
pub async fn get_reviews(
    State(app_state): State<AppState>,
    Path(associate_id): Path<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    // Role: Public (Anyone can read reviews)

    let reviews_result: Result<Vec<(Uuid, Uuid, i32, String, chrono::DateTime<chrono::Utc>)>, _> =
        sqlx::query_as(
            r#"
        SELECT id, user_id, rating, comment, created_at
        FROM reviews
        WHERE associate_id = $1
        "#,
        )
        .bind(associate_id)
        .fetch_all(&app_state.db)
        .await;

    let mut reviews = vec![];
    if let Ok(records) = reviews_result {
        for record in records {
            reviews.push(ReviewDto {
                review_id: record.0,
                user_id: record.1,
                rating: record.2 as u8,
                comment: record.3,
                created_at: record.4,
            });
        }
    }

    let response = ApiResponse {
        success: true,
        message: "Reviews retrieved successfully".to_string(),
        data: reviews,
    };

    Ok((StatusCode::OK, Json(response)))
}
