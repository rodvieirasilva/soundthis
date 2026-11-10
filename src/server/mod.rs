pub mod http;
pub mod ws;

use crate::audio::AudioBroadcaster;
use crate::config::ServerConfig;
use crate::error::SoundThisError;
use crate::state::AppState;
use axum::{
    http::HeaderValue,
    routing::{get, post},
    Router,
};
use std::net::SocketAddr;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tracing::info;

pub fn build_app(
    state: AppState,
    broadcaster: Arc<AudioBroadcaster>,
    config: &ServerConfig,
) -> Router {
    let mut cors = CorsLayer::new();

    // 1. Configure Origins
    if config.cors.allow_origins.iter().any(|o| o == "*") {
        cors = cors.allow_origin(Any);
    } else {
        let origins: Vec<HeaderValue> = config
            .cors
            .allow_origins
            .iter()
            .filter_map(|o| o.parse().ok())
            .collect();
        cors = cors.allow_origin(origins);
    }

    // 2. Configure Methods
    let methods: Vec<axum::http::Method> = config
        .cors
        .allow_methods
        .iter()
        .filter_map(|m| m.parse().ok())
        .collect();
    if !methods.is_empty() {
        cors = cors.allow_methods(methods);
    } else {
        cors = cors.allow_methods(Any);
    }

    // 3. Configure Headers
    if config.cors.allow_headers.iter().any(|h| h == "*") {
        cors = cors.allow_headers(Any);
    } else {
        let headers: Vec<axum::http::HeaderName> = config
            .cors
            .allow_headers
            .iter()
            .filter_map(|h| h.parse().ok())
            .collect();
        cors = cors.allow_headers(headers);
    }

    let state_with_broadcaster = (state.clone(), broadcaster);

    Router::new()
        // Web UI and assets
        .route("/", get(http::handle_index))
        .route("/favicon.svg", get(http::handle_favicon_svg))
        .route("/favicon.ico", get(http::handle_favicon_ico))
        .route("/favicon-{file}", get(http::handle_favicon_fallback))
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
        .layer(cors)
}

pub async fn run_server(
    state: AppState,
    broadcaster: Arc<AudioBroadcaster>,
    config: &ServerConfig,
) -> Result<(), SoundThisError> {
    let app = build_app(state.clone(), broadcaster, config);

    let bind_addr: SocketAddr = format!("{}:{}", config.host, config.port)
        .parse()
        .map_err(|e| SoundThisError::Network(format!("Invalid bind address: {e}")))?;

    let scheme = config.protocol.scheme();
    info!(
        "SoundThis listening on {scheme}://{}:{} (LAN: {scheme}://{}:{})",
        config.host, config.port, state.lan_ip, config.port
    );

    if config.protocol.is_https() {
        let _ = rustls::crypto::ring::default_provider().install_default();
        let (cert_pem, key_pem) = config.resolve_tls_pem(&state.lan_ip)?;
        let tls_config = axum_server::tls_rustls::RustlsConfig::from_pem(cert_pem, key_pem)
            .await
            .map_err(|e| SoundThisError::Network(format!("Invalid TLS configuration: {e}")))?;

        axum_server::bind_rustls(bind_addr, tls_config)
            .serve(app.into_make_service_with_connect_info::<SocketAddr>())
            .await
            .map_err(|e| SoundThisError::Network(format!("HTTPS server error: {e}")))?;
    } else {
        let listener = tokio::net::TcpListener::bind(&bind_addr)
            .await
            .map_err(|e| SoundThisError::Network(format!("Failed to bind to {bind_addr}: {e}")))?;

        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
        .map_err(|e| SoundThisError::Network(format!("HTTP server error: {e}")))?;
    }

    Ok(())
}
