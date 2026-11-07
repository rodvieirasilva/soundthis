pub mod http;
pub mod ws;

use crate::audio::AudioBroadcaster;
use crate::error::SoundThisError;
use crate::state::AppState;
use axum::{
    routing::{get, post},
    Router,
};
use std::net::SocketAddr;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tracing::info;

pub async fn run_server(
    state: AppState,
    broadcaster: Arc<AudioBroadcaster>,
    host: &str,
    port: u16,
) -> Result<(), SoundThisError> {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let state_with_broadcaster = (state.clone(), broadcaster);

    let app = Router::new()
        // Web UI and assets
        .route("/", get(http::handle_index))
        .route("/favicon.svg", get(http::handle_favicon_svg))
        .route("/favicon.ico", get(http::handle_favicon_ico))
        .route("/health", get(http::handle_health))
        // API routes
        .route("/api/status", get(http::handle_api_status))
        .route("/api/toggle", post(http::handle_api_toggle))
        .route("/api/enable", post(http::handle_api_enable))
        .route("/api/disable", post(http::handle_api_disable))
        .with_state(state.clone())
        // WebSocket streaming route
        .route("/ws", get(ws::handle_ws))
        .with_state(state_with_broadcaster)
        .layer(cors);

    let bind_addr: SocketAddr = format!("{host}:{port}")
        .parse()
        .map_err(|e| SoundThisError::Network(format!("Invalid bind address: {e}")))?;

    info!(
        "SoundThis listening on http://{}:{} (LAN: http://{}:{})",
        host, port, state.lan_ip, port
    );

    let listener = tokio::net::TcpListener::bind(&bind_addr)
        .await
        .map_err(|e| SoundThisError::Network(format!("Failed to bind to {bind_addr}: {e}")))?;

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .map_err(|e| SoundThisError::Network(format!("Server error: {e}")))?;

    Ok(())
}
