use crate::server::theme::ThemePalette;
use crate::state::{AppState, StatusResponse};
use axum::{
    http::{header, HeaderValue},
    response::{IntoResponse, Json},
};

const INDEX_HTML: &str = include_str!("../../web/dist/index.html");
const FAVICON_SVG: &str = include_str!("../../web/src/assets/icons/logo.svg");
const FAVICON_ICO: &[u8] = include_bytes!("../../assets/favicon.ico");

pub fn validate_assets() -> Result<(), crate::error::SoundThisError> {
    if INDEX_HTML.trim().is_empty() {
        return Err(crate::error::SoundThisError::Network(
            "Embedded index.html is empty".into(),
        ));
    }
    if !INDEX_HTML.contains("<!DOCTYPE html>") {
        return Err(crate::error::SoundThisError::Network(
            "Embedded index.html missing DOCTYPE".into(),
        ));
    }
    if FAVICON_SVG.trim().is_empty() {
        return Err(crate::error::SoundThisError::Network(
            "Embedded favicon.svg is empty".into(),
        ));
    }
    if FAVICON_ICO.is_empty() {
        return Err(crate::error::SoundThisError::Network(
            "Embedded favicon.ico is empty".into(),
        ));
    }
    Ok(())
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema, PartialEq, Eq)]
pub struct ToggleResponse {
    #[schema(example = true)]
    pub ok: bool,
    #[schema(example = true)]
    pub enabled: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema, PartialEq, Eq)]
pub struct ThemeReloadResponse {
    #[schema(example = true)]
    pub ok: bool,
    pub theme: crate::server::theme::ThemePalette,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema, PartialEq, Eq)]
pub struct CopyResponse {
    #[schema(example = true)]
    pub ok: bool,
    #[schema(example = true)]
    pub copied: bool,
    #[schema(example = "http://192.168.1.100:9697")]
    pub url: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema, PartialEq, Eq)]
pub struct KeepStreamingResponse {
    #[schema(example = false)]
    pub keep: bool,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, utoipa::ToSchema, PartialEq, Eq)]
pub struct KeepStreamingPayload {
    #[schema(example = true)]
    pub keep: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema, PartialEq, Eq)]
pub struct SetKeepStreamingResponse {
    #[schema(example = true)]
    pub ok: bool,
    #[schema(example = true)]
    pub keep: bool,
}

#[utoipa::path(
    get,
    path = "/",
    tag = "Web UI",
    summary = "Web Audio Player UI",
    description = "Serves the single-file bundled HTML/JS/CSS web audio player.",
    responses(
        (status = 200, description = "HTML web application", content_type = "text/html")
    )
)]
pub async fn handle_index() -> impl IntoResponse {
    (
        [(
            header::CONTENT_TYPE,
            HeaderValue::from_static("text/html; charset=utf-8"),
        )],
        INDEX_HTML,
    )
}

#[utoipa::path(
    get,
    path = "/favicon.svg",
    tag = "Web UI",
    summary = "Web Player Logo / Favicon SVG",
    description = "Serves the embedded SVG logo used as the web favicon and UI header icon.",
    responses(
        (status = 200, description = "SVG image", content_type = "image/svg+xml")
    )
)]
pub async fn handle_favicon_svg() -> impl IntoResponse {
    (
        [(
            header::CONTENT_TYPE,
            HeaderValue::from_static("image/svg+xml"),
        )],
        FAVICON_SVG,
    )
}

#[utoipa::path(
    get,
    path = "/favicon.ico",
    tag = "Web UI",
    summary = "Root Favicon ICO",
    description = "Serves the embedded root ICO favicon.",
    responses(
        (status = 200, description = "ICO image", content_type = "image/x-icon")
    )
)]
pub async fn handle_favicon_ico() -> impl IntoResponse {
    (
        [(
            header::CONTENT_TYPE,
            HeaderValue::from_static("image/x-icon"),
        )],
        FAVICON_ICO,
    )
}

pub async fn handle_favicon_fallback(
    axum::extract::Path(file): axum::extract::Path<String>,
) -> impl IntoResponse {
    if file.ends_with(".svg") {
        (
            [
                (
                    header::CONTENT_TYPE,
                    HeaderValue::from_static("image/svg+xml"),
                ),
                (
                    header::CACHE_CONTROL,
                    HeaderValue::from_static("public, max-age=86400"),
                ),
            ],
            FAVICON_SVG,
        )
            .into_response()
    } else if file.ends_with(".ico") {
        (
            [
                (
                    header::CONTENT_TYPE,
                    HeaderValue::from_static("image/x-icon"),
                ),
                (
                    header::CACHE_CONTROL,
                    HeaderValue::from_static("public, max-age=86400"),
                ),
            ],
            FAVICON_ICO,
        )
            .into_response()
    } else {
        (axum::http::StatusCode::NOT_FOUND, "Not Found").into_response()
    }
}

#[utoipa::path(
    get,
    path = "/health",
    tag = "Health",
    summary = "Health Check",
    description = "Returns OK if the SoundThis server is running and responsive.",
    responses(
        (status = 200, description = "Server is healthy", body = String, content_type = "text/plain")
    )
)]
pub async fn handle_health() -> &'static str {
    "OK"
}

#[utoipa::path(
    get,
    path = "/api/v1/status",
    tag = "Status",
    summary = "Server & Streaming Status",
    description = "Returns comprehensive server status, audio streaming state, network configuration, and list of connected listeners.",
    responses(
        (status = 200, description = "Current server and listener status", body = StatusResponse)
    )
)]
pub async fn handle_api_status(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> impl IntoResponse {
    Json(state.to_status_response().await)
}

#[utoipa::path(
    post,
    path = "/api/v1/toggle",
    tag = "Streaming",
    summary = "Toggle Audio Streaming",
    description = "Toggles global desktop audio capture and broadcast between enabled and disabled (muted).",
    responses(
        (status = 200, description = "Streaming state toggled successfully", body = ToggleResponse)
    )
)]
pub async fn handle_api_toggle(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> impl IntoResponse {
    let now_enabled = state.toggle_enabled();
    Json(ToggleResponse {
        ok: true,
        enabled: now_enabled,
    })
}

#[utoipa::path(
    post,
    path = "/api/v1/enable",
    tag = "Streaming",
    summary = "Enable Audio Streaming",
    description = "Enables desktop audio capture and broadcast to all connected WebSocket listeners.",
    responses(
        (status = 200, description = "Streaming enabled successfully", body = ToggleResponse)
    )
)]
pub async fn handle_api_enable(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> impl IntoResponse {
    state.set_enabled(true);
    Json(ToggleResponse {
        ok: true,
        enabled: true,
    })
}

#[utoipa::path(
    post,
    path = "/api/v1/disable",
    tag = "Streaming",
    summary = "Disable Audio Streaming",
    description = "Disables (mutes) desktop audio broadcast without disconnecting active clients.",
    responses(
        (status = 200, description = "Streaming disabled successfully", body = ToggleResponse)
    )
)]
pub async fn handle_api_disable(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> impl IntoResponse {
    state.set_enabled(false);
    Json(ToggleResponse {
        ok: true,
        enabled: false,
    })
}

#[utoipa::path(
    get,
    path = "/api/v1/theme",
    tag = "Theme",
    summary = "Get Theme Palette",
    description = "Returns the current Omarchy system theme color palette (Tokyo Night default or loaded from colors.toml).",
    responses(
        (status = 200, description = "Current theme colors", body = ThemePalette)
    )
)]
pub async fn handle_api_theme() -> impl IntoResponse {
    let palette = crate::server::theme::ThemePalette::load_current();
    Json(palette)
}

#[utoipa::path(
    post,
    path = "/api/v1/theme/reload",
    tag = "Theme",
    summary = "Reload Theme Palette",
    description = "Reloads theme colors from disk and broadcasts update via WebSocket to all connected web clients in real time.",
    responses(
        (status = 200, description = "Theme reloaded and broadcasted", body = ThemeReloadResponse)
    )
)]
pub async fn handle_api_theme_reload(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> impl IntoResponse {
    let palette = crate::server::theme::ThemePalette::load_current();
    state.broadcast_theme(palette.clone());
    Json(ThemeReloadResponse {
        ok: true,
        theme: palette,
    })
}

#[utoipa::path(
    get,
    path = "/api/v1/qr.svg",
    tag = "Theme",
    summary = "Get QR Code SVG",
    description = "Generates a dynamic QR code SVG for connecting mobile devices, styled with the current theme foreground and background colors.",
    responses(
        (status = 200, description = "SVG QR code image", content_type = "image/svg+xml"),
        (status = 500, description = "Internal error generating QR SVG", body = String)
    )
)]
pub async fn handle_api_qr_svg(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> impl IntoResponse {
    let palette = crate::server::theme::ThemePalette::load_current();
    let url = state.get_url();
    match palette.generate_qr_svg(&url) {
        Ok(svg) => (
            [(
                header::CONTENT_TYPE,
                HeaderValue::from_static("image/svg+xml"),
            )],
            svg,
        )
            .into_response(),
        Err(e) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            format!("Error generating QR: {e}"),
        )
            .into_response(),
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/copy",
    tag = "Desktop",
    summary = "Copy Server URL to Clipboard",
    description = "Copies the LAN web player URL to the system clipboard using wl-copy (Wayland) or xclip (X11).",
    responses(
        (status = 200, description = "URL copied to clipboard", body = CopyResponse)
    )
)]
pub async fn handle_api_copy(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> impl IntoResponse {
    let url = state.get_url();
    let copied = crate::ui::copy_to_clipboard(&url);
    Json(CopyResponse {
        ok: true,
        copied,
        url,
    })
}

#[utoipa::path(
    get,
    path = "/api/v1/config/keep_streaming",
    tag = "Desktop",
    summary = "Get Keep Streaming Setting",
    description = "Retrieves the persistent desktop GUI setting whether audio streaming should continue when the desktop window is closed.",
    responses(
        (status = 200, description = "Keep streaming setting value", body = KeepStreamingResponse)
    )
)]
pub async fn handle_api_get_keep_streaming() -> impl IntoResponse {
    let cfg = crate::ui::read_gui_config();
    Json(KeepStreamingResponse {
        keep: cfg.keep_streaming,
    })
}

#[utoipa::path(
    post,
    path = "/api/v1/config/keep_streaming",
    tag = "Desktop",
    summary = "Set Keep Streaming Setting",
    description = "Updates the persistent desktop GUI setting for background streaming when the window is closed.",
    request_body = KeepStreamingPayload,
    responses(
        (status = 200, description = "Setting saved successfully", body = SetKeepStreamingResponse)
    )
)]
pub async fn handle_api_set_keep_streaming(
    Json(payload): Json<KeepStreamingPayload>,
) -> impl IntoResponse {
    let mut cfg = crate::ui::read_gui_config();
    cfg.keep_streaming = payload.keep;
    let ok = crate::ui::save_gui_config(&cfg).is_ok();
    Json(SetKeepStreamingResponse {
        ok,
        keep: payload.keep,
    })
}

/// Redirects unversioned /api/* requests to /api/v1/* preserving method and query string.
pub async fn handle_redirect_to_v1(uri: axum::http::Uri) -> impl IntoResponse {
    let path = uri.path();
    let subpath = path.strip_prefix("/api/").unwrap_or("");
    if subpath.starts_with("v1/") || subpath == "v1" {
        return (
            axum::http::StatusCode::NOT_FOUND,
            [(
                axum::http::header::CONTENT_TYPE,
                HeaderValue::from_static("text/plain"),
            )],
            "Not Found",
        )
            .into_response();
    }
    let target = if let Some(query) = uri.query() {
        format!("/api/v1/{subpath}?{query}")
    } else {
        format!("/api/v1/{subpath}")
    };
    (
        axum::http::StatusCode::PERMANENT_REDIRECT,
        [(axum::http::header::LOCATION, target)],
    )
        .into_response()
}

/// Redirects /v1/* requests without /api prefix to /api/v1/* preserving method and query string.
pub async fn handle_redirect_to_api_v1(uri: axum::http::Uri) -> impl IntoResponse {
    let path = uri.path();
    let subpath = path.strip_prefix("/v1/").unwrap_or("");
    let target = if let Some(query) = uri.query() {
        format!("/api/v1/{subpath}?{query}")
    } else {
        format!("/api/v1/{subpath}")
    };
    (
        axum::http::StatusCode::PERMANENT_REDIRECT,
        [(axum::http::header::LOCATION, target)],
    )
}
