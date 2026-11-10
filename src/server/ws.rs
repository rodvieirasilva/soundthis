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

#[derive(Deserialize, Default)]
pub struct WsQuery {
    pub listen: Option<String>,
    pub paused: Option<String>,
}

impl WsQuery {
    pub fn is_initially_paused(&self) -> bool {
        if let Some(ref p) = self.paused {
            let p_lower = p.to_lowercase();
            if p_lower == "true" || p_lower == "1" || p_lower == "yes" {
                return true;
            }
        }
        if let Some(ref l) = self.listen {
            let l_lower = l.to_lowercase();
            if l_lower == "false" || l_lower == "0" || l_lower == "no" {
                return true;
            }
        }
        false
    }
}

#[derive(Deserialize)]
struct ClientMessage {
    #[allow(dead_code)]
    #[serde(rename = "type")]
    msg_type: Option<String>,
    name: Option<String>,
    playing: Option<bool>,
    paused: Option<bool>,
}

pub async fn handle_ws(
    ws: WebSocketUpgrade,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    axum::extract::Query(query): axum::extract::Query<WsQuery>,
    State((state, broadcaster)): State<(AppState, Arc<AudioBroadcaster>)>,
) -> impl IntoResponse {
    let initially_paused = query.is_initially_paused();
    ws.on_upgrade(move |socket| {
        handle_ws_session(socket, addr, state, broadcaster, initially_paused)
    })
}

async fn handle_ws_session(
    mut socket: WebSocket,
    addr: SocketAddr,
    state: AppState,
    broadcaster: Arc<AudioBroadcaster>,
    initially_paused: bool,
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
        listening: !initially_paused,
    };

    state.add_client(initial_info).await;
    info!(
        "Client connected: {addr} (listening: {})",
        !initially_paused
    );

    let mut audio_rx = broadcaster.subscribe();
    let mut client_paused = initially_paused;

    loop {
        tokio::select! {
            // Receive messages from the client (e.g. name identification, playback toggle)
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
                            if let Some(msg_type) = client_msg.msg_type.as_deref() {
                                match msg_type {
                                    "pause" => {
                                        client_paused = true;
                                        state.update_client_listening(&client_id, false).await;
                                    }
                                    "resume" | "play" => {
                                        client_paused = false;
                                        state.update_client_listening(&client_id, true).await;
                                    }
                                    _ => {}
                                }
                            }
                            if let Some(playing) = client_msg.playing {
                                client_paused = !playing;
                                state.update_client_listening(&client_id, playing).await;
                            } else if let Some(paused) = client_msg.paused {
                                client_paused = paused;
                                state.update_client_listening(&client_id, !paused).await;
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

            // Receive audio chunks from broadcaster and stream to client only when NOT paused
            audio_result = audio_rx.recv() => {
                match audio_result {
                    Ok(chunk) => {
                        if !client_paused
                            && state.is_enabled()
                            && socket
                                .send(Message::Binary((*chunk).clone().into()))
                                .await
                                .is_err()
                        {
                            break;
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
