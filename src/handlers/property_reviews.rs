// src/handlers/property_reviews.rs
//
// Property Review & Rating APIs:
//   POST   /api/reviews/property                     — Submit Review
//   GET    /api/reviews/property/{property_id}       — Get Property Reviews
//   PUT    /api/reviews/property/{review_id}         — Edit Review
//   DELETE /api/reviews/property/{review_id}         — Delete Review
//   POST   /api/reviews/property/{review_id}/reply   — Reply to Review

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
            RatingBreakdown, ReplyRequest, ReviewDeletedData,
            ReviewListQuery, ReviewReplyData,
        },
        property_review::{
            CreatePropertyReviewRequest, EditPropertyReviewRequest,
            PropertyReviewCreatedData, PropertyReviewItem,
            PropertyReviewSummary, PropertyReviewUpdatedData,
            PropertyReviewsListData,
        },
        response::ApiResponse,
    },
    utils::{api_error::ApiError, auth_extractor::AuthenticationUser},
};

fn validate_rating(rating: f64, field_name: &str) -> Result<(), ApiError> {
    if !(1.0..=5.0).contains(&rating) {
        return Err(ApiError::CustomError(
            StatusCode::BAD_REQUEST,
            format!("{} must be between 1.0 and 5.0", field_name),
            "INVALID_RATING".to_string(),
        ));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// POST /api/reviews/property — Submit Property Review
// ---------------------------------------------------------------------------

pub async fn create_property_review(
    State(app_state): State<AppState>,
    auth: AuthenticationUser,
    Json(payload): Json<CreatePropertyReviewRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let reviewer_id = Uuid::parse_str(&auth.user_id)
        .map_err(|_| ApiError::Unauthorized("Invalid user token".to_string()))?;

    // 1. Check visit is completed
    let visit_row: Option<(String,)> = sqlx::query_as(
        "SELECT status FROM site_visits WHERE id = $1",
    )
    .bind(payload.visit_id)
    .fetch_optional(&app_state.db)
    .await
    .map_err(|e| ApiError::InternalServerError(format!("Database error: {}", e)))?;

    match visit_row {
        Some((status,)) => {
            if status != "completed" {
                return Err(ApiError::CustomError(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "Visit must be completed before submitting a review".to_string(),
                    "VISIT_NOT_COMPLETED".to_string(),
                ));
            }
        }
        None => {
            return Err(ApiError::NotFound("Visit not found".to_string()));
        }
    }

    // 2. Check for existing review on this visit
    let existing: Option<(Uuid,)> = sqlx::query_as(
        "SELECT id FROM property_reviews WHERE visit_id = $1",
    )
    .bind(payload.visit_id)
    .fetch_optional(&app_state.db)
    .await
    .map_err(|e| ApiError::InternalServerError(format!("Database error: {}", e)))?;

    if existing.is_some() {
        return Err(ApiError::CustomError(
            StatusCode::CONFLICT,
            "A review already exists for this visit".to_string(),
            "REVIEW_ALREADY_EXISTS".to_string(),
        ));
    }

    // 3. Validate rating
    validate_rating(payload.rating, "rating")?;

    // 4. Validate sub-ratings if provided
    if let Some(lr) = payload.location_rating {
        validate_rating(lr, "location_rating")?;
    }
    if let Some(cr) = payload.cleanliness_rating {
        validate_rating(cr, "cleanliness_rating")?;
    }
    if let Some(vr) = payload.value_rating {
        validate_rating(vr, "value_rating")?;
    }

    // 5. Insert review
    let review_id = Uuid::new_v4();
    let now = Utc::now();

    sqlx::query(
        r#"
        INSERT INTO property_reviews
            (id, visit_id, property_id, reviewer_id, rating, comment,
             location_rating, cleanliness_rating, value_rating, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $10)
        "#,
    )
    .bind(review_id)
    .bind(payload.visit_id)
    .bind(payload.property_id)
    .bind(reviewer_id)
    .bind(payload.rating)
    .bind(&payload.comment)
    .bind(payload.location_rating)
    .bind(payload.cleanliness_rating)
    .bind(payload.value_rating)
    .bind(now)
    .execute(&app_state.db)
    .await
    .map_err(|e| ApiError::InternalServerError(format!("Failed to create review: {}", e)))?;

    let response = ApiResponse {
        success: true,
        message: "Review submitted successfully".to_string(),
        data: PropertyReviewCreatedData {
            review_id,
            visit_id: payload.visit_id,
            property_id: payload.property_id,
            reviewer_id,
            rating: payload.rating,
            comment: payload.comment,
            location_rating: payload.location_rating,
            cleanliness_rating: payload.cleanliness_rating,
            value_rating: payload.value_rating,
            created_at: now.to_rfc3339(),
        },
    };

    Ok((StatusCode::CREATED, Json(response)))
}

// ---------------------------------------------------------------------------
// GET /api/reviews/property/{property_id} — Get Property Reviews
// ---------------------------------------------------------------------------

pub async fn get_property_reviews(
    State(app_state): State<AppState>,
    _auth: AuthenticationUser,
    Path(property_id): Path<Uuid>,
    Query(params): Query<ReviewListQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let limit = params.limit.unwrap_or(10).clamp(1, 100);
    let offset = params.offset.unwrap_or(0).max(0);

    // Total count
    let total_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM property_reviews WHERE property_id = $1",
    )
    .bind(property_id)
    .fetch_one(&app_state.db)
    .await
    .map_err(|e| ApiError::InternalServerError(format!("Database error: {}", e)))?;

    // Summary aggregation
    let summary_row: Option<(Option<f64>, Option<f64>, Option<f64>, Option<f64>)> = sqlx::query_as(
        r#"
        SELECT
            AVG(rating)::FLOAT8,
            AVG(location_rating)::FLOAT8,
            AVG(cleanliness_rating)::FLOAT8,
            AVG(value_rating)::FLOAT8
        FROM property_reviews
        WHERE property_id = $1
        "#,
    )
    .bind(property_id)
    .fetch_optional(&app_state.db)
    .await
    .map_err(|e| ApiError::InternalServerError(format!("Database error: {}", e)))?;

    let (avg_rating, avg_location, avg_cleanliness, avg_value) =
        summary_row.unwrap_or((None, None, None, None));

    // Rating breakdown
    let breakdown_rows: Vec<(i32, i64)> = sqlx::query_as(
        r#"
        SELECT FLOOR(rating)::INT as star, COUNT(*) as cnt
        FROM property_reviews
        WHERE property_id = $1
        GROUP BY star
        "#,
    )
    .bind(property_id)
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
    let review_rows: Vec<(Uuid, Option<String>, Option<String>, f64, Option<String>, Option<f64>, Option<f64>, Option<f64>, Option<String>, chrono::DateTime<chrono::Utc>)> = sqlx::query_as(
        r#"
        SELECT
            pr.id,
            CONCAT(COALESCE(u.first_name, ''), ' ', COALESCE(u.last_name, '')) as reviewer_name,
            u.profile_image_url as reviewer_image,
            pr.rating::FLOAT8,
            pr.comment,
            pr.location_rating::FLOAT8,
            pr.cleanliness_rating::FLOAT8,
            pr.value_rating::FLOAT8,
            pr.reply,
            pr.created_at
        FROM property_reviews pr
        LEFT JOIN users u ON u.id = pr.reviewer_id
        WHERE pr.property_id = $1
        ORDER BY pr.created_at DESC
        LIMIT $2 OFFSET $3
        "#,
    )
    .bind(property_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(&app_state.db)
    .await
    .map_err(|e| ApiError::InternalServerError(format!("Database error: {}", e)))?;

    let reviews: Vec<PropertyReviewItem> = review_rows
        .into_iter()
        .map(|(id, reviewer_name, reviewer_image, rating, comment, location_rating, cleanliness_rating, value_rating, reply, created_at)| {
            PropertyReviewItem {
                id,
                reviewer_name: reviewer_name.unwrap_or_default(),
                reviewer_image,
                rating,
                comment,
                location_rating,
                cleanliness_rating,
                value_rating,
                reply,
                review_date: created_at.to_rfc3339(),
            }
        })
        .collect();

    let current_page = (offset / limit) + 1;
    let total_pages = if total_count == 0 { 0 } else { (total_count + limit - 1) / limit };

    let response = ApiResponse {
        success: true,
        message: "Reviews retrieved successfully".to_string(),
        data: PropertyReviewsListData {
            reviews,
            summary: PropertyReviewSummary {
                average_rating: avg_rating.unwrap_or(0.0),
                total_reviews: total_count,
                average_location_rating: avg_location,
                average_cleanliness_rating: avg_cleanliness,
                average_value_rating: avg_value,
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
// PUT /api/reviews/property/{review_id} — Edit Property Review
// ---------------------------------------------------------------------------

pub async fn edit_property_review(
    State(app_state): State<AppState>,
    auth: AuthenticationUser,
    Path(review_id): Path<Uuid>,
    Json(payload): Json<EditPropertyReviewRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let user_id = Uuid::parse_str(&auth.user_id)
        .map_err(|_| ApiError::Unauthorized("Invalid user token".to_string()))?;

    // 1. Review must exist
    let review_row: Option<(Uuid, chrono::DateTime<chrono::Utc>, f64, Option<String>, Option<f64>, Option<f64>, Option<f64>)> = sqlx::query_as(
        r#"
        SELECT reviewer_id, created_at, rating::FLOAT8, comment,
               location_rating::FLOAT8, cleanliness_rating::FLOAT8, value_rating::FLOAT8
        FROM property_reviews WHERE id = $1
        "#,
    )
    .bind(review_id)
    .fetch_optional(&app_state.db)
    .await
    .map_err(|e| ApiError::InternalServerError(format!("Database error: {}", e)))?;

    let (reviewer_id, created_at, current_rating, current_comment, cur_loc, cur_clean, cur_val) = match review_row {
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

    // 4. Validate ratings
    let new_rating = payload.rating.unwrap_or(current_rating);
    validate_rating(new_rating, "rating")?;

    let new_loc = payload.location_rating.or(cur_loc);
    if let Some(lr) = new_loc {
        validate_rating(lr, "location_rating")?;
    }
    let new_clean = payload.cleanliness_rating.or(cur_clean);
    if let Some(cr) = new_clean {
        validate_rating(cr, "cleanliness_rating")?;
    }
    let new_val = payload.value_rating.or(cur_val);
    if let Some(vr) = new_val {
        validate_rating(vr, "value_rating")?;
    }

    let new_comment = payload.comment.or(current_comment);
    let now = Utc::now();

    sqlx::query(
        r#"
        UPDATE property_reviews
        SET rating = $1, comment = $2, location_rating = $3,
            cleanliness_rating = $4, value_rating = $5, updated_at = $6
        WHERE id = $7
        "#,
    )
    .bind(new_rating)
    .bind(&new_comment)
    .bind(new_loc)
    .bind(new_clean)
    .bind(new_val)
    .bind(now)
    .bind(review_id)
    .execute(&app_state.db)
    .await
    .map_err(|e| ApiError::InternalServerError(format!("Failed to update review: {}", e)))?;

    let response = ApiResponse {
        success: true,
        message: "Review updated successfully".to_string(),
        data: PropertyReviewUpdatedData {
            review_id,
            rating: new_rating,
            comment: new_comment,
            location_rating: new_loc,
            cleanliness_rating: new_clean,
            value_rating: new_val,
            updated_at: now.to_rfc3339(),
        },
    };

    Ok((StatusCode::OK, Json(response)))
}

// ---------------------------------------------------------------------------
// DELETE /api/reviews/property/{review_id} — Delete Property Review
// ---------------------------------------------------------------------------

pub async fn delete_property_review(
    State(app_state): State<AppState>,
    auth: AuthenticationUser,
    Path(review_id): Path<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    let user_id = Uuid::parse_str(&auth.user_id)
        .map_err(|_| ApiError::Unauthorized("Invalid user token".to_string()))?;

    // 1. Review must exist
    let review_row: Option<(Uuid, chrono::DateTime<chrono::Utc>)> = sqlx::query_as(
        "SELECT reviewer_id, created_at FROM property_reviews WHERE id = $1",
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

    sqlx::query("DELETE FROM property_reviews WHERE id = $1")
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
// POST /api/reviews/property/{review_id}/reply — Reply to Property Review
// ---------------------------------------------------------------------------

pub async fn reply_to_property_review(
    State(app_state): State<AppState>,
    auth: AuthenticationUser,
    Path(review_id): Path<Uuid>,
    Json(payload): Json<ReplyRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let user_id = Uuid::parse_str(&auth.user_id)
        .map_err(|_| ApiError::Unauthorized("Invalid user token".to_string()))?;

    // 1. Review must exist
    let review_row: Option<(Uuid, Option<String>)> = sqlx::query_as(
        "SELECT property_id, reply FROM property_reviews WHERE id = $1",
    )
    .bind(review_id)
    .fetch_optional(&app_state.db)
    .await
    .map_err(|e| ApiError::InternalServerError(format!("Database error: {}", e)))?;

    let (property_id, existing_reply) = match review_row {
        Some(row) => row,
        None => {
            return Err(ApiError::CustomError(
                StatusCode::NOT_FOUND,
                "Review not found".to_string(),
                "REVIEW_NOT_FOUND".to_string(),
            ));
        }
    };

    // 2. Must be the property owner — check properties.user_id
    let owner_row: Option<(Uuid,)> = sqlx::query_as(
        "SELECT user_id FROM properties WHERE id = $1",
    )
    .bind(property_id)
    .fetch_optional(&app_state.db)
    .await
    .map_err(|e| ApiError::InternalServerError(format!("Database error: {}", e)))?;

    let is_owner = match owner_row {
        Some((owner_id,)) => owner_id == user_id,
        None => false,
    };

    if !is_owner {
        return Err(ApiError::CustomError(
            StatusCode::FORBIDDEN,
            "Only the property owner can reply to reviews".to_string(),
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
        "UPDATE property_reviews SET reply = $1, reply_at = $2 WHERE id = $3",
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
