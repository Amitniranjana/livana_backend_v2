// src/handlers/carecrew_reviews.rs
//
// CareCrew Review & Rating APIs:
//   POST   /api/reviews/carecrew                    — Submit Review
//   GET    /api/reviews/carecrew/{provider_id}      — Get Provider Reviews
//   PUT    /api/reviews/carecrew/{review_id}        — Edit Review
//   DELETE /api/reviews/carecrew/{review_id}        — Delete Review
//   POST   /api/reviews/carecrew/{review_id}/reply  — Reply to Review

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chrono::Utc;
use uuid::Uuid;

use crate::{
    app_state::AppState,
    dtos::{
        carecrew_review::{
            CarecrewReviewCreatedData, CarecrewReviewItem, CarecrewReviewSummary,
            CarecrewReviewUpdatedData, CarecrewReviewsListData,
            CreateCarecrewReviewRequest, EditCarecrewReviewRequest,
            RatingBreakdown, ReviewDeletedData, ReviewListQuery, ReviewReplyData,
            ReplyRequest,
        },
        response::ApiResponse,
    },
    utils::{api_error::ApiError, auth_extractor::AuthenticationUser},
};

fn validate_rating(rating: f64) -> Result<(), ApiError> {
    if !(1.0..=5.0).contains(&rating) {
        return Err(ApiError::CustomError(
            StatusCode::BAD_REQUEST,
            "Rating must be between 1.0 and 5.0".to_string(),
            "INVALID_RATING".to_string(),
        ));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// POST /api/reviews/carecrew — Submit CareCrew Review
// ---------------------------------------------------------------------------

pub async fn create_carecrew_review(
    State(app_state): State<AppState>,
    auth: AuthenticationUser,
    Json(payload): Json<CreateCarecrewReviewRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let reviewer_id = Uuid::parse_str(&auth.user_id)
        .map_err(|_| ApiError::Unauthorized("Invalid user token".to_string()))?;

    // 1. Check booking is completed
    let booking_row: Option<(String,)> = sqlx::query_as(
        "SELECT status FROM carecrew_bookings WHERE id = $1",
    )
    .bind(payload.booking_id)
    .fetch_optional(&app_state.db)
    .await
    .map_err(|e| ApiError::InternalServerError(format!("Database error: {}", e)))?;

    match booking_row {
        Some((status,)) => {
            if status != "completed" {
                return Err(ApiError::CustomError(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "Booking must be completed before submitting a review".to_string(),
                    "BOOKING_NOT_COMPLETED".to_string(),
                ));
            }
        }
        None => {
            return Err(ApiError::NotFound("Booking not found".to_string()));
        }
    }

    // 2. Check for existing review on this booking
    let existing: Option<(Uuid,)> = sqlx::query_as(
        "SELECT id FROM carecrew_reviews WHERE booking_id = $1",
    )
    .bind(payload.booking_id)
    .fetch_optional(&app_state.db)
    .await
    .map_err(|e| ApiError::InternalServerError(format!("Database error: {}", e)))?;

    if existing.is_some() {
        return Err(ApiError::CustomError(
            StatusCode::CONFLICT,
            "A review already exists for this booking".to_string(),
            "REVIEW_ALREADY_EXISTS".to_string(),
        ));
    }

    // 3. Validate rating
    validate_rating(payload.rating)?;

    // 4. Insert review
    let review_id = Uuid::new_v4();
    let now = Utc::now();

    sqlx::query(
        r#"
        INSERT INTO carecrew_reviews (id, booking_id, provider_id, reviewer_id, rating, comment, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $7)
        "#,
    )
    .bind(review_id)
    .bind(payload.booking_id)
    .bind(payload.provider_id)
    .bind(reviewer_id)
    .bind(payload.rating)
    .bind(&payload.comment)
    .bind(now)
    .execute(&app_state.db)
    .await
    .map_err(|e| ApiError::InternalServerError(format!("Failed to create review: {}", e)))?;

    let response = ApiResponse {
        success: true,
        message: "Review submitted successfully".to_string(),
        data: CarecrewReviewCreatedData {
            review_id,
            booking_id: payload.booking_id,
            provider_id: payload.provider_id,
            reviewer_id,
            rating: payload.rating,
            comment: payload.comment,
            created_at: now.to_rfc3339(),
        },
    };

    Ok((StatusCode::CREATED, Json(response)))
}

// ---------------------------------------------------------------------------
// GET /api/reviews/carecrew/{provider_id} — Get Provider Reviews
// ---------------------------------------------------------------------------

pub async fn get_carecrew_reviews(
    State(app_state): State<AppState>,
    _auth: AuthenticationUser,
    Path(provider_id): Path<Uuid>,
    Query(params): Query<ReviewListQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let limit = params.limit.unwrap_or(10).clamp(1, 100);
    let offset = params.offset.unwrap_or(0).max(0);

    // Check provider exists
    let provider_exists: Option<(Uuid,)> = sqlx::query_as(
        "SELECT id FROM carecrew_providers WHERE id = $1",
    )
    .bind(provider_id)
    .fetch_optional(&app_state.db)
    .await
    .map_err(|e| ApiError::InternalServerError(format!("Database error: {}", e)))?;

    if provider_exists.is_none() {
        return Err(ApiError::CustomError(
            StatusCode::NOT_FOUND,
            "Provider not found".to_string(),
            "NOT_FOUND".to_string(),
        ));
    }

    // Total count
    let total_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM carecrew_reviews WHERE provider_id = $1",
    )
    .bind(provider_id)
    .fetch_one(&app_state.db)
    .await
    .map_err(|e| ApiError::InternalServerError(format!("Database error: {}", e)))?;

    // Average rating
    let avg_rating: Option<f64> = sqlx::query_scalar(
        "SELECT AVG(rating)::FLOAT8 FROM carecrew_reviews WHERE provider_id = $1",
    )
    .bind(provider_id)
    .fetch_one(&app_state.db)
    .await
    .map_err(|e| ApiError::InternalServerError(format!("Database error: {}", e)))?;

    // Rating breakdown
    let breakdown_rows: Vec<(i32, i64)> = sqlx::query_as(
        r#"
        SELECT FLOOR(rating)::INT as star, COUNT(*) as cnt
        FROM carecrew_reviews
        WHERE provider_id = $1
        GROUP BY star
        "#,
    )
    .bind(provider_id)
    .fetch_all(&app_state.db)
    .await
    .map_err(|e| ApiError::InternalServerError(format!("Database error: {}", e)))?;

    let mut breakdown = RatingBreakdown {
        five: 0, four: 0, three: 0, two: 0, one: 0,
    };
    for (star, cnt) in &breakdown_rows {
        match star {
            5 => breakdown.five = *cnt,
            4 => breakdown.four = *cnt,
            3 => breakdown.three = *cnt,
            2 => breakdown.two = *cnt,
            1 => breakdown.one = *cnt,
            _ => {}
        }
    }

    // Paginated reviews with reviewer info
    let review_rows: Vec<(Uuid, Option<String>, Option<String>, f64, Option<String>, Option<String>, Option<chrono::DateTime<chrono::Utc>>, chrono::DateTime<chrono::Utc>)> = sqlx::query_as(
        r#"
        SELECT
            cr.id,
            CONCAT(COALESCE(u.first_name, ''), ' ', COALESCE(u.last_name, '')) as reviewer_name,
            u.profile_image_url as reviewer_image,
            cr.rating::FLOAT8,
            cr.comment,
            cr.reply,
            cr.reply_at,
            cr.created_at
        FROM carecrew_reviews cr
        LEFT JOIN users u ON u.id = cr.reviewer_id
        WHERE cr.provider_id = $1
        ORDER BY cr.created_at DESC
        LIMIT $2 OFFSET $3
        "#,
    )
    .bind(provider_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(&app_state.db)
    .await
    .map_err(|e| ApiError::InternalServerError(format!("Database error: {}", e)))?;

    let reviews: Vec<CarecrewReviewItem> = review_rows
        .into_iter()
        .map(|(id, reviewer_name, reviewer_image, rating, comment, reply, reply_at, created_at)| {
            CarecrewReviewItem {
                id,
                reviewer_name: reviewer_name.unwrap_or_default(),
                reviewer_image,
                rating,
                comment,
                reply,
                reply_at: reply_at.map(|t| t.to_rfc3339()),
                review_date: created_at.to_rfc3339(),
            }
        })
        .collect();

    let current_page = (offset / limit) + 1;
    let total_pages = if total_count == 0 { 0 } else { (total_count + limit - 1) / limit };

    let response = ApiResponse {
        success: true,
        message: "Reviews retrieved successfully".to_string(),
        data: CarecrewReviewsListData {
            reviews,
            summary: CarecrewReviewSummary {
                average_rating: avg_rating.unwrap_or(0.0),
                total_reviews: total_count,
                breakdown,
            },
            total_count,
            current_page,
            total_pages,
        },
    };

    Ok((StatusCode::OK, Json(response)))
}

// ---------------------------------------------------------------------------
// PUT /api/reviews/carecrew/{review_id} — Edit CareCrew Review
// ---------------------------------------------------------------------------

pub async fn edit_carecrew_review(
    State(app_state): State<AppState>,
    auth: AuthenticationUser,
    Path(review_id): Path<Uuid>,
    Json(payload): Json<EditCarecrewReviewRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let user_id = Uuid::parse_str(&auth.user_id)
        .map_err(|_| ApiError::Unauthorized("Invalid user token".to_string()))?;

    // 1. Review must exist
    let review_row: Option<(Uuid, chrono::DateTime<chrono::Utc>, f64, Option<String>)> = sqlx::query_as(
        "SELECT reviewer_id, created_at, rating::FLOAT8, comment FROM carecrew_reviews WHERE id = $1",
    )
    .bind(review_id)
    .fetch_optional(&app_state.db)
    .await
    .map_err(|e| ApiError::InternalServerError(format!("Database error: {}", e)))?;

    let (reviewer_id, created_at, current_rating, current_comment) = match review_row {
        Some(row) => row,
        None => {
            return Err(ApiError::CustomError(
                StatusCode::NOT_FOUND,
                "Review not found".to_string(),
                "REVIEW_NOT_FOUND".to_string(),
            ));
        }
    };

    // 2. Must be the reviewer
    if user_id != reviewer_id {
        return Err(ApiError::CustomError(
            StatusCode::FORBIDDEN,
            "You can only edit your own reviews".to_string(),
            "ACCESS_DENIED".to_string(),
        ));
    }

    // 3. Within 30 days
    let days_since = (Utc::now() - created_at).num_days();
    if days_since > 30 {
        return Err(ApiError::CustomError(
            StatusCode::FORBIDDEN,
            "Review edit period has expired (30 days)".to_string(),
            "EDIT_PERIOD_EXPIRED".to_string(),
        ));
    }

    // 4. Validate new rating if provided
    let new_rating = payload.rating.unwrap_or(current_rating);
    validate_rating(new_rating)?;

    let new_comment = payload.comment.or(current_comment);
    let now = Utc::now();

    sqlx::query(
        "UPDATE carecrew_reviews SET rating = $1, comment = $2, updated_at = $3 WHERE id = $4",
    )
    .bind(new_rating)
    .bind(&new_comment)
    .bind(now)
    .bind(review_id)
    .execute(&app_state.db)
    .await
    .map_err(|e| ApiError::InternalServerError(format!("Failed to update review: {}", e)))?;

    let response = ApiResponse {
        success: true,
        message: "Review updated successfully".to_string(),
        data: CarecrewReviewUpdatedData {
            review_id,
            rating: new_rating,
            comment: new_comment,
            updated_at: now.to_rfc3339(),
        },
    };

    Ok((StatusCode::OK, Json(response)))
}

// ---------------------------------------------------------------------------
// DELETE /api/reviews/carecrew/{review_id} — Delete CareCrew Review
// ---------------------------------------------------------------------------

pub async fn delete_carecrew_review(
    State(app_state): State<AppState>,
    auth: AuthenticationUser,
    Path(review_id): Path<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    let user_id = Uuid::parse_str(&auth.user_id)
        .map_err(|_| ApiError::Unauthorized("Invalid user token".to_string()))?;

    // 1. Review must exist
    let review_row: Option<(Uuid, chrono::DateTime<chrono::Utc>)> = sqlx::query_as(
        "SELECT reviewer_id, created_at FROM carecrew_reviews WHERE id = $1",
    )
    .bind(review_id)
    .fetch_optional(&app_state.db)
    .await
    .map_err(|e| ApiError::InternalServerError(format!("Database error: {}", e)))?;

    let (reviewer_id, created_at) = match review_row {
        Some(row) => row,
        None => {
            return Err(ApiError::CustomError(
                StatusCode::NOT_FOUND,
                "Review not found".to_string(),
                "REVIEW_NOT_FOUND".to_string(),
            ));
        }
    };

    // 2. Must be the reviewer
    if user_id != reviewer_id {
        return Err(ApiError::CustomError(
            StatusCode::FORBIDDEN,
            "You can only delete your own reviews".to_string(),
            "ACCESS_DENIED".to_string(),
        ));
    }

    // 3. Within 30 days
    let days_since = (Utc::now() - created_at).num_days();
    if days_since > 30 {
        return Err(ApiError::CustomError(
            StatusCode::FORBIDDEN,
            "Review delete period has expired (30 days)".to_string(),
            "EDIT_PERIOD_EXPIRED".to_string(),
        ));
    }

    sqlx::query("DELETE FROM carecrew_reviews WHERE id = $1")
        .bind(review_id)
        .execute(&app_state.db)
        .await
        .map_err(|e| ApiError::InternalServerError(format!("Failed to delete review: {}", e)))?;

    let response = ApiResponse {
        success: true,
        message: "Review deleted successfully".to_string(),
        data: ReviewDeletedData {
            deleted: true,
            review_id,
        },
    };

    Ok((StatusCode::OK, Json(response)))
}

// ---------------------------------------------------------------------------
// POST /api/reviews/carecrew/{review_id}/reply — Reply to CareCrew Review
// ---------------------------------------------------------------------------

pub async fn reply_to_carecrew_review(
    State(app_state): State<AppState>,
    auth: AuthenticationUser,
    Path(review_id): Path<Uuid>,
    Json(payload): Json<ReplyRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let user_id = Uuid::parse_str(&auth.user_id)
        .map_err(|_| ApiError::Unauthorized("Invalid user token".to_string()))?;

    // 1. Review must exist
    let review_row: Option<(Uuid, Option<String>)> = sqlx::query_as(
        "SELECT provider_id, reply FROM carecrew_reviews WHERE id = $1",
    )
    .bind(review_id)
    .fetch_optional(&app_state.db)
    .await
    .map_err(|e| ApiError::InternalServerError(format!("Database error: {}", e)))?;

    let (provider_id, existing_reply) = match review_row {
        Some(row) => row,
        None => {
            return Err(ApiError::CustomError(
                StatusCode::NOT_FOUND,
                "Review not found".to_string(),
                "REVIEW_NOT_FOUND".to_string(),
            ));
        }
    };

    // 2. Must be the provider (check carecrew_providers.user_id)
    let provider_user: Option<(Uuid,)> = sqlx::query_as(
        "SELECT user_id FROM carecrew_providers WHERE id = $1",
    )
    .bind(provider_id)
    .fetch_optional(&app_state.db)
    .await
    .map_err(|e| ApiError::InternalServerError(format!("Database error: {}", e)))?;

    let is_owner = match provider_user {
        Some((uid,)) => uid == user_id,
        None => false,
    };

    if !is_owner {
        return Err(ApiError::CustomError(
            StatusCode::FORBIDDEN,
            "Only the reviewed provider can reply".to_string(),
            "ACCESS_DENIED".to_string(),
        ));
    }

    // 3. No existing reply
    if existing_reply.is_some() {
        return Err(ApiError::CustomError(
            StatusCode::CONFLICT,
            "A reply already exists for this review".to_string(),
            "REPLY_ALREADY_EXISTS".to_string(),
        ));
    }

    let now = Utc::now();

    sqlx::query(
        "UPDATE carecrew_reviews SET reply = $1, reply_at = $2 WHERE id = $3",
    )
    .bind(&payload.reply)
    .bind(now)
    .bind(review_id)
    .execute(&app_state.db)
    .await
    .map_err(|e| ApiError::InternalServerError(format!("Failed to add reply: {}", e)))?;

    let response = ApiResponse {
        success: true,
        message: "Reply added successfully".to_string(),
        data: ReviewReplyData {
            review_id,
            reply: payload.reply,
            replied_at: now.to_rfc3339(),
        },
    };

    Ok((StatusCode::CREATED, Json(response)))
}
