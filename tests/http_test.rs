use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use soundthis::audio::AudioBroadcaster;
use soundthis::config::{Protocol, ServerConfig};
use soundthis::server::build_app;
use soundthis::state::{AppState, ClientInfo};
use std::sync::Arc;
use tower::ServiceExt;

#[tokio::test]
async fn test_app_state_management() {
    let state = AppState::new(9697, "192.168.1.100".into(), Protocol::Http);
    assert!(state.is_enabled());

    let toggled = state.toggle_enabled();
    assert!(!toggled);
    assert!(!state.is_enabled());

    state.set_enabled(true);
    assert!(state.is_enabled());

    let client_info = ClientInfo {
        id: "127.0.0.1:1234".into(),
        name: "Test Browser".into(),
        ip: "127.0.0.1".into(),
        connected_at: 1000,
        listening: true,
    };

    state.add_client(client_info).await;
    let clients = state.get_clients().await;
    assert_eq!(clients.len(), 1);
    assert_eq!(clients[0].name, "Test Browser");

    state
        .update_client_name("127.0.0.1:1234", "Updated Browser".into())
        .await;
    let clients = state.get_clients().await;
    assert_eq!(clients[0].name, "Updated Browser");

    let status = state.to_status_response().await;
    assert_eq!(status.url, "http://192.168.1.100:9697");
    assert_eq!(status.protocol, "http");
    assert_eq!(status.version, env!("CARGO_PKG_VERSION"));

    state.remove_client("127.0.0.1:1234").await;
    assert_eq!(state.get_clients().await.len(), 0);
}

#[tokio::test]
async fn test_http_api_routes() {
    let state = AppState::new(9697, "127.0.0.1".into(), Protocol::Http);
    let broadcaster = Arc::new(AudioBroadcaster::new());
    let config = ServerConfig::default();

    let app = build_app(state.clone(), broadcaster, &config);

    // Test /health
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Test / (HTML)
    let response = app
        .clone()
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get("content-type").unwrap(),
        "text/html; charset=utf-8"
    );

    // Test /favicon.svg
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/favicon.svg")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get("content-type").unwrap(),
        "image/svg+xml"
    );

    // Test /favicon.ico
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/favicon.ico")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get("content-type").unwrap(),
        "image/x-icon"
    );

    // Test hashed fallback: /favicon-BiVrq98m.svg
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/favicon-BiVrq98m.svg")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get("content-type").unwrap(),
        "image/svg+xml"
    );

    // Test hashed fallback: /favicon-nc31v7o9.ico
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/favicon-nc31v7o9.ico")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get("content-type").unwrap(),
        "image/x-icon"
    );

    // Test invalid fallback: /favicon-unknown.txt returns 404
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/favicon-unknown.txt")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    // Test /api/v1/status
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/status")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Test unversioned /api/status redirect to /api/v1/status
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/status")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::PERMANENT_REDIRECT);
    assert_eq!(
        response.headers().get("location").unwrap(),
        "/api/v1/status"
    );

    // Test unversioned redirect with query parameters
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/status?foo=bar")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::PERMANENT_REDIRECT);
    assert_eq!(
        response.headers().get("location").unwrap(),
        "/api/v1/status?foo=bar"
    );

    // Test /v1/status redirect to /api/v1/status
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/status")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::PERMANENT_REDIRECT);
    assert_eq!(
        response.headers().get("location").unwrap(),
        "/api/v1/status"
    );

    // Test /api/v1/theme
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/theme")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get("content-type").unwrap(),
        "application/json"
    );

    // Test unversioned /api/theme redirect
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/theme")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::PERMANENT_REDIRECT);
    assert_eq!(response.headers().get("location").unwrap(), "/api/v1/theme");

    // Test /api/v1/theme/reload
    let mut theme_sub = state.subscribe_theme();
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/theme/reload")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(theme_sub.try_recv().is_ok());

    // Test unversioned /api/theme/reload redirect
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/theme/reload")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::PERMANENT_REDIRECT);
    assert_eq!(
        response.headers().get("location").unwrap(),
        "/api/v1/theme/reload"
    );

    // Test /api/v1/qr.svg
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/qr.svg")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get("content-type").unwrap(),
        "image/svg+xml"
    );

    // Test unversioned /api/qr.svg redirect
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/qr.svg")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::PERMANENT_REDIRECT);
    assert_eq!(
        response.headers().get("location").unwrap(),
        "/api/v1/qr.svg"
    );

    // Test /api/v1/disable
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/disable")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(!state.is_enabled());

    // Test /api/v1/enable
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/enable")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(state.is_enabled());

    // Test /api/v1/toggle
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/toggle")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(!state.is_enabled());

    // Test unversioned /api/toggle redirect
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/toggle")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::PERMANENT_REDIRECT);
    assert_eq!(
        response.headers().get("location").unwrap(),
        "/api/v1/toggle"
    );

    // Test /api/v1/copy
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/copy")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Test GET /api/v1/config/keep_streaming
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/config/keep_streaming")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Test POST /api/v1/config/keep_streaming
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/config/keep_streaming")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"keep": true}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Test GET /openapi.json
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/openapi.json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let openapi_json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(openapi_json["openapi"].is_string());
    assert_eq!(openapi_json["info"]["title"], "SoundThis API");
    assert!(openapi_json["paths"]["/health"].is_object());
    assert!(openapi_json["paths"]["/api/v1/status"].is_object());
    assert!(openapi_json["paths"]["/api/v1/toggle"].is_object());
    assert!(openapi_json["paths"]["/api/v1/theme"].is_object());
    assert!(openapi_json["paths"]["/ws"].is_object());

    // Test GET /api-docs/openapi.json (SwaggerUi generated endpoint)
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api-docs/openapi.json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Test GET /swagger-ui/
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/swagger-ui/")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body_str = String::from_utf8_lossy(
        &axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .to_string();
    assert!(body_str.to_lowercase().contains("swagger"));

    // Test GET /swagger redirect
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/swagger")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::PERMANENT_REDIRECT);
    assert_eq!(response.headers().get("location").unwrap(), "/swagger-ui/");

    // Test GET /swagger/ redirect
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/swagger/")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::PERMANENT_REDIRECT);
    assert_eq!(response.headers().get("location").unwrap(), "/swagger-ui/");

    // Test asset validation
    assert!(soundthis::server::http::validate_assets().is_ok());

    // Test 404 for unknown API route
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/nonexistent-endpoint")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    // Test favicon fallback routes
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/favicon-custom.svg")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get("content-type").unwrap(),
        "image/svg+xml"
    );

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/favicon-custom.ico")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get("content-type").unwrap(),
        "image/x-icon"
    );

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/favicon-invalid.txt")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    // Directly test openapi handlers
    let _ = soundthis::server::openapi::handle_openapi_json().await;
    let _ = soundthis::server::openapi::handle_swagger_redirect().await;
}

#[tokio::test]
async fn test_cors_specific_headers() {
    let state = AppState::new(9697, "127.0.0.1".into(), Protocol::Http);
    let broadcaster = Arc::new(AudioBroadcaster::new());
    let mut config = ServerConfig::default();
    config.cors.allow_origins = vec!["http://mysite.local".into()];
    config.cors.allow_methods = vec!["GET".into()];
    config.cors.allow_headers = vec!["X-Custom".into()];

    let app = build_app(state, broadcaster, &config);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .header("Origin", "http://mysite.local")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get("access-control-allow-origin")
            .unwrap(),
        "http://mysite.local"
    );
}

#[tokio::test]
async fn test_all_redirect_routes_matrix() {
    let state = AppState::new(9697, "127.0.0.1".into(), Protocol::Http);
    let broadcaster = Arc::new(AudioBroadcaster::new());
    let config = ServerConfig::default();
    let app = build_app(state, broadcaster, &config);

    // List of (method, unversioned_path, expected_v1_path)
    let cases = vec![
        ("GET", "/api/status", "/api/v1/status"),
        ("GET", "/api/status?query=true", "/api/v1/status?query=true"),
        ("POST", "/api/toggle", "/api/v1/toggle"),
        ("POST", "/api/enable", "/api/v1/enable"),
        ("POST", "/api/disable", "/api/v1/disable"),
        ("GET", "/api/theme", "/api/v1/theme"),
        ("POST", "/api/theme/reload", "/api/v1/theme/reload"),
        ("GET", "/api/qr.svg", "/api/v1/qr.svg"),
        ("POST", "/api/copy", "/api/v1/copy"),
        (
            "GET",
            "/api/config/keep_streaming",
            "/api/v1/config/keep_streaming",
        ),
        (
            "POST",
            "/api/config/keep_streaming",
            "/api/v1/config/keep_streaming",
        ),
        // Direct /v1/* convenience redirects
        ("GET", "/v1/status", "/api/v1/status"),
        ("POST", "/v1/toggle", "/api/v1/toggle"),
        ("POST", "/v1/enable", "/api/v1/enable"),
        ("POST", "/v1/disable", "/api/v1/disable"),
        ("GET", "/v1/theme", "/api/v1/theme"),
        ("POST", "/v1/theme/reload", "/api/v1/theme/reload"),
        ("GET", "/v1/qr.svg", "/api/v1/qr.svg"),
        ("POST", "/v1/copy", "/api/v1/copy"),
        (
            "GET",
            "/v1/config/keep_streaming",
            "/api/v1/config/keep_streaming",
        ),
        (
            "POST",
            "/v1/config/keep_streaming",
            "/api/v1/config/keep_streaming",
        ),
    ];

    for (method, source_path, expected_target) in cases {
        let req = Request::builder()
            .method(method)
            .uri(source_path)
            .body(Body::empty())
            .unwrap();

        let response = app.clone().oneshot(req).await.unwrap();
        assert_eq!(
            response.status(),
            StatusCode::PERMANENT_REDIRECT,
            "Route {method} {source_path} did not return 308 Permanent Redirect"
        );
        let loc = response
            .headers()
            .get("location")
            .unwrap()
            .to_str()
            .unwrap();
        assert_eq!(
            loc, expected_target,
            "Route {method} {source_path} redirected to {loc} instead of {expected_target}"
        );
    }
}

#[tokio::test]
async fn test_swagger_static_assets_and_spec_completeness() {
    let state = AppState::new(9697, "127.0.0.1".into(), Protocol::Http);
    let broadcaster = Arc::new(AudioBroadcaster::new());
    let config = ServerConfig::default();
    let app = build_app(state, broadcaster, &config);

    // Test vendored Swagger UI assets
    let css_res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/swagger-ui/swagger-ui.css")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(css_res.status(), StatusCode::OK);

    let js_res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/swagger-ui/swagger-ui-bundle.js")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(js_res.status(), StatusCode::OK);

    // Verify OpenAPI full spec paths
    let spec_res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/openapi.json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(spec_res.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(spec_res.into_body(), usize::MAX)
        .await
        .unwrap();
    let spec: serde_json::Value = serde_json::from_slice(&bytes).unwrap();

    let paths = spec["paths"]
        .as_object()
        .expect("paths should be an object");
    let required_paths = [
        "/health",
        "/api/v1/status",
        "/api/v1/toggle",
        "/api/v1/enable",
        "/api/v1/disable",
        "/api/v1/theme",
        "/api/v1/theme/reload",
        "/api/v1/qr.svg",
        "/api/v1/copy",
        "/api/v1/config/keep_streaming",
        "/ws",
    ];

    for path in required_paths {
        assert!(
            paths.contains_key(path),
            "OpenAPI spec is missing required path: {path}"
        );
    }
}
