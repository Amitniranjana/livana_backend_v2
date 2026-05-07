use axum::{
    extract::{Query, State, WebSocketUpgrade, ws::{WebSocket, Message}},
    response::IntoResponse,
};
use futures::{sink::SinkExt, stream::StreamExt};
use serde::Deserialize;
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::{app_state::AppState, utils::auth::decode_jwt};

#[derive(Deserialize)]
pub struct WsAuthQuery {
    pub token: String,
}

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    Query(query): Query<WsAuthQuery>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    // Authenticate the connection via the token query parameter
    let claims = match decode_jwt(&query.token, &state.jwt_secret) {
        Ok(c) => c,
        Err(_) => {
            return axum::http::StatusCode::UNAUTHORIZED.into_response();
        }
    };

    let user_id = match Uuid::parse_str(&claims.sub) {
        Ok(id) => id,
        Err(_) => {
            return axum::http::StatusCode::UNAUTHORIZED.into_response();
        }
    };

    ws.on_upgrade(move |socket| handle_socket(socket, user_id, state))
}

async fn handle_socket(socket: WebSocket, user_id: Uuid, state: AppState) {
    let (mut sender, mut receiver) = socket.split();

    // Create a channel for pushing messages to this connection
    let (tx, mut rx) = mpsc::channel::<String>(100);

    // Store in DashMap
    state.active_sockets.insert(user_id, tx);

    // Forward messages from mpsc channel to the actual WebSocket
    let mut send_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if sender.send(Message::Text(msg.into())).await.is_err() {
                break;
            }
        }
    });

    // Listen for incoming messages (ping/pong or client events) or disconnects
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            // Process incoming messages if necessary, or just keep connection alive
            if let Message::Close(_) = msg {
                break;
            }
        }
    });

    // Wait for either task to finish (meaning connection closed)
    tokio::select! {
        _ = (&mut send_task) => recv_task.abort(),
        _ = (&mut recv_task) => send_task.abort(),
    };

    // Remove from DashMap on disconnect
    state.active_sockets.remove(&user_id);
}
