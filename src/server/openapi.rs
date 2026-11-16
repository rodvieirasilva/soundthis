use axum::response::{IntoResponse, Json, Redirect};
use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    paths(
        crate::server::http::handle_health,
        crate::server::http::handle_api_status,
        crate::server::http::handle_api_toggle,
        crate::server::http::handle_api_enable,
        crate::server::http::handle_api_disable,
        crate::server::http::handle_api_theme,
        crate::server::http::handle_api_theme_reload,
        crate::server::http::handle_api_qr_svg,
        crate::server::http::handle_api_copy,
        crate::server::http::handle_api_get_keep_streaming,
        crate::server::http::handle_api_set_keep_streaming,
        crate::server::http::handle_index,
        crate::server::http::handle_favicon_svg,
        crate::server::http::handle_favicon_ico,
        crate::server::ws::handle_ws,
    ),
    components(
        schemas(
            crate::state::StatusResponse,
            crate::state::ClientInfo,
            crate::server::theme::ThemePalette,
            crate::server::http::ToggleResponse,
            crate::server::http::ThemeReloadResponse,
            crate::server::http::CopyResponse,
            crate::server::http::KeepStreamingResponse,
            crate::server::http::KeepStreamingPayload,
            crate::server::http::SetKeepStreamingResponse,
        )
    ),
    tags(
        (name = "Health", description = "Server health endpoints"),
        (name = "Status", description = "Server status and connected client inspection"),
        (name = "Streaming", description = "Audio streaming controls and WebSocket endpoint"),
        (name = "Desktop", description = "Desktop integration and persistent settings"),
        (name = "Theme", description = "Omarchy OS theme synchronization and QR codes"),
        (name = "Web UI", description = "Embedded single-file web client and icons"),
    ),
    info(
        title = "SoundThis API",
        version = env!("CARGO_PKG_VERSION"),
        description = "Ultra-low latency, real-time desktop audio streaming server for Omarchy Linux",
        license(name = "MIT"),
        contact(name = "SoundThis Maintainers", email = "rodvieirasilva@gmail.com")
    )
)]
pub struct ApiDoc;

pub async fn handle_openapi_json() -> impl IntoResponse {
    Json(ApiDoc::openapi())
}

pub async fn handle_swagger_redirect() -> impl IntoResponse {
    Redirect::permanent("/swagger-ui/")
}
