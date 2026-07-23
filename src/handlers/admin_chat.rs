use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde_json::json;
use sqlx::Row;
use uuid::Uuid;


use crate::{
    app_state::AppState,
    dtos::admin_chat::{
        AdminChatMessageResponse, AdminChatMessagesData, AdminChatMessagesQuery,
        AdminChatMessagesResponse, AdminChatThreadResponse, AdminChatThreadsListResponse,
        AdminChatThreadsQuery, SendAdminMessageRequest,
    },
};

use jsonwebtoken::{decode, DecodingKey, Validation};
use crate::handlers::admin_auth::AdminClaims;
use crate::utils::auth::Claims;

fn extract_admin_id(
    headers: &axum::http::HeaderMap,
    admin_jwt_secret: &str,
) -> Result<String, String> {
    let cookie_header = headers.get(axum::http::header::COOKIE).and_then(|v| v.to_str().ok()).unwrap_or("");
    let mut token = None;
    for cookie in cookie_header.split(';') {
        let cookie = cookie.trim();
        if cookie.starts_with("admin_session=") {
            token = Some(&cookie["admin_session=".len()..]);
            break;
        }
    }
    if let Some(t) = token {
        let token_data = decode::<AdminClaims>(
            t,
            &DecodingKey::from_secret(admin_jwt_secret.as_bytes()),
            &Validation::default(),
        ).map_err(|e| e.to_string())?;
        Ok(token_data.claims.sub)
    } else {
        Err("No admin session cookie".into())
    }
}

fn require_auth_optional(
    headers: &axum::http::HeaderMap,
    jwt_secret: &str,
) -> Result<Option<Uuid>, String> {
    let bearer = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer ").map(|t| t.to_string()));

    if let Some(token) = bearer {
        let data = decode::<Claims>(
            &token,
            &DecodingKey::from_secret(jwt_secret.as_bytes()),
            &Validation::default(),
        ).map_err(|e| e.to_string())?;
        let uid = Uuid::parse_str(&data.claims.sub).map_err(|e| e.to_string())?;
        Ok(Some(uid))
    } else {
        Ok(None)
    }
}

// ---------------------------------------------------------------------------
// POST /api/chat/admin/messages
// ---------------------------------------------------------------------------
pub async fn send_admin_message(
    State(app_state): State<AppState>,
    headers: axum::http::HeaderMap,
    Json(payload): Json<SendAdminMessageRequest>,
) -> impl axum::response::IntoResponse {
    let is_admin;
    let sender_id_str;
    let mut user_uuid: Option<Uuid> = None;

    // We allow either Admin or User
    if let Ok(admin_token_id) = extract_admin_id(&headers, &app_state.admin_jwt_secret) {
        is_admin = true;
        sender_id_str = admin_token_id;
    } else if let Ok(uid) = require_auth_optional(&headers, &app_state.jwt_secret) {
        if let Some(uid) = uid {
            is_admin = false;
            sender_id_str = uid.to_string();
            user_uuid = Some(uid);
        } else {
            return (StatusCode::UNAUTHORIZED, Json(json!({"success":false,"message":"Unauthorized"})));
        }
    } else {
        return (StatusCode::UNAUTHORIZED, Json(json!({"success":false,"message":"Unauthorized"})));
    }

    let mut thread_id = payload.thread_id;
    let mut tx = match app_state.db.begin().await {
        Ok(t) => t,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success":false,"message":format!("DB error: {}", e)}))),
    };

    if thread_id.is_none() {
        if is_admin {
            return (StatusCode::BAD_REQUEST, Json(json!({"success":false,"message":"Admin must specify thread_id"})));
        } else {
            // Auto create thread for user if absent
            let existing: Option<Uuid> = sqlx::query_scalar("SELECT id FROM admin_chat_threads WHERE user_id = $1 LIMIT 1")
                .bind(user_uuid.unwrap())
                .fetch_optional(&mut *tx)
                .await
                .unwrap_or(None);

            if let Some(t_id) = existing {
                thread_id = Some(t_id);
            } else {
                let new_id = Uuid::new_v4();
                match sqlx::query("INSERT INTO admin_chat_threads (id, user_id) VALUES ($1, $2)")
                    .bind(new_id)
                    .bind(user_uuid.unwrap())
                    .execute(&mut *tx)
                    .await {
                        Ok(_) => thread_id = Some(new_id),
                        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success":false,"message":format!("DB error: {}", e)}))),
                    }
            }
        }
    }

    let t_id = thread_id.unwrap();
    
    // Ensure thread exists and belongs to user if user
    let thread_row = match sqlx::query("SELECT user_id, admin_id FROM admin_chat_threads WHERE id = $1")
        .bind(t_id)
        .fetch_optional(&mut *tx)
        .await
    {
        Ok(Some(r)) => r,
        Ok(None) => return (StatusCode::NOT_FOUND, Json(json!({"success":false,"message":"Thread not found"}))),
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success":false,"message":"DB error"}))),
    };

    let thread_user_id: Uuid = thread_row.get("user_id");
    
    if !is_admin && user_uuid.unwrap() != thread_user_id {
        return (StatusCode::FORBIDDEN, Json(json!({"success":false,"message":"Forbidden"})));
    }

    let msg_id = Uuid::new_v4();
    let sender_role = if is_admin { "admin" } else { "user" };

    match sqlx::query(
        "INSERT INTO admin_chat_messages (id, thread_id, sender_id, sender_role, message, attachment_url) VALUES ($1, $2, $3, $4, $5, $6)"
    )
    .bind(msg_id)
    .bind(t_id)
    .bind(&sender_id_str)
    .bind(sender_role)
    .bind(&payload.message)
    .bind(&payload.attachment_url)
    .execute(&mut *tx)
    .await {
        Ok(_) => {},
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success":false,"message":format!("DB error: {}", e)}))),
    };

    // Link admin to thread if not set and sent by admin
    if is_admin {
        let _ = sqlx::query("UPDATE admin_chat_threads SET admin_id = $1, updated_at = NOW() WHERE id = $2")
            .bind(&sender_id_str)
            .bind(t_id)
            .execute(&mut *tx)
            .await;
    } else {
        let _ = sqlx::query("UPDATE admin_chat_threads SET updated_at = NOW() WHERE id = $1")
            .bind(t_id)
            .execute(&mut *tx)
            .await;
    }
    
    // Trigger notification (basic mock logic)
    let recipient_user_id = if is_admin { Some(thread_user_id) } else { None };
    if let Some(r_id) = recipient_user_id {
        let _ = sqlx::query("INSERT INTO notifications (id, user_id, type, title, message, is_read, created_at) VALUES ($1, $2, $3, $4, $5, false, NOW())")
            .bind(Uuid::new_v4())
            .bind(r_id)
            .bind("admin_chat")
            .bind("Admin Support")
            .bind("You have a new message from Support.")
            .execute(&mut *tx)
            .await;
    }

    if let Err(e) = tx.commit().await {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success":false,"message":format!("DB error: {}", e)})));
    }

    (StatusCode::OK, Json(json!({"success":true,"message":"Message sent successfully"})))
}

// ---------------------------------------------------------------------------
// GET /api/chat/admin/threads/{thread_id}/messages
// ---------------------------------------------------------------------------
pub async fn get_admin_messages(
    State(app_state): State<AppState>,
    headers: axum::http::HeaderMap,
    Path(thread_id): Path<Uuid>,
    Query(q): Query<AdminChatMessagesQuery>,
) -> impl axum::response::IntoResponse {
    let mut is_admin = false;
    let mut user_uuid: Option<Uuid> = None;

    if let Ok(_) = extract_admin_id(&headers, &app_state.admin_jwt_secret) {
        is_admin = true;
    } else if let Ok(uid) = require_auth_optional(&headers, &app_state.jwt_secret) {
        if let Some(uid) = uid {
            user_uuid = Some(uid);
        } else {
            return (StatusCode::UNAUTHORIZED, Json(json!({"success":false,"message":"Unauthorized"})));
        }
    } else {
        return (StatusCode::UNAUTHORIZED, Json(json!({"success":false,"message":"Unauthorized"})));
    }

    let thread_row = match sqlx::query("SELECT user_id FROM admin_chat_threads WHERE id = $1")
        .bind(thread_id)
        .fetch_optional(&app_state.db)
        .await
    {
        Ok(Some(r)) => r,
        Ok(None) => return (StatusCode::NOT_FOUND, Json(json!({"success":false,"message":"Thread not found"}))),
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success":false,"message":"DB error"}))),
    };

    if !is_admin && user_uuid.unwrap() != thread_row.get::<Uuid, _>("user_id") {
        return (StatusCode::FORBIDDEN, Json(json!({"success":false,"message":"Forbidden"})));
    }

    let limit = q.limit.unwrap_or(50).clamp(1, 100);
    let offset = q.offset.unwrap_or(0).max(0);

    let mut query = sqlx::QueryBuilder::new("SELECT * FROM admin_chat_messages WHERE thread_id = ");
    query.push_bind(thread_id);

    if let Some(since) = q.since {
        query.push(" AND created_at > ");
        query.push_bind(since);
    }

    query.push(" ORDER BY created_at DESC LIMIT ");
    query.push_bind(limit);
    query.push(" OFFSET ");
    query.push_bind(offset);

    let rows = match query.build().fetch_all(&app_state.db).await {
        Ok(r) => r,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success":false,"message":"DB error"}))),
    };

    let messages = rows.into_iter().map(|row| AdminChatMessageResponse {
        id: row.get("id"),
        thread_id: row.get("thread_id"),
        sender_id: row.get("sender_id"),
        sender_role: row.get("sender_role"),
        message: row.get("message"),
        attachment_url: row.try_get("attachment_url").unwrap_or(None),
        created_at: row.get("created_at"),
    }).collect::<Vec<_>>();

    let resp = AdminChatMessagesResponse {
        success: true,
        data: AdminChatMessagesData {
            total: messages.len() as i64,
            messages,
        }
    };

    (StatusCode::OK, Json(json!(resp)))
}

// ---------------------------------------------------------------------------
// GET /api/chat/admin/threads
// ---------------------------------------------------------------------------
pub async fn get_admin_threads(
    State(app_state): State<AppState>,
    headers: axum::http::HeaderMap,
    Query(q): Query<AdminChatThreadsQuery>,
) -> impl axum::response::IntoResponse {
    let mut is_admin = false;
    let mut user_uuid: Option<Uuid> = None;

    if let Ok(_) = extract_admin_id(&headers, &app_state.admin_jwt_secret) {
        is_admin = true;
    } else if let Ok(uid) = require_auth_optional(&headers, &app_state.jwt_secret) {
        if let Some(uid) = uid {
            user_uuid = Some(uid);
        } else {
            return (StatusCode::UNAUTHORIZED, Json(json!({"success":false,"message":"Unauthorized"})));
        }
    } else {
        return (StatusCode::UNAUTHORIZED, Json(json!({"success":false,"message":"Unauthorized"})));
    }

    let mut query = sqlx::QueryBuilder::new(
        "SELECT t.*, (SELECT message FROM admin_chat_messages m WHERE m.thread_id = t.id ORDER BY m.created_at DESC LIMIT 1) as last_message FROM admin_chat_threads t WHERE 1=1"
    );

    if !is_admin {
        query.push(" AND t.user_id = ");
        query.push_bind(user_uuid.unwrap());
    } else {
        if let Some(st) = q.status {
            query.push(" AND t.status = ");
            query.push_bind(st);
        }
        if let Some(u_id) = q.user_id {
            query.push(" AND t.user_id = ");
            query.push_bind(u_id);
        }
    }

    query.push(" ORDER BY t.updated_at DESC");

    let rows = match query.build().fetch_all(&app_state.db).await {
        Ok(r) => r,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success":false,"message":"DB error"}))),
    };

    let threads = rows.into_iter().map(|row| AdminChatThreadResponse {
        id: row.get("id"),
        user_id: row.get("user_id"),
        admin_id: row.try_get("admin_id").unwrap_or(None),
        status: row.get("status"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
        last_message: row.try_get("last_message").unwrap_or(None),
    }).collect::<Vec<_>>();

    let resp = AdminChatThreadsListResponse {
        success: true,
        data: threads,
    };

    (StatusCode::OK, Json(json!(resp)))
}
