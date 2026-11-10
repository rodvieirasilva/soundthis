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

    // Test /api/status
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
    assert_eq!(response.status(), StatusCode::OK);

    // Test /api/disable
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/disable")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(!state.is_enabled());

    // Test /api/enable
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/enable")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(state.is_enabled());

    // Test /api/toggle
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
    assert_eq!(response.status(), StatusCode::OK);
    assert!(!state.is_enabled());
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
