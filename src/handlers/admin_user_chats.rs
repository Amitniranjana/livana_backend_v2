use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde_json::json;
use sqlx::Row;
use uuid::Uuid;


use crate::{
    app_state::AppState,
    dtos::admin_user_chats::{
        AdminUserChatsQuery, AdminUserChatResponse, AdminUserChatParticipant,
        AdminUserChatsListResponse, AdminUserChatsListData,
        AdminUserChatMessagesQuery, AdminUserChatMessageResponse,
        AdminUserChatMessagesListResponse, AdminUserChatMessagesListData,
    },
};

pub async fn get_all_user_chats_admin(
    State(app_state): State<AppState>,
    Query(q): Query<AdminUserChatsQuery>,
) -> impl axum::response::IntoResponse {
    let limit = q.limit.unwrap_or(50).clamp(1, 100);
    let page = q.page.unwrap_or(1).max(1);
    let offset = (page - 1) * limit;

    let search = q.search.clone().unwrap_or_default();
    let is_blocked = q.is_blocked;
    let is_archived = q.is_archived;

    // Building the query to fetch distinct chats based on filters
    let mut count_query = sqlx::QueryBuilder::new(
        "SELECT COUNT(DISTINCT c.id) 
         FROM chats c
         LEFT JOIN chat_participants cp ON c.id = cp.chat_id
         LEFT JOIN users u ON cp.user_id = u.id "
    );

    let mut query = sqlx::QueryBuilder::new(
        "SELECT c.id, c.name, c.created_at,
            (SELECT content FROM messages m WHERE m.chat_id = c.id ORDER BY m.created_at DESC LIMIT 1) as last_message,
            (SELECT created_at FROM messages m WHERE m.chat_id = c.id ORDER BY m.created_at DESC LIMIT 1) as last_message_at
         FROM chats c
         LEFT JOIN chat_participants cp ON c.id = cp.chat_id
         LEFT JOIN users u ON cp.user_id = u.id "
    );

    let mut conditions = vec!["1=1".to_string()];

    if !search.is_empty() {
        let search_pattern = format!("%{}%", search);
        conditions.push(format!("(u.name ILIKE '{}' OR u.email ILIKE '{}')", search_pattern, search_pattern));
    }

    if let Some(blocked) = is_blocked {
        if blocked {
            conditions.push("EXISTS (
                SELECT 1 FROM chat_participants cp1
                JOIN chat_participants cp2 ON cp1.chat_id = cp2.chat_id AND cp1.user_id != cp2.user_id
                JOIN blocked_users bu ON bu.blocker_id = cp1.user_id AND bu.blocked_id = cp2.user_id
                WHERE cp1.chat_id = c.id
            )".to_string());
        } else {
            conditions.push("NOT EXISTS (
                SELECT 1 FROM chat_participants cp1
                JOIN chat_participants cp2 ON cp1.chat_id = cp2.chat_id AND cp1.user_id != cp2.user_id
                JOIN blocked_users bu ON bu.blocker_id = cp1.user_id AND bu.blocked_id = cp2.user_id
                WHERE cp1.chat_id = c.id
            )".to_string());
        }
    }

    if let Some(archived) = is_archived {
        if archived {
            conditions.push("EXISTS (SELECT 1 FROM archived_chats ac WHERE ac.chat_id = c.id)".to_string());
        } else {
            conditions.push("NOT EXISTS (SELECT 1 FROM archived_chats ac WHERE ac.chat_id = c.id)".to_string());
        }
    }

    let where_clause = format!("WHERE {}", conditions.join(" AND "));
    count_query.push(&where_clause);
    query.push(&where_clause);

    query.push(" GROUP BY c.id ORDER BY last_message_at DESC NULLS LAST, c.created_at DESC LIMIT ");
    query.push_bind(limit as i64);
    query.push(" OFFSET ");
    query.push_bind(offset as i64);

    let total: i64 = match count_query.build_query_scalar().fetch_one(&app_state.db).await {
        Ok(t) => t,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success": false, "message": "Failed to count chats"}))).into_response(),
    };

    let rows = match query.build().fetch_all(&app_state.db).await {
        Ok(r) => r,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success": false, "message": "Failed to fetch chats"}))).into_response(),
    };

    let mut chats = Vec::new();

    for row in rows {
        let chat_id: Uuid = row.get("id");
        
        // Fetch participants for this chat
        let participants_rows = sqlx::query(
            "SELECT u.id, u.name, u.email, u.profile_picture, cp.joined_at 
             FROM chat_participants cp
             JOIN users u ON cp.user_id = u.id
             WHERE cp.chat_id = $1"
        )
        .bind(chat_id)
        .fetch_all(&app_state.db)
        .await
        .unwrap_or_default();

        let mut participants = Vec::new();
        for p_row in participants_rows {
            participants.push(AdminUserChatParticipant {
                id: p_row.get("id"),
                name: p_row.try_get("name").unwrap_or_default(),
                email: p_row.try_get("email").unwrap_or_default(),
                profile_picture: p_row.try_get("profile_picture").unwrap_or(None),
                joined_at: p_row.get("joined_at"),
            });
        }

        // Check if blocked
        let is_blocked: bool = sqlx::query_scalar(
            "SELECT EXISTS (
                SELECT 1 FROM chat_participants cp1
                JOIN chat_participants cp2 ON cp1.chat_id = cp2.chat_id AND cp1.user_id != cp2.user_id
                JOIN blocked_users bu ON bu.blocker_id = cp1.user_id AND bu.blocked_id = cp2.user_id
                WHERE cp1.chat_id = $1
            )"
        )
        .bind(chat_id)
        .fetch_one(&app_state.db)
        .await
        .unwrap_or(false);

        // Check if archived
        let is_archived: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM archived_chats WHERE chat_id = $1)"
        )
        .bind(chat_id)
        .fetch_one(&app_state.db)
        .await
        .unwrap_or(false);

        chats.push(AdminUserChatResponse {
            id: chat_id,
            name: row.try_get("name").unwrap_or(None),
            created_at: row.get("created_at"),
            participants,
            last_message: row.try_get("last_message").unwrap_or(None),
            last_message_at: row.try_get("last_message_at").unwrap_or(None),
            is_blocked,
            is_archived,
        });
    }

    let resp = AdminUserChatsListResponse {
        success: true,
        data: AdminUserChatsListData {
            total,
            chats,
        },
    };

    (StatusCode::OK, Json(resp)).into_response()
}

pub async fn get_user_chat_messages_admin(
    State(app_state): State<AppState>,
    Path(chat_id): Path<Uuid>,
    Query(q): Query<AdminUserChatMessagesQuery>,
) -> impl axum::response::IntoResponse {
    let limit = q.limit.unwrap_or(50).clamp(1, 100);
    let page = q.page.unwrap_or(1).max(1);
    let offset = (page - 1) * limit;

    let count_query = sqlx::query_scalar("SELECT COUNT(id) FROM messages WHERE chat_id = $1")
        .bind(chat_id);
    
    let total: i64 = match count_query.fetch_one(&app_state.db).await {
        Ok(t) => t,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success": false, "message": "Failed to count messages"}))).into_response(),
    };

    let query = sqlx::query(
        "SELECT m.id, m.chat_id, m.sender_id, u.name as sender_name, u.email as sender_email, m.content, m.created_at
         FROM messages m
         LEFT JOIN users u ON m.sender_id = u.id
         WHERE m.chat_id = $1
         ORDER BY m.created_at DESC
         LIMIT $2 OFFSET $3"
    )
    .bind(chat_id)
    .bind(limit as i64)
    .bind(offset as i64);

    let rows = match query.fetch_all(&app_state.db).await {
        Ok(r) => r,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success": false, "message": "Failed to fetch messages"}))).into_response(),
    };

    let messages = rows.into_iter().map(|row| {
        AdminUserChatMessageResponse {
            id: row.get("id"),
            chat_id: row.get("chat_id"),
            sender_id: row.try_get("sender_id").unwrap_or(None),
            sender_name: row.try_get("sender_name").unwrap_or(None),
            sender_email: row.try_get("sender_email").unwrap_or(None),
            content: row.get("content"),
            created_at: row.get("created_at"),
        }
    }).collect();

    let resp = AdminUserChatMessagesListResponse {
        success: true,
        data: AdminUserChatMessagesListData {
            total,
            messages,
        },
    };

    (StatusCode::OK, Json(resp)).into_response()
}

pub async fn force_delete_chat_admin(
    State(app_state): State<AppState>,
    Path(chat_id): Path<Uuid>,
) -> impl axum::response::IntoResponse {
    let mut tx = match app_state.db.begin().await {
        Ok(t) => t,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success":false,"message":format!("DB error: {}", e)}))).into_response(),
    };

    // Because of ON DELETE CASCADE on chat_participants and messages, we just need to delete the chat.
    match sqlx::query("DELETE FROM chats WHERE id = $1")
        .bind(chat_id)
        .execute(&mut *tx)
        .await 
    {
        Ok(res) => {
            if res.rows_affected() == 0 {
                return (StatusCode::NOT_FOUND, Json(json!({"success":false,"message":"Chat not found"}))).into_response();
            }
        },
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success":false,"message":format!("DB error: {}", e)}))).into_response(),
    };

    // Optional: Log the admin action
    let _ = sqlx::query("INSERT INTO admin_action_logs (action_type, entity_type, entity_id, description) VALUES ($1, $2, $3, $4)")
        .bind("FORCE_DELETE")
        .bind("CHAT")
        .bind(chat_id.to_string())
        .bind("Admin force deleted a chat and all its messages.")
        .execute(&mut *tx)
        .await;

    if let Err(e) = tx.commit().await {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success":false,"message":format!("DB error: {}", e)}))).into_response();
    }

    (StatusCode::OK, Json(json!({"success":true,"message":"Chat deleted successfully"}))).into_response()
}

pub async fn force_delete_message_admin(
    State(app_state): State<AppState>,
    Path(message_id): Path<Uuid>,
) -> impl axum::response::IntoResponse {
    let mut tx = match app_state.db.begin().await {
        Ok(t) => t,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success":false,"message":format!("DB error: {}", e)}))).into_response(),
    };

    match sqlx::query("DELETE FROM messages WHERE id = $1")
        .bind(message_id)
        .execute(&mut *tx)
        .await 
    {
        Ok(res) => {
            if res.rows_affected() == 0 {
                return (StatusCode::NOT_FOUND, Json(json!({"success":false,"message":"Message not found"}))).into_response();
            }
        },
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success":false,"message":format!("DB error: {}", e)}))).into_response(),
    };

    // Optional: Log the admin action
    let _ = sqlx::query("INSERT INTO admin_action_logs (action_type, entity_type, entity_id, description) VALUES ($1, $2, $3, $4)")
        .bind("FORCE_DELETE")
        .bind("MESSAGE")
        .bind(message_id.to_string())
        .bind("Admin force deleted a specific message.")
        .execute(&mut *tx)
        .await;

    if let Err(e) = tx.commit().await {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success":false,"message":format!("DB error: {}", e)}))).into_response();
    }

    (StatusCode::OK, Json(json!({"success":true,"message":"Message deleted successfully"}))).into_response()
}
