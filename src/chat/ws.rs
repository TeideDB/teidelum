use crate::chat::auth;
use crate::chat::events::{ClientEvent, ServerEvent};
use crate::chat::handlers::AppState;
use axum::extract::ws::{Message, WebSocket};
use axum::{
    extract::{Query, State, WebSocketUpgrade},
    response::{IntoResponse, Response},
};
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct WsQuery {
    pub token: String,
}

pub async fn ws_upgrade(
    State(state): State<AppState>,
    Query(query): Query<WsQuery>,
    ws: WebSocketUpgrade,
) -> Response {
    let secret = match std::env::var("TEIDE_CHAT_SECRET") {
        Ok(s) if !s.is_empty() => s,
        _ => {
            return axum::http::Response::builder()
                .status(401)
                .body(axum::body::Body::empty())
                .unwrap()
                .into_response();
        }
    };

    let claims = match auth::validate_token(&secret, &query.token) {
        Ok(c) => c,
        Err(_) => {
            return axum::http::Response::builder()
                .status(401)
                .body(axum::body::Body::empty())
                .unwrap()
                .into_response();
        }
    };

    ws.on_upgrade(move |socket| handle_socket(state, claims, socket))
}

async fn handle_socket(state: AppState, claims: auth::Claims, socket: WebSocket) {
    let user_id = claims.user_id;
    let mut rx = state.hub.connect(user_id).await;

    let (mut ws_sink, mut ws_stream) = socket.split();

    // Send hello
    let hello = serde_json::to_string(&ServerEvent::Hello).unwrap();
    if ws_sink.send(Message::Text(hello.into())).await.is_err() {
        return;
    }

    // Broadcast presence online
    let presence = ServerEvent::PresenceChange {
        user: user_id.to_string(),
        presence: "online".to_string(),
    };
    let online = state.hub.online_users().await;
    for uid in online {
        state.hub.send_to_user(uid, &presence).await;
    }

    let state_clone = state.clone();
    let user_id_clone = user_id;

    // Task: forward hub events to WebSocket
    let send_task = tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            if ws_sink
                .send(Message::Text((*msg).clone().into()))
                .await
                .is_err()
            {
                break;
            }
        }
    });

    // Task: process incoming client messages
    let recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = ws_stream.next().await {
            match msg {
                Message::Text(text) => {
                    if let Ok(event) = serde_json::from_str::<ClientEvent>(&text) {
                        match event {
                            ClientEvent::Typing { channel } => {
                                if let Ok(ch_id) = channel.parse::<i64>() {
                                    if state_clone
                                        .hub
                                        .is_channel_member(ch_id, user_id_clone)
                                        .await
                                        && state_clone
                                            .hub
                                            .should_broadcast_typing(user_id_clone, ch_id)
                                            .await
                                    {
                                        let typing_event = ServerEvent::Typing {
                                            channel: channel.clone(),
                                            user: user_id_clone.to_string(),
                                        };
                                        state_clone
                                            .hub
                                            .broadcast_to_channel(ch_id, &typing_event)
                                            .await;
                                    }
                                }
                            }
                            ClientEvent::Ping => {
                                // Pong handled by axum automatically
                            }
                        }
                    }
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
    });

    // Wait for either task to finish
    tokio::select! {
        _ = send_task => {},
        _ = recv_task => {},
    }

    // Cleanup
    state.hub.disconnect(user_id).await;

    // Broadcast offline if no more connections
    if !state.hub.is_online(user_id).await {
        let offline = ServerEvent::PresenceChange {
            user: user_id.to_string(),
            presence: "offline".to_string(),
        };
        let online = state.hub.online_users().await;
        for uid in online {
            state.hub.send_to_user(uid, &offline).await;
        }
    }
}
