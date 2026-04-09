// src/handlers/carecrew_reviews.rs
//
// CareCrew Review & Rating APIs:
//   POST   /api/reviews/carecrew                    — Submit Review
//   GET    /api/reviews/carecrew/{provider_id}      — Get Provider Reviews
//   PUT    /api/reviews/carecrew/{review_id}        — Edit Review
//   DELETE /api/reviews/carecrew/{review_id}        — Delete Review
//   POST   /api/reviews/carecrew/{review_id}/reply  — Reply to Review

use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use chrono::Utc;
use sqlx::Row;
use uuid::Uuid;

use crate::{
    app_state::AppState,
    dtos::{
        carecrew_review::{
            CreateCarecrewReviewRequest, CreateReviewData, DeleteReviewData,
            EditCarecrewReviewRequest, EditReviewData, ReplyData, ReplyRequest, ReviewBreakdown,
            ReviewItem, ReviewSummary, ReviewsListData, ReviewsQuery,
        },
        response::ApiResponse,
    },
    utils::{api_error::ApiError, auth_extractor::AuthenticationUser},
};

fn validate_rating(rating: f64) -> Result<(), ApiError> {
    if rating < 1.0 || rating > 5.0 {
        return Err(ApiError::invalid_rating());
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// POST /api/reviews/carecrew — Submit CareCrew Review
// ---------------------------------------------------------------------------

pub async fn create_review(
    State(app_state): State<AppState>,
    auth: AuthenticationUser,
    Json(body): Json<CreateCarecrewReviewRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let reviewer_id = Uuid::parse_str(&auth.user_id)
        .map_err(|_| ApiError::Unauthorized("Invalid user token".to_string()))?;

    validate_rating(body.rating)?;

    let booking_id = match Uuid::parse_str(&body.booking_id) {
        Ok(u) => u,
        Err(_) => return Err(ApiError::BadRequest("Invalid booking_id format".to_string())),
    };

    let provider_id = match Uuid::parse_str(&body.provider_id) {
        Ok(u) => u,
        Err(_) => return Err(ApiError::BadRequest("Invalid provider_id format".to_string())),
    };

    // 1. Check booking exists and is completed
    let booking_row: Option<(String,)> =
        sqlx::query_as("SELECT status FROM carecrew_bookings WHERE id = $1 AND user_id = $2")
            .bind(booking_id)
            .bind(reviewer_id)
            .fetch_optional(&app_state.db)
            .await
            .map_err(|e| ApiError::InternalServerError(format!("Database error: {}", e)))?;

    match booking_row {
        None => return Err(ApiError::NotFound("Booking not found".to_string())),
        Some((status,)) if status != "completed" => {
            return Err(ApiError::booking_not_completed());
        }
        _ => {}
    }

    // 2. Check for duplicate review
    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM carecrew_reviews WHERE booking_id = $1)")
            .bind(booking_id)
            .fetch_one(&app_state.db)
            .await
            .map_err(|e| ApiError::InternalServerError(format!("Database error: {}", e)))?;

    if exists {
        return Err(ApiError::review_already_exists());
    }

    // 3. Insert review
    let review_id = Uuid::new_v4();
    let now = Utc::now();

    sqlx::query(
        r#"
        INSERT INTO carecrew_reviews (id, booking_id, provider_id, reviewer_id, rating, comment, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $7)
        "#,
    )
    .bind(review_id)
    .bind(booking_id)
    .bind(provider_id)
    .bind(reviewer_id)
    .bind(body.rating)
    .bind(&body.comment)
    .bind(now)
    .execute(&app_state.db)
    .await
    .map_err(|e| ApiError::InternalServerError(format!("Failed to create review: {}", e)))?;

    let response = ApiResponse {
        success: true,
        message: "Review submitted successfully".to_string(),
        data: CreateReviewData {
            review_id,
            booking_id,
            provider_id,
            reviewer_id,
            rating: body.rating,
            comment: body.comment,
            created_at: now,
        },
    };

    Ok((StatusCode::CREATED, Json(response)))
}

// ---------------------------------------------------------------------------
// GET /api/reviews/carecrew/{provider_id} — Get Provider Reviews
// ---------------------------------------------------------------------------

pub async fn get_provider_reviews(
    State(app_state): State<AppState>,
    _auth: AuthenticationUser,
    Path(provider_id): Path<Uuid>,
    Query(params): Query<ReviewsQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let limit = params.limit.unwrap_or(10).min(100).max(1);
    let offset = params.offset.unwrap_or(0).max(0);

    // Total count
    let total_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM carecrew_reviews WHERE provider_id = $1")
            .bind(provider_id)
            .fetch_one(&app_state.db)
            .await
            .map_err(|e| ApiError::InternalServerError(format!("Database error: {}", e)))?;

    // Summary with rating breakdown using FILTER
    let summary_row = sqlx::query(
        r#"
        SELECT
            COALESCE(AVG(rating), 0.0)::FLOAT8                              AS avg_rating,
            COUNT(*)                                                         AS total,
            COUNT(*) FILTER (WHERE rating >= 4.5)                           AS five_star,
            COUNT(*) FILTER (WHERE rating >= 3.5 AND rating < 4.5)          AS four_star,
            COUNT(*) FILTER (WHERE rating >= 2.5 AND rating < 3.5)          AS three_star,
            COUNT(*) FILTER (WHERE rating >= 1.5 AND rating < 2.5)          AS two_star,
            COUNT(*) FILTER (WHERE rating < 1.5)                            AS one_star
        FROM carecrew_reviews
        WHERE provider_id = $1
        "#,
    )
    .bind(provider_id)
    .fetch_one(&app_state.db)
    .await
    .map_err(|e| ApiError::InternalServerError(format!("Database error: {}", e)))?;

    // Paginated reviews with reviewer info
    let review_rows = sqlx::query(
        r#"
        SELECT
            cr.id,
            CONCAT(COALESCE(u.first_name, ''), ' ', COALESCE(u.last_name, '')) AS reviewer_name,
            u.profile_image_url                                                 AS reviewer_image,
            cr.rating::FLOAT8,
            cr.comment,
            cr.reply,
            cr.reply_at,
            cr.created_at                                                       AS review_date
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

    let reviews: Vec<ReviewItem> = review_rows
        .iter()
        .map(|r| ReviewItem {
            id: r.get("id"),
            reviewer_name: r
                .get::<Option<String>, _>("reviewer_name")
                .unwrap_or_default(),
            reviewer_image: r.get("reviewer_image"),
            rating: r.get("rating"),
            comment: r.get("comment"),
            reply: r.get("reply"),
            reply_at: r.get("reply_at"),
            review_date: r.get("review_date"),
        })
        .collect();

    let total_pages = if total_count == 0 {
        0
    } else {
        (total_count as f64 / limit as f64).ceil() as i64
    };
    let current_page = (offset / limit) + 1;

    let response = ApiResponse {
        success: true,
        message: "Reviews retrieved successfully".to_string(),
        data: ReviewsListData {
            reviews,
            summary: ReviewSummary {
                average_rating: summary_row.get::<f64, _>("avg_rating"),
                total_reviews: summary_row.get("total"),
                breakdown: ReviewBreakdown {
                    five: summary_row.get("five_star"),
                    four: summary_row.get("four_star"),
                    three: summary_row.get("three_star"),
                    two: summary_row.get("two_star"),
                    one: summary_row.get("one_star"),
                },
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

pub async fn edit_review(
    State(app_state): State<AppState>,
    auth: AuthenticationUser,
    Path(review_id): Path<Uuid>,
    Json(body): Json<EditCarecrewReviewRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let user_id = Uuid::parse_str(&auth.user_id)
        .map_err(|_| ApiError::Unauthorized("Invalid user token".to_string()))?;

    if let Some(r) = body.rating {
        validate_rating(r)?;
    }

    // 1. Review must exist
    let review_row = sqlx::query(
        "SELECT reviewer_id, rating::FLOAT8, comment, created_at FROM carecrew_reviews WHERE id = $1",
    )
    .bind(review_id)
    .fetch_optional(&app_state.db)
    .await
    .map_err(|e| ApiError::InternalServerError(format!("Database error: {}", e)))?
    .ok_or_else(ApiError::review_not_found)?;

    let reviewer_id: Uuid = review_row.get("reviewer_id");
    let created_at: chrono::DateTime<Utc> = review_row.get("created_at");
    let current_rating: f64 = review_row.get("rating");
    let current_comment: Option<String> = review_row.get("comment");

    // 2. Must be the reviewer
    if user_id != reviewer_id {
        return Err(ApiError::access_denied());
    }

    // 3. Within 30 days
    if (Utc::now() - created_at).num_days() > 30 {
        return Err(ApiError::edit_period_expired());
    }

    let new_rating = body.rating.unwrap_or(current_rating);
    let new_comment = body.comment.or(current_comment);
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
        data: EditReviewData {
            review_id,
            rating: new_rating,
            comment: new_comment,
            updated_at: now,
        },
    };

    Ok((StatusCode::OK, Json(response)))
}

// ---------------------------------------------------------------------------
// DELETE /api/reviews/carecrew/{review_id} — Delete CareCrew Review
// ---------------------------------------------------------------------------

pub async fn delete_review(
    State(app_state): State<AppState>,
    auth: AuthenticationUser,
    Path(review_id): Path<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    let user_id = Uuid::parse_str(&auth.user_id)
        .map_err(|_| ApiError::Unauthorized("Invalid user token".to_string()))?;

    let review_row =
        sqlx::query("SELECT reviewer_id, created_at FROM carecrew_reviews WHERE id = $1")
            .bind(review_id)
            .fetch_optional(&app_state.db)
            .await
            .map_err(|e| ApiError::InternalServerError(format!("Database error: {}", e)))?
            .ok_or_else(ApiError::review_not_found)?;

    let reviewer_id: Uuid = review_row.get("reviewer_id");
    let created_at: chrono::DateTime<Utc> = review_row.get("created_at");

    if user_id != reviewer_id {
        return Err(ApiError::access_denied());
    }

    if (Utc::now() - created_at).num_days() > 30 {
        return Err(ApiError::edit_period_expired());
    }

    sqlx::query("DELETE FROM carecrew_reviews WHERE id = $1")
        .bind(review_id)
        .execute(&app_state.db)
        .await
        .map_err(|e| ApiError::InternalServerError(format!("Failed to delete review: {}", e)))?;

    let response = ApiResponse {
        success: true,
        message: "Review deleted successfully".to_string(),
        data: DeleteReviewData {
            deleted: true,
            review_id,
        },
    };

    Ok((StatusCode::OK, Json(response)))
}

// ---------------------------------------------------------------------------
// POST /api/reviews/carecrew/{review_id}/reply — Reply to CareCrew Review
// ---------------------------------------------------------------------------

pub async fn reply_to_review(
    State(app_state): State<AppState>,
    auth: AuthenticationUser,
    Path(review_id): Path<Uuid>,
    Json(body): Json<ReplyRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let user_id = Uuid::parse_str(&auth.user_id)
        .map_err(|_| ApiError::Unauthorized("Invalid user token".to_string()))?;

    // Review must exist
    let review_row = sqlx::query("SELECT provider_id, reply FROM carecrew_reviews WHERE id = $1")
        .bind(review_id)
        .fetch_optional(&app_state.db)
        .await
        .map_err(|e| ApiError::InternalServerError(format!("Database error: {}", e)))?
        .ok_or_else(ApiError::review_not_found)?;

    let provider_id: Uuid = review_row.get("provider_id");
    let existing_reply: Option<String> = review_row.get("reply");

    // Must be the provider — check carecrew_providers.user_id
    let provider_user: Option<(Option<Uuid>,)> =
        sqlx::query_as("SELECT user_id FROM carecrew_providers WHERE id = $1")
            .bind(provider_id)
            .fetch_optional(&app_state.db)
            .await
            .map_err(|e| ApiError::InternalServerError(format!("Database error: {}", e)))?;

    let is_owner = match provider_user {
        Some((Some(uid),)) => uid == user_id,
        _ => false,
    };

    if !is_owner {
        return Err(ApiError::access_denied());
    }

    if existing_reply.is_some() {
        return Err(ApiError::reply_already_exists());
    }

    let now = Utc::now();

    sqlx::query("UPDATE carecrew_reviews SET reply = $1, reply_at = $2 WHERE id = $3")
        .bind(&body.reply)
        .bind(now)
        .bind(review_id)
        .execute(&app_state.db)
        .await
        .map_err(|e| ApiError::InternalServerError(format!("Failed to add reply: {}", e)))?;

    let response = ApiResponse {
        success: true,
        message: "Reply added successfully".to_string(),
        data: ReplyData {
            review_id,
            reply: body.reply,
            replied_at: now,
        },
    };

    Ok((StatusCode::CREATED, Json(response)))
}
