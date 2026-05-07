use crate::app_state::AppState;
use crate::models::chat::{
    AddMemberRequest, CreateChannelRequest, CreateUserRequest, SendMessageRequest,
};
use crate::utils::auth_extractor::AuthenticationUser;
use axum::{
    extract::{Json, Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use axum_extra::extract::Multipart;
use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::json;
use std::collections::HashSet;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

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
async fn hydrate_chime_arn(
    app_state: &AppState,
    user_id_str: &str,
) -> Result<(uuid::Uuid, String), String> {
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

        match app_state
            .chat_service
            .create_app_instance_user(&app_instance_arn, &user.id.to_string(), &user_name)
            .await
        {
            Ok(chat_user) => {
                let _ = app_state
                    .user_service
                    .update_chime_arn(&user.id.to_string(), &chat_user.app_instance_user_arn)
                    .await;
                chat_user.app_instance_user_arn
            }
            Err(e) => {
                eprintln!(
                    "[Chat] Failed to create chime user: {}. Using local ARN.",
                    e
                );
                format!("local_user_arn/{}", user.id)
            }
        }
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
    let (_creator_uuid, creator_arn) = match hydrate_chime_arn(&app_state, &auth_user.user_id).await
    {
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
                    return (
                        StatusCode::BAD_REQUEST,
                        Json(json!({"error": format!("Failed for participant {}: {}", pid, e)})),
                    )
                        .into_response();
                }
            }
        }
    }

    // 2.5 Deduplicate 1-on-1 chats
    if final_participants.len() == 2 {
        let u1 = final_participants[0].0;
        let u2 = final_participants[1].0;

        let existing_chat_id: Option<uuid::Uuid> = sqlx::query_scalar(
            r#"
            SELECT cp1.chat_id
            FROM chat_participants cp1
            JOIN chat_participants cp2 ON cp1.chat_id = cp2.chat_id
            JOIN chats c ON c.id = cp1.chat_id
            WHERE cp1.user_id = $1 AND cp2.user_id = $2
              AND c.is_deleted = FALSE
              AND (
                  SELECT COUNT(*) FROM chat_participants cp3 WHERE cp3.chat_id = cp1.chat_id
              ) = 2
            ORDER BY c.created_at DESC
            LIMIT 1
            "#,
        )
        .bind(u1)
        .bind(u2)
        .fetch_optional(&app_state.db)
        .await
        .unwrap_or(None);

        if let Some(chat_id) = existing_chat_id {
            return (
                StatusCode::OK,
                Json(crate::models::chat::ChatChannel {
                    channel_arn: format!("local_channel_arn/{}", chat_id),
                    name: payload.name.clone(),
                    mode: payload.mode.clone(),
                    privacy: payload.privacy.clone(),
                }),
            )
                .into_response();
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
            eprintln!(
                "[Chat] Chime channel creation failed: {}. Falling back to local PostgreSQL chat.",
                e
            );
            let local_uuid = uuid::Uuid::new_v4();
            crate::models::chat::ChatChannel {
                channel_arn: format!("local_channel_arn/{}", local_uuid),
                name: payload.name.clone(),
                mode: payload.mode.clone(),
                privacy: payload.privacy.clone(),
            }
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
                if arn != creator_arn
                    && !arn.starts_with("local_user_arn")
                    && !channel.channel_arn.starts_with("local_channel_arn")
                {
                    let _ = app_state
                        .chat_service
                        .add_channel_flow(&channel.channel_arn, &arn, &creator_arn)
                        .await;
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
                eprintln!(
                    "[Chat] Failed to create chime user in send_message: {}. Using local ARN.",
                    e
                );
                format!("local_user_arn/{}", user.id)
            }
        }
    };

    // 2.5 Block check
    if let Some(chat_id_str) = payload.channel_arn.split('/').last() {
        if let Ok(chat_uuid) = uuid::Uuid::parse_str(chat_id_str) {
            if let Ok(sender_uuid) = uuid::Uuid::parse_str(&auth_user.user_id) {
                let is_blocked: Option<bool> = sqlx::query_scalar(
                    r#"
                    SELECT EXISTS (
                        SELECT 1
                        FROM chat_participants cp
                        JOIN blocked_users b ON (b.blocker_id = $1 AND b.blocked_id = cp.user_id)
                                             OR (b.blocker_id = cp.user_id AND b.blocked_id = $1)
                        WHERE cp.chat_id = $2 AND cp.user_id != $1
                    )
                    "#,
                )
                .bind(sender_uuid)
                .bind(chat_uuid)
                .fetch_optional(&app_state.db)
                .await
                .unwrap_or(None);

                if is_blocked == Some(true) {
                    return (
                        StatusCode::FORBIDDEN,
                        Json(json!({
                            "success": false,
                            "error": "Cannot send message. A block exists between you and a participant in this chat."
                        })),
                    ).into_response();
                }
            }
        }
    }

    // 3. Send Message — try Chime first, fall back to Postgres-only if Chime fails
    let chime_result = app_state
        .chat_service
        .send_message(&payload.channel_arn, &payload.content, &sender_arn)
        .await;

    if let Err(ref e) = chime_result {
        eprintln!(
            "[Chat] Chime send_message failed (will fallback to DB): {}",
            e
        );
    }

    // 4. Always persist to PostgreSQL regardless of Chime outcome
    let mut db_message_id: Option<String> = None;

    if let Some(chat_id_str) = payload.channel_arn.split('/').last() {
        if let Ok(chat_uuid) = uuid::Uuid::parse_str(chat_id_str) {
            if let Ok(sender_uuid) = uuid::Uuid::parse_str(&auth_user.user_id) {
                let local_msg_id = uuid::Uuid::new_v4();

                // Auto-hydrate the chats table if it doesn't exist
                if let Err(e) = sqlx::query(
                    "INSERT INTO chats (id, name, created_at) VALUES ($1, 'AWS Chime Chat', NOW()) ON CONFLICT DO NOTHING"
                )
                .bind(chat_uuid)
                .execute(&app_state.db)
                .await {
                    eprintln!("Failed to hydrate chat in db: {}", e);
                }

                match sqlx::query(
                    "INSERT INTO messages (id, chat_id, sender_id, content, message_type, created_at) VALUES ($1, $2, $3, $4, 'text', NOW())"
                )
                .bind(local_msg_id)
                .bind(chat_uuid)
                .bind(sender_uuid)
                .bind(&payload.content)
                .execute(&app_state.db)
                .await {
                    Ok(_) => { 
                        db_message_id = Some(local_msg_id.to_string()); 
                        push_message_and_notification(
                            &app_state,
                            chat_uuid,
                            sender_uuid,
                            local_msg_id,
                            payload.content.clone(),
                            "text".to_string(),
                        )
                        .await;
                    }
                    Err(e) => { eprintln!("Failed to insert message to db: {}", e); }
                }

                // Also make sure sender is part of this chat in participants just in case
                if let Err(e) = sqlx::query(
                    "INSERT INTO chat_participants (chat_id, user_id, joined_at) VALUES ($1, $2, NOW()) ON CONFLICT DO NOTHING"
                )
                .bind(chat_uuid)
                .bind(sender_uuid)
                .execute(&app_state.db)
                .await {
                    eprintln!("Failed to insert sender to chat_participants: {}", e);
                }
            }
        }
    }

    // Return success if either Chime or DB succeeded
    match (chime_result, db_message_id) {
        (Ok(msg_id), _) => (StatusCode::OK, Json(json!({"message_id": msg_id}))).into_response(),
        (Err(_), Some(local_id)) => {
            // Chime failed but DB save succeeded — still report success to client
            (
                StatusCode::OK,
                Json(json!({"message_id": local_id, "source": "local"})),
            )
                .into_response()
        }
        (Err(e), None) => {
            // Both Chime and DB failed — this is a real error
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            )
                .into_response()
        }
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

// ─────────────────────────────────────────────────────────────────────────────
// Message row returned by GET /api/v1/chats/{chat_id}/messages
// ─────────────────────────────────────────────────────────────────────────────
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct MessageRow {
    pub id: Uuid,
    pub chat_id: Uuid,
    pub sender_id: Uuid,
    pub content: String,
    pub message_type: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
}

// ─────────────────────────────────────────────────────────────────────────────
/// POST /api/v1/chats/upload
///
/// Upload an image or document and immediately create a message entry.
///
/// Multipart fields:
///   - `file`    — the file bytes (required)
///   - `chat_id` — UUID of the target chat (required)
///
/// Response:
/// ```json
/// { "success": true, "file_url": "/uploads/chat/...", "file_type": "image", "message_id": "..." }
/// ```
// ─────────────────────────────────────────────────────────────────────────────
pub async fn upload_chat_media(
    State(app_state): State<AppState>,
    auth: AuthenticationUser,
    mut multipart: Multipart,
) -> impl IntoResponse {
    // [Fix]: First, securely parse the standard sender's UUID from the JWT token.
    let sender_uuid = match Uuid::parse_str(&auth.user_id) {
        Ok(id) => id,
        Err(_) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"success": false, "message": "Invalid token"})),
            )
                .into_response();
        }
    };

    let mut file_bytes: Option<Vec<u8>> = None;
    let mut file_name_orig: Option<String> = None;
    let mut content_type_str: Option<String> = None;
    let mut chat_id_opt: Option<Uuid> = None;

    // ── Parse multipart fields ──
    while let Ok(Some(field)) = multipart.next_field().await {
        let field_name = field.name().unwrap_or("").to_string();

        match field_name.as_str() {
            "file" => {
                file_name_orig = Some(field.file_name().unwrap_or("upload").to_string());
                content_type_str = Some(
                    field
                        .content_type()
                        .unwrap_or("application/octet-stream")
                        .to_string(),
                );
                match field.bytes().await {
                    Ok(b) => file_bytes = Some(b.to_vec()),
                    Err(e) => {
                        return (
                            StatusCode::BAD_REQUEST,
                            Json(json!({"success": false, "message": format!("Failed to read file: {}", e)})),
                        )
                            .into_response();
                    }
                }
            }
            "chat_id" => {
                if let Ok(Some(text)) = field.text().await.map(Some) {
                    match Uuid::parse_str(text.trim()) {
                        Ok(uid) => chat_id_opt = Some(uid),
                        Err(_) => {
                            return (
                                StatusCode::BAD_REQUEST,
                                Json(json!({"success": false, "message": "Invalid chat_id UUID"})),
                            )
                                .into_response();
                        }
                    }
                }
            }
            _ => {} // ignore unknown fields
        }
    }

    // ── Validate required fields ──
    let data = match file_bytes {
        Some(b) => b,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"success": false, "message": "No file field in request"})),
            )
                .into_response();
        }
    };

    let chat_uuid = match chat_id_opt {
        Some(id) => id,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"success": false, "message": "Missing chat_id field"})),
            )
                .into_response();
        }
    };

    let ct = content_type_str
        .as_deref()
        .unwrap_or("application/octet-stream");
    let orig_name = file_name_orig.as_deref().unwrap_or("upload");

    // [Fix]: Categorize the file type. AWS Chime only supports text natively,
    // so we store media as direct URLs. We categorize into 'image' or 'document'
    // to allow the frontend to render the appropriate UI widget.
    let message_type = if ct.starts_with("image/") {
        "image"
    } else if ct == "application/pdf"
        || ct == "application/msword"
        || ct.starts_with("application/vnd.openxmlformats")
        || ct == "text/plain"
        || ct == "application/vnd.ms-excel"
        || ct.starts_with("application/vnd.ms-powerpoint")
    {
        "document"
    } else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "success": false,
                "message": format!("Unsupported file type: {}. Allowed: images and documents (pdf, docx, txt, etc.)", ct)
            })),
        )
            .into_response();
    };

    // ── File size check (5 MB) ──
    if data.len() > 5 * 1024 * 1024 {
        return (
            StatusCode::PAYLOAD_TOO_LARGE,
            Json(json!({"success": false, "message": "File too large (max 5MB)"})),
        )
            .into_response();
    }

    // ── Build file path ──
    let ext = std::path::Path::new(orig_name)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("bin");
    let new_filename = format!("{}_{}.{}", sender_uuid, Uuid::new_v4(), ext);
    let dir = "uploads/chat";
    let filepath = format!("{}/{}", dir, new_filename);

    if let Err(e) = tokio::fs::create_dir_all(dir).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"success": false, "message": format!("Storage error: {}", e)})),
        )
            .into_response();
    }

    let mut f = match File::create(&filepath).await {
        Ok(f) => f,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success": false, "message": format!("Failed to create file: {}", e)})),
            )
                .into_response();
        }
    };

    if let Err(e) = f.write_all(&data).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"success": false, "message": format!("Failed to write file: {}", e)})),
        )
            .into_response();
    }

    let file_url = format!("/uploads/chat/{}", new_filename);

    // ── Ensure chat row exists ──
    let _ = sqlx::query(
        "INSERT INTO chats (id, name, created_at) VALUES ($1, 'AWS Chime Chat', NOW()) ON CONFLICT DO NOTHING"
    )
    .bind(chat_uuid)
    .execute(&app_state.db)
    .await;

    // ── Ensure sender is a chat participant ──
    let _ = sqlx::query(
        "INSERT INTO chat_participants (chat_id, user_id, joined_at) VALUES ($1, $2, NOW()) ON CONFLICT DO NOTHING"
    )
    .bind(chat_uuid)
    .bind(sender_uuid)
    .execute(&app_state.db)
    .await;

    // [Fix]: Instead of sending the media over AWS Chime (which drops complex media),
    // we bypass Chime entirely and directly save the uploaded file message to our PostgreSQL database.
    // This perfectly preserves the file_url and message_type.
    let message_id = Uuid::new_v4();
    match sqlx::query(
        "INSERT INTO messages (id, chat_id, sender_id, content, message_type, created_at)
         VALUES ($1, $2, $3, $4, $5, NOW())",
    )
    .bind(message_id)
    .bind(chat_uuid)
    .bind(sender_uuid)
    .bind(&file_url)
    .bind(message_type)
    .execute(&app_state.db)
    .await
    {
        Ok(_) => {
            push_message_and_notification(
                &app_state,
                chat_uuid,
                sender_uuid,
                message_id,
                file_url.clone(),
                message_type.to_string(),
            )
            .await;

            (
                StatusCode::CREATED,
                Json(json!({
                    "success": true,
                    "message": "Media uploaded and message created",
                    "file_url": file_url,
                    "file_type": message_type,
                    "message_id": message_id
                })),
            ).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"success": false, "message": format!("Failed to save message: {}", e)})),
        )
            .into_response(),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Helper to push WebSocket updates or Database notifications after a message is sent
// ─────────────────────────────────────────────────────────────────────────────
async fn push_message_and_notification(
    app_state: &AppState,
    chat_uuid: Uuid,
    sender_uuid: Uuid,
    message_id: Uuid,
    content: String,
    message_type: String,
) {
    let receiver_id_opt: Option<Uuid> = sqlx::query_scalar(
        "SELECT user_id FROM chat_participants WHERE chat_id = $1 AND user_id != $2 LIMIT 1",
    )
    .bind(chat_uuid)
    .bind(sender_uuid)
    .fetch_optional(&app_state.db)
    .await
    .unwrap_or(None);

    if let Some(receiver_id) = receiver_id_opt {
        if let Some(socket) = app_state.active_sockets.get(&receiver_id) {
            // Receiver is connected, send via WebSocket
            let ws_msg = crate::models::chat::WsMessage::NewMessage {
                message_id,
                chat_id: chat_uuid,
                sender_id: sender_uuid,
                content: content.clone(),
                message_type,
                created_at: chrono::Utc::now(),
            };
            if let Ok(json_str) = serde_json::to_string(&ws_msg) {
                let _ = socket.send(json_str).await;
            }

            // Mark as delivered
            let _ = sqlx::query("UPDATE messages SET status = 'delivered' WHERE id = $1")
                .bind(message_id)
                .execute(&app_state.db)
                .await;

            // Push delivery receipt to sender
            if let Some(sender_socket) = app_state.active_sockets.get(&sender_uuid) {
                let receipt = crate::models::chat::WsMessage::MessageDelivered {
                    message_id,
                    delivered_at: chrono::Utc::now(),
                };
                if let Ok(receipt_str) = serde_json::to_string(&receipt) {
                    let _ = sender_socket.send(receipt_str).await;
                }
            }
        } else {
            // Receiver is NOT connected, insert notification
            let sender_name_opt: Option<String> = sqlx::query_scalar(
                "SELECT first_name || ' ' || last_name FROM users WHERE id = $1",
            )
            .bind(sender_uuid)
            .fetch_optional(&app_state.db)
            .await
            .unwrap_or(Some("Someone".to_string()));

            let sender_name = sender_name_opt.unwrap_or("Someone".to_string());
            let preview = if content.len() > 50 {
                format!("{}...", &content[..47])
            } else {
                content.clone()
            };

            let _ = sqlx::query(
                "INSERT INTO notifications (user_id, title, message, type, is_read, related_entity_id, related_entity_type, created_at) VALUES ($1, $2, $3, 'MESSAGE', false, $4, 'chat', NOW())"
            )
            .bind(receiver_id)
            .bind(&sender_name)
            .bind(&preview)
            .bind(chat_uuid)
            .execute(&app_state.db)
            .await;
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
/// PATCH /api/v1/chats/{chat_id}/seen
///
/// Mark all unread messages in a chat as seen, clear notifications, and send WS receipt.
// ─────────────────────────────────────────────────────────────────────────────
pub async fn mark_chat_seen(
    State(app_state): State<AppState>,
    auth: AuthenticationUser,
    Path(chat_id_str): Path<String>,
) -> impl IntoResponse {
    let chat_uuid = match Uuid::parse_str(&chat_id_str) {
        Ok(id) => id,
        Err(_) => return (StatusCode::BAD_REQUEST, Json(json!({"success": false, "message": "Invalid chat_id"}))).into_response(),
    };

    let user_uuid = match Uuid::parse_str(&auth.user_id) {
        Ok(id) => id,
        Err(_) => return (StatusCode::UNAUTHORIZED, Json(json!({"success": false, "message": "Invalid token"}))).into_response(),
    };

    // 1. Update message status
    // All messages in this chat where sender is NOT the current user and status is NOT seen
    let _ = sqlx::query(
        "UPDATE messages SET status = 'seen' WHERE chat_id = $1 AND sender_id != $2 AND status != 'seen'"
    )
    .bind(chat_uuid)
    .bind(user_uuid)
    .execute(&app_state.db)
    .await;

    // 2. Clear notifications for this chat
    let _ = sqlx::query(
        "UPDATE notifications SET is_read = true WHERE user_id = $1 AND related_entity_id = $2 AND type = 'MESSAGE'"
    )
    .bind(user_uuid)
    .bind(chat_uuid)
    .execute(&app_state.db)
    .await;

    // 3. Notify the OTHER participant via WebSocket that their messages were seen
    let sender_id_opt: Option<Uuid> = sqlx::query_scalar(
        "SELECT user_id FROM chat_participants WHERE chat_id = $1 AND user_id != $2 LIMIT 1",
    )
    .bind(chat_uuid)
    .bind(user_uuid)
    .fetch_optional(&app_state.db)
    .await
    .unwrap_or(None);

    if let Some(sender_id) = sender_id_opt {
        if let Some(socket) = app_state.active_sockets.get(&sender_id) {
            let receipt = crate::models::chat::WsMessage::MessageSeen {
                conversation_id: chat_uuid,
                seen_by: user_uuid,
                seen_at: chrono::Utc::now(),
            };
            if let Ok(receipt_str) = serde_json::to_string(&receipt) {
                let _ = socket.send(receipt_str).await;
            }
        }
    }

    (
        StatusCode::OK,
        Json(json!({"success": true, "message": "Chat marked as seen"})),
    ).into_response()
}

// ─────────────────────────────────────────────────────────────────────────────
/// GET /api/v1/chats/{chat_id}/messages
///
/// Fetch all messages for a chat (text + media), oldest first.
/// The caller must be a participant of the chat.
// ─────────────────────────────────────────────────────────────────────────────
pub async fn get_chat_messages(
    State(app_state): State<AppState>,
    auth: AuthenticationUser,
    Path(chat_id_str): Path<String>,
) -> impl IntoResponse {
    // [Fix]: Support local chat identifiers sent by Flutter when Chime is down.
    // These are handled gracefully as empty sessions.
    let chat_id = match Uuid::parse_str(&chat_id_str) {
        Ok(uid) => uid,
        Err(_) => {
            if chat_id_str.starts_with("local_chat_")
                || chat_id_str.starts_with("local_channel_arn")
            {
                return (
                    StatusCode::OK,
                    Json(json!({
                        "success": true,
                        "message": "Local chat session (ephemeral)",
                        "data": []
                    })),
                )
                    .into_response();
            }
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "success": false,
                    "message": format!("Invalid chat ID format: '{}'. Expected UUID or local string.", chat_id_str)
                })),
            )
                .into_response();
        }
    };

    let user_uuid = match Uuid::parse_str(&auth.user_id) {
        Ok(id) => id,
        Err(_) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"success": false, "message": "Invalid token"})),
            )
                .into_response();
        }
    };

    // ── Verify user is a participant ──
    let is_participant: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM chat_participants WHERE chat_id = $1 AND user_id = $2)",
    )
    .bind(chat_id)
    .bind(user_uuid)
    .fetch_one(&app_state.db)
    .await
    .unwrap_or(false);

    if !is_participant {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({
                "success": false,
                "message": "You are not a participant of this chat"
            })),
        )
            .into_response();
    }

    // [Fix]: Since media messages bypass AWS Chime, we query our own local PostgreSQL
    // `messages` table to construct the full chat history. This retrieves standard text
    // messages (synced earlier) as well as direct image/document uploads.
    // We default legacy messages to 'text'.
    let messages = sqlx::query_as::<_, MessageRow>(
        r#"
        SELECT id, chat_id, sender_id, content,
               COALESCE(message_type, 'text') AS message_type,
               status,
               created_at
        FROM messages
        WHERE chat_id = $1
        ORDER BY created_at ASC
        "#,
    )
    .bind(chat_id)
    .fetch_all(&app_state.db)
    .await;

    match messages {
        Ok(rows) => (
            StatusCode::OK,
            Json(json!({
                "success": true,
                "message": "Messages fetched successfully",
                "data": rows
            })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "success": false,
                "message": format!("Failed to fetch messages: {}", e)
            })),
        )
            .into_response(),
    }
}

/// GET /api/v1/chats/channel/messages?channel_arn=arn:aws:chime:...
///
/// Accepts the full Chime channel ARN as a query parameter and extracts
/// the UUID from the last path segment to fetch messages.
#[derive(serde::Deserialize)]
pub struct ChannelArnQuery {
    pub channel_arn: String,
}

pub async fn get_chat_messages_by_channel(
    State(app_state): State<AppState>,
    auth: AuthenticationUser,
    axum::extract::Query(query): axum::extract::Query<ChannelArnQuery>,
) -> impl IntoResponse {
    // Extract UUID from the last segment of the Chime ARN
    let chat_id = match query
        .channel_arn
        .split('/')
        .last()
        .and_then(|s| Uuid::parse_str(s).ok())
    {
        Some(id) => id,
        None => {
            // Maybe it's already a plain UUID
            match Uuid::parse_str(&query.channel_arn) {
                Ok(id) => id,
                Err(_) => {
                    return (
                        StatusCode::BAD_REQUEST,
                        Json(json!({"success": false, "message": "Could not extract a valid chat UUID from channel_arn"})),
                    )
                        .into_response();
                }
            }
        }
    };

    let user_uuid = match Uuid::parse_str(&auth.user_id) {
        Ok(id) => id,
        Err(_) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"success": false, "message": "Invalid token"})),
            )
                .into_response();
        }
    };

    // Verify user is a participant
    let is_participant: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM chat_participants WHERE chat_id = $1 AND user_id = $2)",
    )
    .bind(chat_id)
    .bind(user_uuid)
    .fetch_one(&app_state.db)
    .await
    .unwrap_or(false);

    if !is_participant {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({
                "success": false,
                "message": "You are not a participant of this chat"
            })),
        )
            .into_response();
    }

    let messages = sqlx::query_as::<_, MessageRow>(
        r#"
        SELECT id, chat_id, sender_id, content,
               COALESCE(message_type, 'text') AS message_type,
               created_at
        FROM messages
        WHERE chat_id = $1
        ORDER BY created_at ASC
        "#,
    )
    .bind(chat_id)
    .fetch_all(&app_state.db)
    .await;

    match messages {
        Ok(rows) => (
            StatusCode::OK,
            Json(json!({
                "success": true,
                "message": "Messages fetched successfully",
                "data": rows
            })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "success": false,
                "message": format!("Failed to fetch messages: {}", e)
            })),
        )
            .into_response(),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
/// GET /api/v1/chats/{arn_part}/{app_instance_id}/channel/{channel_id}/messages
///
/// Flutter sends the FULL AWS Chime ARN unencoded directly in the path (which contains slashes).
/// We map an Axum route strictly matching those segments to extract the final `channel_id`
/// and route it transparently to the main message fetcher.
// ─────────────────────────────────────────────────────────────────────────────
pub async fn get_chat_messages_arn_path(
    state: State<AppState>,
    auth: AuthenticationUser,
    Path((_arn_prefix, _app_id, channel_id)): Path<(String, String, String)>,
) -> impl IntoResponse {
    // Just forward the extracted channel_id string to our main local logic
    get_chat_messages(state, auth, Path(channel_id)).await
}
