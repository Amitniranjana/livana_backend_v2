use crate::app_state::AppState;
use crate::models::chat::{
    AddMemberRequest, CreateChannelRequest, CreateUserRequest, SendMessageRequest,
};
use axum::{
    extract::{Json, Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde_json::json;
use std::collections::HashSet;
use crate::utils::auth_extractor::AuthenticationUser;

/// Create User
pub async fn create_user(
    State(app_state): State<AppState>,
    Json(payload): Json<CreateUserRequest>,
) -> impl IntoResponse {
    // NOTE: In a real app, `app_instance_arn` should probably be in config/env
    let app_instance_arn = std::env::var("CHIME_APP_INSTANCE_ARN").unwrap_or_default();

    match app_state
        .chat_service
        .create_app_instance_user(&app_instance_arn, &payload.user_id, &payload.username)
        .await
    {
        Ok(user) => (StatusCode::CREATED, Json(user)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// Helper to lazily create or fetch a Chime ARN for a user
async fn hydrate_chime_arn(app_state: &AppState, user_id_str: &str) -> Result<(uuid::Uuid, String), String> {
    let user = app_state
        .user_service
        .user_repository
        .find_by_id(user_id_str)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("User not found: {}", user_id_str))?;

    let arn = if let Some(arn) = user.chime_user_arn {
        arn
    } else {
        let app_instance_arn = std::env::var("CHIME_APP_INSTANCE_ARN").unwrap_or_default();
        let user_name = format!("{} {}", user.first_name, user.last_name);
        
        let chat_user = app_state
            .chat_service
            .create_app_instance_user(&app_instance_arn, &user.id.to_string(), &user_name)
            .await
            .map_err(|e| format!("Failed to create chime user: {}", e))?;
            
        let _ = app_state
            .user_service
            .update_chime_arn(&user.id.to_string(), &chat_user.app_instance_user_arn)
            .await;
            
        chat_user.app_instance_user_arn
    };

    Ok((user.id, arn))
}

/// Create Channel
pub async fn create_channel(
    State(app_state): State<AppState>,
    auth_user: AuthenticationUser,
    Json(payload): Json<CreateChannelRequest>,
) -> impl IntoResponse {
    let app_instance_arn = std::env::var("CHIME_APP_INSTANCE_ARN").unwrap_or_default();
    
    // 1. Get creator ARN
    let (_creator_uuid, creator_arn) = match hydrate_chime_arn(&app_state, &auth_user.user_id).await {
        Ok(res) => res,
        Err(e) => {
            return (StatusCode::BAD_REQUEST, Json(json!({"error": e}))).into_response();
        }
    };

    // 2. Gather all expected participants (deduplicated)
    let mut participant_ids_to_hydrate = payload.participant_ids.clone();
    participant_ids_to_hydrate.push(auth_user.user_id.clone());
    
    let mut unique_ids = HashSet::new();
    let mut final_participants: Vec<(uuid::Uuid, String)> = Vec::new();
    
    for pid in participant_ids_to_hydrate {
        if unique_ids.insert(pid.clone()) {
            match hydrate_chime_arn(&app_state, &pid).await {
                Ok(res) => final_participants.push(res),
                Err(e) => {
                    return (StatusCode::BAD_REQUEST, Json(json!({"error": format!("Failed for participant {}: {}", pid, e)}))).into_response();
                }
            }
        }
    }

    // 3. Create the Channel
    let channel = match app_state
        .chat_service
        .create_channel(
            &app_instance_arn,
            &payload.name,
            &payload.mode,
            &payload.privacy,
            &creator_arn,
        )
        .await
    {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            ).into_response();
        }
    };

    // 4. Add Members to Chime Channel AND sync to PostgreSQL
    if let Some(chat_id_str) = channel.channel_arn.split('/').last() {
        if let Ok(chat_uuid) = uuid::Uuid::parse_str(chat_id_str) {
            // Insert into chats
            let _ = sqlx::query("INSERT INTO chats (id, name, created_at) VALUES ($1, $2, NOW()) ON CONFLICT DO NOTHING")
                .bind(chat_uuid)
                .bind(&payload.name)
                .execute(&app_state.db)
                .await;

            // Process Members
            for (uid, arn) in final_participants {
                // If the user isn't the creator, add them via add_channel_flow
                if arn != creator_arn {
                    let _ = app_state.chat_service.add_channel_flow(&channel.channel_arn, &arn, &creator_arn).await;
                }

                // Insert into Postgres
                let _ = sqlx::query("INSERT INTO chat_participants (chat_id, user_id, joined_at) VALUES ($1, $2, NOW()) ON CONFLICT DO NOTHING")
                    .bind(chat_uuid)
                    .bind(uid)
                    .execute(&app_state.db)
                    .await;
            }
        }
    }

    (StatusCode::CREATED, Json(channel)).into_response()
}

/// Add Member
pub async fn add_member(
    State(app_state): State<AppState>,
    Path(channel_arn): Path<String>,
    Json(payload): Json<AddMemberRequest>,
) -> impl IntoResponse {
    // Helper to extract whoever is making the request (admin)
    let admin_arn = std::env::var("CHIME_ADMIN_ARN").unwrap_or_default(); // Placeholder

    match app_state
        .chat_service
        .add_channel_flow(&channel_arn, &payload.member_arn, &admin_arn)
        .await
    {
        Ok(_) => (StatusCode::OK, Json(json!({"message": "Member added"}))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

// Import AuthenticationUser is now at the top

/// Send Message
pub async fn send_message(
    State(app_state): State<AppState>,
    auth_user: AuthenticationUser,
    Json(payload): Json<SendMessageRequest>,
) -> impl IntoResponse {
    // 1. Get user from DB to check if they have a Chime ARN
    let user = match app_state
        .user_service
        .user_repository
        .find_by_id(&auth_user.user_id)
        .await
    {
        Ok(Some(user)) => user,
        Ok(None) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "User not found"})),
            )
                .into_response();
        }
        Err(e) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e}))).into_response();
        }
    };

    // 2. Determine Sender ARN (Lazy Creation)
    let sender_arn = if let Some(arn) = user.chime_user_arn {
        arn
    } else {
        // Create Chime User
        let app_instance_arn = std::env::var("CHIME_APP_INSTANCE_ARN").unwrap_or_default();
        let user_name = format!("{} {}", user.first_name, user.last_name);

        match app_state
            .chat_service
            .create_app_instance_user(&app_instance_arn, &user.id.to_string(), &user_name)
            .await
        {
            Ok(chat_user) => {
                // Save to DB
                if let Err(e) = app_state
                    .user_service
                    .update_chime_arn(&user.id.to_string(), &chat_user.app_instance_user_arn)
                    .await
                {
                    eprintln!("Failed to save chime ARN: {}", e);
                    // Continue anyway since we have the ARN, but log error
                }
                chat_user.app_instance_user_arn
            }
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": format!("Failed to create chime user: {}", e)})),
                )
                    .into_response();
            }
        }
    };

    // 3. Send Message as USER
    match app_state
        .chat_service
        .send_message(&payload.channel_arn, &payload.content, &sender_arn)
        .await
    {
        Ok(msg_id) => {
            // ── SYNC TO POSTGRESQL ──
            if let Some(chat_id_str) = payload.channel_arn.split('/').last() {
                if let Ok(chat_uuid) = uuid::Uuid::parse_str(chat_id_str) {
                    if let Ok(sender_uuid) = uuid::Uuid::parse_str(&auth_user.user_id) {
                        let local_msg_id = uuid::Uuid::new_v4();

                        // Auto-hydrate the chats table if it doesn't exist
                        let _ = sqlx::query(
                            "INSERT INTO chats (id, name, created_at) VALUES ($1, 'AWS Chime Chat', NOW()) ON CONFLICT DO NOTHING"
                        )
                        .bind(chat_uuid)
                        .execute(&app_state.db)
                        .await;

                        let _ = sqlx::query(
                            "INSERT INTO messages (id, chat_id, sender_id, content, created_at) VALUES ($1, $2, $3, $4, NOW()) ON CONFLICT DO NOTHING"
                        )
                        .bind(local_msg_id)
                        .bind(chat_uuid)
                        .bind(sender_uuid)
                        .bind(&payload.content)
                        .execute(&app_state.db)
                        .await;
                        
                        // Also make sure sender is part of this chat in participants just in case
                        let _ = sqlx::query(
                            "INSERT INTO chat_participants (chat_id, user_id, joined_at) VALUES ($1, $2, NOW()) ON CONFLICT DO NOTHING"
                        )
                        .bind(chat_uuid)
                        .bind(sender_uuid)
                        .execute(&app_state.db)
                        .await;
                    }
                }
            }

            (StatusCode::OK, Json(json!({"message_id": msg_id}))).into_response()
        },
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// Get WebSocket Endpoint
pub async fn get_auth_creds(State(app_state): State<AppState>) -> impl IntoResponse {
    match app_state.chat_service.get_messaging_endpoint().await {
        Ok(endpoint) => (StatusCode::OK, Json(json!({"endpoint": endpoint}))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}
