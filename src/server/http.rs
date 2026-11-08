use crate::state::AppState;
use axum::{
    http::{header, HeaderValue},
    response::{IntoResponse, Json},
};

const INDEX_HTML: &str = include_str!("../../web/dist/index.html");
const FAVICON_SVG: &str = include_str!("../../web/favicon.svg");
const FAVICON_ICO: &[u8] = include_bytes!("../../web/favicon.ico");

pub fn validate_assets() -> Result<(), crate::error::SoundThisError> {
    if INDEX_HTML.trim().is_empty() {
        return Err(crate::error::SoundThisError::Network("Embedded index.html is empty".into()));
    }
    if !INDEX_HTML.contains("<!DOCTYPE html>") {
        return Err(crate::error::SoundThisError::Network("Embedded index.html missing DOCTYPE".into()));
    }
    if FAVICON_SVG.trim().is_empty() {
        return Err(crate::error::SoundThisError::Network("Embedded favicon.svg is empty".into()));
    }
    if FAVICON_ICO.is_empty() {
        return Err(crate::error::SoundThisError::Network("Embedded favicon.ico is empty".into()));
    }
    Ok(())
}

pub async fn handle_index() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, HeaderValue::from_static("text/html; charset=utf-8"))],
        INDEX_HTML,
    )
}

pub async fn handle_favicon_svg() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, HeaderValue::from_static("image/svg+xml"))],
        FAVICON_SVG,
    )
}

pub async fn handle_favicon_ico() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, HeaderValue::from_static("image/x-icon"))],
        FAVICON_ICO,
    )
}

pub async fn handle_health() -> &'static str {
    "OK"
}

pub async fn handle_api_status(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> impl IntoResponse {
    Json(state.to_status_response().await)
}

pub async fn handle_api_toggle(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> impl IntoResponse {
    let now_enabled = state.toggle_enabled();
    Json(serde_json::json!({
        "ok": true,
        "enabled": now_enabled
    }))
}

pub async fn handle_api_enable(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> impl IntoResponse {
    state.set_enabled(true);
    Json(serde_json::json!({
        "ok": true,
        "enabled": true
    }))
}

pub async fn handle_api_disable(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> impl IntoResponse {
    state.set_enabled(false);
    Json(serde_json::json!({
        "ok": true,
        "enabled": false
    }))
}
