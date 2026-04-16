// src/handlers/community.rs
//
// Module 8: Community APIs
//   8.0  GET /api/v1/communities            — List communities
//   8.1  POST /api/v1/communities           — Create community
//   8.2  POST /api/v1/communities/{id}/join  — Join community
//   8.3  POST /api/v1/communities/{id}/posts — Post in community

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use uuid::Uuid;

use crate::{
    app_state::AppState,
    dtos::{
        community::{
            CommunityPostResponseDto, CommunityResponseDto, CreateCommunityDto,
            CreateCommunityPostDto, UpdateCommunityDto, UpdateCommunityPostDto,
        },
        response::ApiResponse,
    },
    utils::{api_error::ApiError, auth_extractor::AuthenticationUser},
};

// ---------------------------------------------------------------------------
// 8.1  POST /api/v1/communities — Create a new community
// ---------------------------------------------------------------------------

pub async fn create_community(
    State(app_state): State<AppState>,
    auth: AuthenticationUser,
    Json(payload): Json<CreateCommunityDto>,
) -> Result<impl IntoResponse, ApiError> {
    let user_id = Uuid::parse_str(&auth.user_id)
        .map_err(|_| ApiError::Unauthorized("Invalid user".to_string()))?;

    if payload.name.trim().is_empty() {
        return Err(ApiError::BadRequest(
            "Community name cannot be empty".to_string(),
        ));
    }

    let community_id = Uuid::new_v4();
    let now = chrono::Utc::now();

    // Create the community
    sqlx::query(
        r#"
        INSERT INTO communities (id, name, description, created_by, created_at)
        VALUES ($1, $2, $3, $4, $5)
        "#,
    )
    .bind(community_id)
    .bind(&payload.name)
    .bind(&payload.description)
    .bind(user_id)
    .bind(now)
    .execute(&app_state.db)
    .await
    .map_err(|e| ApiError::InternalServerError(format!("Failed to create community: {}", e)))?;

    // Automatically add the creator as a member
    sqlx::query(
        r#"
        INSERT INTO community_members (id, community_id, user_id, joined_at)
        VALUES ($1, $2, $3, $4)
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(community_id)
    .bind(user_id)
    .bind(now)
    .execute(&app_state.db)
    .await
    .map_err(|e| {
        ApiError::InternalServerError(format!("Failed to add creator as member: {}", e))
    })?;

    let response = ApiResponse {
        success: true,
        message: "Community created successfully".to_string(),
        data: CommunityResponseDto {
            id: community_id,
            name: payload.name,
            description: payload.description,
            created_by: user_id,
            created_at: now,
            is_joined: true,
        },
    };

    Ok((StatusCode::CREATED, Json(response)))
}

// ---------------------------------------------------------------------------
// 8.2  POST /api/v1/communities/{id}/join — Join a community
// ---------------------------------------------------------------------------

pub async fn join_community(
    State(app_state): State<AppState>,
    auth: AuthenticationUser,
    Path(community_id): Path<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    let user_id = Uuid::parse_str(&auth.user_id)
        .map_err(|_| ApiError::Unauthorized("Invalid user".to_string()))?;

    // Verify the community exists
    let exists: Option<Uuid> = sqlx::query_scalar("SELECT id FROM communities WHERE id = $1")
        .bind(community_id)
        .fetch_optional(&app_state.db)
        .await
        .map_err(|e| ApiError::InternalServerError(format!("Database error: {}", e)))?;

    if exists.is_none() {
        return Err(ApiError::NotFound("Community not found".to_string()));
    }

    // Idempotent join — ON CONFLICT DO NOTHING handles already-a-member
    sqlx::query(
        r#"
        INSERT INTO community_members (id, community_id, user_id, joined_at)
        VALUES ($1, $2, $3, NOW())
        ON CONFLICT (community_id, user_id) DO NOTHING
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(community_id)
    .bind(user_id)
    .execute(&app_state.db)
    .await
    .map_err(|e| ApiError::InternalServerError(format!("Failed to join community: {}", e)))?;

    // Return empty data object as specified
    let response = ApiResponse {
        success: true,
        message: "Successfully joined the community".to_string(),
        data: serde_json::json!({}),
    };

    Ok((StatusCode::OK, Json(response)))
}

// ---------------------------------------------------------------------------
// 8.3  POST /api/v1/communities/{id}/posts — Post in a community
// ---------------------------------------------------------------------------

pub async fn create_community_post(
    State(app_state): State<AppState>,
    auth: AuthenticationUser,
    Path(community_id): Path<Uuid>,
    Json(payload): Json<CreateCommunityPostDto>,
) -> Result<impl IntoResponse, ApiError> {
    let user_id = Uuid::parse_str(&auth.user_id)
        .map_err(|_| ApiError::Unauthorized("Invalid user".to_string()))?;

    // Verify the community exists
    let exists: Option<Uuid> = sqlx::query_scalar("SELECT id FROM communities WHERE id = $1")
        .bind(community_id)
        .fetch_optional(&app_state.db)
        .await
        .map_err(|e| ApiError::InternalServerError(format!("Database error: {}", e)))?;

    if exists.is_none() {
        return Err(ApiError::NotFound("Community not found".to_string()));
    }

    // Verify the user is a member
    let is_member: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM community_members WHERE community_id = $1 AND user_id = $2",
    )
    .bind(community_id)
    .bind(user_id)
    .fetch_optional(&app_state.db)
    .await
    .map_err(|e| ApiError::InternalServerError(format!("Database error: {}", e)))?;

    if is_member.is_none() {
        return Err(ApiError::Forbidden(
            "You must be a member of this community to post".to_string(),
        ));
    }

    if payload.content.trim().is_empty() {
        return Err(ApiError::BadRequest(
            "Post content cannot be empty".to_string(),
        ));
    }

    let post_id = Uuid::new_v4();
    let now = chrono::Utc::now();

    sqlx::query(
        r#"
        INSERT INTO community_posts (id, community_id, author_id, content, created_at)
        VALUES ($1, $2, $3, $4, $5)
        "#,
    )
    .bind(post_id)
    .bind(community_id)
    .bind(user_id)
    .bind(&payload.content)
    .bind(now)
    .execute(&app_state.db)
    .await
    .map_err(|e| ApiError::InternalServerError(format!("Failed to create post: {}", e)))?;

    let response = ApiResponse {
        success: true,
        message: "Post created successfully".to_string(),
        data: CommunityPostResponseDto {
            post_id,
            community_id,
            author_id: user_id,
            content: payload.content,
            created_at: now,
        },
    };

    Ok((StatusCode::CREATED, Json(response)))
}

// ---------------------------------------------------------------------------
// 8.0  GET /api/v1/communities — List all communities
// ---------------------------------------------------------------------------

pub async fn get_communities(
    State(app_state): State<AppState>,
    auth: AuthenticationUser,
) -> Result<impl IntoResponse, ApiError> {
    // We just verify the user is properly authenticated
    let user_id = Uuid::parse_str(&auth.user_id)
        .map_err(|_| ApiError::Unauthorized("Invalid user".to_string()))?;

    let communities = sqlx::query_as!(
        CommunityResponseDto,
        r#"
        SELECT
            c.id, c.name, c.description, c.created_by, c.created_at,
            EXISTS(
                SELECT 1 FROM community_members cm 
                WHERE cm.community_id = c.id AND cm.user_id = $1
            ) as "is_joined!"
        FROM communities c
        ORDER BY c.created_at DESC
        "#,
        user_id
    )
    .fetch_all(&app_state.db)
    .await
    .map_err(|e| ApiError::InternalServerError(format!("Database error: {}", e)))?;

    let response = ApiResponse {
        success: true,
        message: "Communities fetched successfully".to_string(),
        data: communities,
    };

    Ok((StatusCode::OK, Json(response)))
}

// ---------------------------------------------------------------------------
// PUT /api/v1/communities/{id} — Edit a community (partial update)
// ---------------------------------------------------------------------------

pub async fn edit_community(
    State(app_state): State<AppState>,
    auth: AuthenticationUser,
    Path(community_id): Path<Uuid>,
    Json(payload): Json<UpdateCommunityDto>,
) -> Result<impl IntoResponse, ApiError> {
    let user_id = Uuid::parse_str(&auth.user_id)
        .map_err(|_| ApiError::Unauthorized("Invalid user".to_string()))?;

    // 1. Ownership check
    let owner: Option<Uuid> =
        sqlx::query_scalar("SELECT created_by FROM communities WHERE id = $1")
            .bind(community_id)
            .fetch_optional(&app_state.db)
            .await
            .map_err(|e| ApiError::InternalServerError(format!("Database error: {}", e)))?;

    match owner {
        None => return Err(ApiError::NotFound("Community not found".to_string())),
        Some(oid) if oid != user_id => return Err(ApiError::access_denied()),
        _ => {}
    }

    // 2. Validate name if provided
    if let Some(ref name) = payload.name {
        if name.trim().is_empty() {
            return Err(ApiError::BadRequest(
                "Community name cannot be empty".to_string(),
            ));
        }
    }

    // 3. Partial update
    let row = sqlx::query_as!(
        CommunityResponseDto,
        r#"
        UPDATE communities SET
            name        = COALESCE($2, name),
            description = COALESCE($3, description),
            updated_at  = NOW()
        WHERE id = $1
        RETURNING id, name, description, created_by, created_at,
            true as "is_joined!"
        "#,
        community_id,
        payload.name,
        payload.description,
    )
    .fetch_one(&app_state.db)
    .await
    .map_err(|e| ApiError::InternalServerError(format!("Failed to update community: {}", e)))?;

    let response = ApiResponse {
        success: true,
        message: "Community updated successfully".to_string(),
        data: row,
    };

    Ok((StatusCode::OK, Json(response)))
}

// ---------------------------------------------------------------------------
// PUT /api/v1/communities/{community_id}/posts/{post_id} — Edit a post
// ---------------------------------------------------------------------------

pub async fn edit_community_post(
    State(app_state): State<AppState>,
    auth: AuthenticationUser,
    Path((community_id, post_id)): Path<(Uuid, Uuid)>,
    Json(payload): Json<UpdateCommunityPostDto>,
) -> Result<impl IntoResponse, ApiError> {
    let user_id = Uuid::parse_str(&auth.user_id)
        .map_err(|_| ApiError::Unauthorized("Invalid user".to_string()))?;

    // Content is required and cannot be empty
    if payload.content.trim().is_empty() {
        return Err(ApiError::BadRequest(
            "Post content cannot be empty".to_string(),
        ));
    }

    // 1. Ownership check — must be the post author AND belong to the community
    let author: Option<Uuid> = sqlx::query_scalar(
        "SELECT author_id FROM community_posts WHERE id = $1 AND community_id = $2",
    )
    .bind(post_id)
    .bind(community_id)
    .fetch_optional(&app_state.db)
    .await
    .map_err(|e| ApiError::InternalServerError(format!("Database error: {}", e)))?;

    match author {
        None => return Err(ApiError::NotFound("Post not found".to_string())),
        Some(aid) if aid != user_id => return Err(ApiError::access_denied()),
        _ => {}
    }

    // 2. Update
    sqlx::query(
        r#"
        UPDATE community_posts SET
            content    = $3,
            updated_at = NOW()
        WHERE id = $1 AND community_id = $2
        "#,
    )
    .bind(post_id)
    .bind(community_id)
    .bind(&payload.content)
    .execute(&app_state.db)
    .await
    .map_err(|e| ApiError::InternalServerError(format!("Failed to update post: {}", e)))?;

    let now = chrono::Utc::now();
    let response = ApiResponse {
        success: true,
        message: "Post updated successfully".to_string(),
        data: CommunityPostResponseDto {
            post_id,
            community_id,
            author_id: user_id,
            content: payload.content,
            created_at: now,
        },
    };

    Ok((StatusCode::OK, Json(response)))
}
