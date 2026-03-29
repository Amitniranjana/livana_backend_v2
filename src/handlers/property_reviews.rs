// src/handlers/property_reviews.rs
//
// Property Review & Rating APIs:
//   POST   /api/reviews/property                     — Submit Review
//   GET    /api/reviews/property/{property_id}       — Get Property Reviews
//   PUT    /api/reviews/property/{review_id}         — Edit Review
//   DELETE /api/reviews/property/{review_id}         — Delete Review
//   POST   /api/reviews/property/{review_id}/reply   — Reply to Review

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
        carecrew_review::{DeleteReviewData, ReplyData, ReplyRequest, ReviewsQuery},
        property_review::{
            CreatePropertyReviewData, CreatePropertyReviewRequest, EditPropertyReviewData,
            EditPropertyReviewRequest, PropertyReviewBreakdown, PropertyReviewItem,
            PropertyReviewSummary, PropertyReviewsListData,
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

fn validate_optional_rating(rating: Option<f64>) -> Result<(), ApiError> {
    if let Some(r) = rating {
        validate_rating(r)?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// POST /api/reviews/property — Submit Property Review
// ---------------------------------------------------------------------------

pub async fn create_review(
    State(app_state): State<AppState>,
    auth: AuthenticationUser,
    Json(body): Json<CreatePropertyReviewRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let reviewer_id = Uuid::parse_str(&auth.user_id)
        .map_err(|_| ApiError::Unauthorized("Invalid user token".to_string()))?;

    validate_rating(body.rating)?;
    validate_optional_rating(body.location_rating)?;
    validate_optional_rating(body.cleanliness_rating)?;
    validate_optional_rating(body.value_rating)?;

    // 1. Check visit is completed (table: site_visits)
    let visit_row: Option<(String,)> =
        sqlx::query_as("SELECT status FROM site_visits WHERE id = $1 AND user_id = $2")
            .bind(body.visit_id)
            .bind(reviewer_id)
            .fetch_optional(&app_state.db)
            .await
            .map_err(|e| ApiError::InternalServerError(format!("Database error: {}", e)))?;

    match visit_row {
        None => return Err(ApiError::NotFound("Visit not found".to_string())),
        Some((status,)) if status != "completed" => {
            return Err(ApiError::visit_not_completed());
        }
        _ => {}
    }

    // 2. Check for duplicate review
    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM property_reviews WHERE visit_id = $1)")
            .bind(body.visit_id)
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
        INSERT INTO property_reviews
            (id, visit_id, property_id, reviewer_id, rating, comment,
             location_rating, cleanliness_rating, value_rating, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $10)
        "#,
    )
    .bind(review_id)
    .bind(body.visit_id)
    .bind(body.property_id)
    .bind(reviewer_id)
    .bind(body.rating)
    .bind(&body.comment)
    .bind(body.location_rating)
    .bind(body.cleanliness_rating)
    .bind(body.value_rating)
    .bind(now)
    .execute(&app_state.db)
    .await
    .map_err(|e| ApiError::InternalServerError(format!("Failed to create review: {}", e)))?;

    let response = ApiResponse {
        success: true,
        message: "Review submitted successfully".to_string(),
        data: CreatePropertyReviewData {
            review_id,
            visit_id: body.visit_id,
            property_id: body.property_id,
            reviewer_id,
            rating: body.rating,
            comment: body.comment,
            location_rating: body.location_rating,
            cleanliness_rating: body.cleanliness_rating,
            value_rating: body.value_rating,
            created_at: now,
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
    Query(params): Query<ReviewsQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let limit = params.limit.unwrap_or(10).min(100).max(1);
    let offset = params.offset.unwrap_or(0).max(0);

    // Total count
    let total_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM property_reviews WHERE property_id = $1")
            .bind(property_id)
            .fetch_one(&app_state.db)
            .await
            .map_err(|e| ApiError::InternalServerError(format!("Database error: {}", e)))?;

    // Summary with sub-rating averages and breakdown
    let summary_row = sqlx::query(
        r#"
        SELECT
            COALESCE(AVG(rating), 0.0)::FLOAT8              AS avg_rating,
            COUNT(*)                                        AS total,
            AVG(location_rating)::FLOAT8                    AS avg_location,
            AVG(cleanliness_rating)::FLOAT8                 AS avg_cleanliness,
            AVG(value_rating)::FLOAT8                       AS avg_value,
            COUNT(*) FILTER (WHERE rating >= 4.5)           AS five_star,
            COUNT(*) FILTER (WHERE rating >= 3.5 AND rating < 4.5) AS four_star,
            COUNT(*) FILTER (WHERE rating >= 2.5 AND rating < 3.5) AS three_star,
            COUNT(*) FILTER (WHERE rating >= 1.5 AND rating < 2.5) AS two_star,
            COUNT(*) FILTER (WHERE rating < 1.5)            AS one_star
        FROM property_reviews
        WHERE property_id = $1
        "#,
    )
    .bind(property_id)
    .fetch_one(&app_state.db)
    .await
    .map_err(|e| ApiError::InternalServerError(format!("Database error: {}", e)))?;

    // Paginated reviews with reviewer info
    let review_rows = sqlx::query(
        r#"
        SELECT
            pr.id,
            CONCAT(COALESCE(u.first_name, ''), ' ', COALESCE(u.last_name, '')) AS reviewer_name,
            u.profile_image_url                                                 AS reviewer_image,
            pr.rating::FLOAT8,
            pr.comment,
            pr.location_rating::FLOAT8,
            pr.cleanliness_rating::FLOAT8,
            pr.value_rating::FLOAT8,
            pr.reply,
            pr.reply_at,
            pr.created_at                                                       AS review_date
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
        .iter()
        .map(|r| PropertyReviewItem {
            id: r.get("id"),
            reviewer_name: r
                .get::<Option<String>, _>("reviewer_name")
                .unwrap_or_default(),
            reviewer_image: r.get("reviewer_image"),
            rating: r.get("rating"),
            comment: r.get("comment"),
            location_rating: r.get("location_rating"),
            cleanliness_rating: r.get("cleanliness_rating"),
            value_rating: r.get("value_rating"),
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
        data: PropertyReviewsListData {
            reviews,
            summary: PropertyReviewSummary {
                average_rating: summary_row.get::<f64, _>("avg_rating"),
                total_reviews: summary_row.get("total"),
                average_location_rating: summary_row.get("avg_location"),
                average_cleanliness_rating: summary_row.get("avg_cleanliness"),
                average_value_rating: summary_row.get("avg_value"),
                breakdown: PropertyReviewBreakdown {
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
// PUT /api/reviews/property/{review_id} — Edit Property Review
// ---------------------------------------------------------------------------

pub async fn edit_review(
    State(app_state): State<AppState>,
    auth: AuthenticationUser,
    Path(review_id): Path<Uuid>,
    Json(body): Json<EditPropertyReviewRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let user_id = Uuid::parse_str(&auth.user_id)
        .map_err(|_| ApiError::Unauthorized("Invalid user token".to_string()))?;

    if let Some(r) = body.rating {
        validate_rating(r)?;
    }
    validate_optional_rating(body.location_rating)?;
    validate_optional_rating(body.cleanliness_rating)?;
    validate_optional_rating(body.value_rating)?;

    let review_row = sqlx::query(
        r#"
        SELECT reviewer_id, rating::FLOAT8, comment,
               location_rating::FLOAT8, cleanliness_rating::FLOAT8, value_rating::FLOAT8, created_at
        FROM property_reviews WHERE id = $1
        "#,
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
    let cur_loc: Option<f64> = review_row.get("location_rating");
    let cur_clean: Option<f64> = review_row.get("cleanliness_rating");
    let cur_val: Option<f64> = review_row.get("value_rating");

    if user_id != reviewer_id {
        return Err(ApiError::access_denied());
    }
    if (Utc::now() - created_at).num_days() > 30 {
        return Err(ApiError::edit_period_expired());
    }

    let new_rating = body.rating.unwrap_or(current_rating);
    let new_comment = body.comment.or(current_comment);
    let new_loc = body.location_rating.or(cur_loc);
    let new_clean = body.cleanliness_rating.or(cur_clean);
    let new_val = body.value_rating.or(cur_val);
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
        data: EditPropertyReviewData {
            review_id,
            rating: new_rating,
            comment: new_comment,
            location_rating: new_loc,
            cleanliness_rating: new_clean,
            value_rating: new_val,
            updated_at: now,
        },
    };

    Ok((StatusCode::OK, Json(response)))
}

// ---------------------------------------------------------------------------
// DELETE /api/reviews/property/{review_id} — Delete Property Review
// ---------------------------------------------------------------------------

pub async fn delete_review(
    State(app_state): State<AppState>,
    auth: AuthenticationUser,
    Path(review_id): Path<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    let user_id = Uuid::parse_str(&auth.user_id)
        .map_err(|_| ApiError::Unauthorized("Invalid user token".to_string()))?;

    let review_row =
        sqlx::query("SELECT reviewer_id, created_at FROM property_reviews WHERE id = $1")
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

    sqlx::query("DELETE FROM property_reviews WHERE id = $1")
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
// POST /api/reviews/property/{review_id}/reply — Reply to Property Review
// ---------------------------------------------------------------------------

pub async fn reply_to_review(
    State(app_state): State<AppState>,
    auth: AuthenticationUser,
    Path(review_id): Path<Uuid>,
    Json(body): Json<ReplyRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let user_id = Uuid::parse_str(&auth.user_id)
        .map_err(|_| ApiError::Unauthorized("Invalid user token".to_string()))?;

    // Review with property owner check via JOIN
    let review_row = sqlx::query(
        r#"
        SELECT pr.reply, p.user_id AS owner_id
        FROM property_reviews pr
        JOIN properties p ON p.id = pr.property_id
        WHERE pr.id = $1
        "#,
    )
    .bind(review_id)
    .fetch_optional(&app_state.db)
    .await
    .map_err(|e| ApiError::InternalServerError(format!("Database error: {}", e)))?
    .ok_or_else(ApiError::review_not_found)?;

    let owner_id: Option<Uuid> = review_row.get("owner_id");
    let existing_reply: Option<String> = review_row.get("reply");

    let is_owner = match owner_id {
        Some(uid) => uid == user_id,
        None => false,
    };

    if !is_owner {
        return Err(ApiError::access_denied());
    }
    if existing_reply.is_some() {
        return Err(ApiError::reply_already_exists());
    }

    let now = Utc::now();

    sqlx::query("UPDATE property_reviews SET reply = $1, reply_at = $2 WHERE id = $3")
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
