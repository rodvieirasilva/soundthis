use axum::{
    body::Body,
    http::{Request, StatusCode},
    routing::{get, post},
    Router,
};
use futures_util::{SinkExt, StreamExt};
use soundthis::audio::AudioBroadcaster;
use soundthis::state::AppState;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;
use tower::ServiceExt;
use tower_http::cors::{Any, CorsLayer};

#[tokio::test]
async fn test_template_and_asset_validations() {
    // Verify external template validation
    assert!(soundthis::installer::Installer::validate_templates().is_ok());

    // Verify external asset validation
    assert!(soundthis::server::http::validate_assets().is_ok());
}

#[tokio::test]
async fn test_app_state_management() {
    let state = AppState::new(9697, "192.168.1.100".into());
    assert!(state.is_enabled());

    let toggled = state.toggle_enabled();
    assert!(!toggled);
    assert!(!state.is_enabled());

    state.set_enabled(true);
    assert!(state.is_enabled());

    let client_info = soundthis::state::ClientInfo {
        id: "127.0.0.1:1234".into(),
        name: "Test Browser".into(),
        ip: "127.0.0.1".into(),
        connected_at: 1000,
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

    state.remove_client("127.0.0.1:1234").await;
    assert_eq!(state.get_clients().await.len(), 0);
}

#[tokio::test]
async fn test_http_api_routes() {
    let state = AppState::new(9697, "127.0.0.1".into());
    let broadcaster = Arc::new(AudioBroadcaster::new());
    let state_with_broadcaster = (state.clone(), broadcaster);

    let cors = CorsLayer::new().allow_origin(Any);

    let app = Router::new()
        .route("/", get(soundthis::server::http::handle_index))
        .route("/favicon.svg", get(soundthis::server::http::handle_favicon_svg))
        .route("/favicon.ico", get(soundthis::server::http::handle_favicon_ico))
        .route("/health", get(soundthis::server::http::handle_health))
        .route("/api/status", get(soundthis::server::http::handle_api_status))
        .route("/api/toggle", post(soundthis::server::http::handle_api_toggle))
        .route("/api/enable", post(soundthis::server::http::handle_api_enable))
        .route("/api/disable", post(soundthis::server::http::handle_api_disable))
        .with_state(state.clone())
        .route("/ws", get(soundthis::server::ws::handle_ws))
        .with_state(state_with_broadcaster)
        .layer(cors);

    // Test /health
    let response = app
        .clone()
        .oneshot(Request::builder().uri("/health").body(Body::empty()).unwrap())
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
        .oneshot(Request::builder().uri("/favicon.svg").body(Body::empty()).unwrap())
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
        .oneshot(Request::builder().uri("/favicon.ico").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get("content-type").unwrap(),
        "image/x-icon"
    );

    // Test /api/status
    let response = app
        .clone()
        .oneshot(Request::builder().uri("/api/status").body(Body::empty()).unwrap())
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
async fn test_websocket_streaming_and_client_lifecycle() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let state = AppState::new(addr.port(), "127.0.0.1".into());
    let broadcaster = Arc::new(AudioBroadcaster::new());
    let state_with_broadcaster = (state.clone(), broadcaster.clone());

    let app = Router::new()
        .route("/ws", get(soundthis::server::ws::handle_ws))
        .with_state(state_with_broadcaster);

    tokio::spawn(async move {
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
        .unwrap();
    });

    let ws_url = format!("ws://{addr}/ws");
    let (mut ws_stream, _) = connect_async(ws_url).await.unwrap();

    // Give server a moment to register connection
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    let clients = state.get_clients().await;
    assert_eq!(clients.len(), 1);

    // Send identify message
    let identify_payload = serde_json::json!({
        "type": "identify",
        "name": "Living Room Phone"
    });
    ws_stream
        .send(Message::Text(identify_payload.to_string().into()))
        .await
        .unwrap();

    // Verify name update in server state
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    let clients = state.get_clients().await;
    assert_eq!(clients.len(), 1);
    assert_eq!(clients[0].name, "Living Room Phone");

    // Broadcast audio chunk and verify client receives binary frame
    let fake_pcm = vec![12u8, 34u8, 56u8, 78u8];
    broadcaster.broadcast_chunk(Arc::new(fake_pcm.clone()));

    // Wait for the binary message on websocket
    let msg = tokio::time::timeout(tokio::time::Duration::from_millis(500), ws_stream.next())
        .await
        .expect("Timeout waiting for audio message")
        .expect("Stream ended")
        .expect("WS error");

    match msg {
        Message::Binary(bin) => {
            assert_eq!(bin.as_ref(), fake_pcm.as_slice());
        }
        other => panic!("Expected Binary message, got: {:?}", other),
    }

    // Close websocket connection
    ws_stream.close(None).await.unwrap();

    // Verify cleanup
    tokio::time::sleep(tokio::time::Duration::from_millis(80)).await;
    let clients = state.get_clients().await;
    assert_eq!(clients.len(), 0);
}
