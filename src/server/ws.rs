use crate::audio::AudioBroadcaster;
use crate::state::{AppState, ClientInfo};
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        ConnectInfo, State,
    },
    response::IntoResponse,
};
use serde::Deserialize;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::broadcast::error::RecvError;
use tracing::{debug, info};

#[derive(Deserialize)]
struct ClientMessage {
    #[allow(dead_code)]
    #[serde(rename = "type")]
    msg_type: Option<String>,
    name: Option<String>,
}

pub async fn handle_ws(
    ws: WebSocketUpgrade,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State((state, broadcaster)): State<(AppState, Arc<AudioBroadcaster>)>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_ws_session(socket, addr, state, broadcaster))
}

async fn handle_ws_session(
    mut socket: WebSocket,
    addr: SocketAddr,
    state: AppState,
    broadcaster: Arc<AudioBroadcaster>,
) {
    let client_id = format!("{addr}");
    let client_ip = addr.ip().to_string();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let initial_info = ClientInfo {
        id: client_id.clone(),
        name: "Device".to_string(),
        ip: client_ip,
        connected_at: now,
    };

    state.add_client(initial_info).await;
    info!("Client connected: {addr}");

    let mut audio_rx = broadcaster.subscribe();

    loop {
        tokio::select! {
            // Receive messages from the client (e.g. name identification)
            incoming = socket.recv() => {
                match incoming {
                    Some(Ok(Message::Text(text))) => {
                        if let Ok(client_msg) = serde_json::from_str::<ClientMessage>(&text) {
                            if let Some(name) = client_msg.name {
                                let clean = name.trim();
                                if !clean.is_empty() {
                                    state.update_client_name(&client_id, clean.to_string()).await;
                                }
                            }
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => {
                        break;
                    }
                    Some(Err(err)) => {
                        debug!("WebSocket error from {addr}: {err}");
                        break;
                    }
                    _ => {}
                }
            }

            // Receive audio chunks from broadcaster and stream to client
            audio_result = audio_rx.recv() => {
                match audio_result {
                    Ok(chunk) => {
                        if state.is_enabled() {
                            if socket.send(Message::Binary((*chunk).clone().into())).await.is_err() {
                                break;
                            }
                        }
                    }
                    Err(RecvError::Lagged(n)) => {
                        debug!("Client {addr} lagged by {n} audio frames; skipping old frames to maintain low latency");
                    }
                    Err(RecvError::Closed) => {
                        break;
                    }
                }
            }
        }
    }

    state.remove_client(&client_id).await;
    info!("Client disconnected: {addr}");
}
