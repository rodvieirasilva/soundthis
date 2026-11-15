use futures_util::{SinkExt, StreamExt};
use soundthis::audio::AudioBroadcaster;
use soundthis::config::{Protocol, ServerConfig};
use soundthis::server::build_app;
use soundthis::state::AppState;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

#[tokio::test]
async fn test_e2e_websocket_streaming_and_client_lifecycle() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let state = AppState::new(addr.port(), "127.0.0.1".into(), Protocol::Http);
    let broadcaster = Arc::new(AudioBroadcaster::new());
    let config = ServerConfig::default();

    let app = build_app(state.clone(), broadcaster.clone(), &config);

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

    // First message received is the theme sync message
    let first = tokio::time::timeout(tokio::time::Duration::from_millis(500), ws_stream.next())
        .await
        .expect("Timeout waiting for theme message")
        .expect("Stream ended")
        .expect("WS error");
    match first {
        Message::Text(txt) => {
            assert!(txt.contains("\"type\":\"theme\""));
        }
        other => panic!("Expected Text theme message, got {:?}", other),
    }

    // Broadcast audio chunk and verify client receives binary frame
    let fake_pcm = vec![12u8, 34u8, 56u8, 78u8];
    broadcaster.broadcast_chunk(Arc::new(fake_pcm.clone()));

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

#[tokio::test]
async fn test_e2e_websocket_paused_client_does_not_receive_binary_audio() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let state = AppState::new(addr.port(), "127.0.0.1".into(), Protocol::Http);
    let broadcaster = Arc::new(AudioBroadcaster::new());
    let config = ServerConfig::default();

    let app = build_app(state.clone(), broadcaster.clone(), &config);

    tokio::spawn(async move {
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
        .unwrap();
    });

    // Connect with ?listen=0 (paused by default, like web browser opening)
    let ws_url = format!("ws://{addr}/ws?listen=0");
    let (mut ws_stream, _) = connect_async(ws_url).await.unwrap();

    // Consume initial theme sync message
    let first = tokio::time::timeout(tokio::time::Duration::from_millis(500), ws_stream.next())
        .await
        .expect("Timeout waiting for theme message")
        .expect("Stream ended")
        .expect("WS error");
    assert!(matches!(first, Message::Text(_)));

    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    let clients = state.get_clients().await;
    assert_eq!(clients.len(), 1);
    assert!(!clients[0].listening);

    // Broadcast audio chunk: paused client MUST NOT receive binary frame
    let fake_pcm = vec![11u8, 22u8, 33u8, 44u8];
    broadcaster.broadcast_chunk(Arc::new(fake_pcm.clone()));

    let res = tokio::time::timeout(tokio::time::Duration::from_millis(150), ws_stream.next()).await;
    assert!(
        res.is_err(),
        "Expected timeout: paused client should not receive binary audio"
    );

    // Client resumes listening
    let resume_payload = serde_json::json!({
        "type": "playback",
        "playing": true
    });
    ws_stream
        .send(Message::Text(resume_payload.to_string().into()))
        .await
        .unwrap();

    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    let clients = state.get_clients().await;
    assert!(clients[0].listening);

    // Broadcast audio chunk: active client DOES receive binary frame
    broadcaster.broadcast_chunk(Arc::new(fake_pcm.clone()));
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

    // Client pauses listening
    let pause_payload = serde_json::json!({
        "type": "playback",
        "playing": false
    });
    ws_stream
        .send(Message::Text(pause_payload.to_string().into()))
        .await
        .unwrap();

    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    let clients = state.get_clients().await;
    assert!(!clients[0].listening);

    // Broadcast audio chunk: paused client MUST NOT receive binary frame
    broadcaster.broadcast_chunk(Arc::new(fake_pcm.clone()));
    let res = tokio::time::timeout(tokio::time::Duration::from_millis(150), ws_stream.next()).await;
    assert!(
        res.is_err(),
        "Expected timeout: paused client should not receive binary audio"
    );

    ws_stream.close(None).await.unwrap();
}

#[tokio::test]
async fn test_e2e_stream_enable_disable_lifecycle() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let state = AppState::new(addr.port(), "127.0.0.1".into(), Protocol::Http);
    let broadcaster = Arc::new(AudioBroadcaster::new());
    let config = ServerConfig::default();

    let app = build_app(state.clone(), broadcaster.clone(), &config);

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
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    // Consume initial theme packet
    let theme_msg = tokio::time::timeout(tokio::time::Duration::from_millis(500), ws_stream.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert!(matches!(theme_msg, Message::Text(_)));

    // 1. When enabled, broadcast chunk is received
    assert!(state.is_enabled());
    let chunk1 = vec![1u8, 2u8, 3u8];
    broadcaster.broadcast_chunk(Arc::new(chunk1.clone()));

    let msg1 = tokio::time::timeout(tokio::time::Duration::from_millis(300), ws_stream.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    if let Message::Binary(bin) = msg1 {
        assert_eq!(bin.as_ref(), chunk1.as_slice());
    } else {
        panic!("Expected binary frame");
    }

    // 2. When disabled, broadcast chunk is NOT sent to client
    state.set_enabled(false);
    let chunk2 = vec![4u8, 5u8, 6u8];
    broadcaster.broadcast_chunk(Arc::new(chunk2));

    let timeout_res =
        tokio::time::timeout(tokio::time::Duration::from_millis(150), ws_stream.next()).await;
    assert!(
        timeout_res.is_err(),
        "Expected timeout since streaming is disabled"
    );

    // 3. When re-enabled, streaming resumes
    state.set_enabled(true);
    let chunk3 = vec![7u8, 8u8, 9u8];
    broadcaster.broadcast_chunk(Arc::new(chunk3.clone()));

    let msg3 = tokio::time::timeout(tokio::time::Duration::from_millis(300), ws_stream.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    if let Message::Binary(bin) = msg3 {
        assert_eq!(bin.as_ref(), chunk3.as_slice());
    } else {
        panic!("Expected binary frame");
    }

    ws_stream.close(None).await.unwrap();
}

#[tokio::test]
async fn test_e2e_websocket_live_theme_broadcast() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let state = AppState::new(addr.port(), "127.0.0.1".into(), Protocol::Http);
    let broadcaster = Arc::new(AudioBroadcaster::new());
    let config = ServerConfig::default();

    let app = build_app(state.clone(), broadcaster.clone(), &config);

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

    // 1. Initial theme received on connection
    let first = tokio::time::timeout(tokio::time::Duration::from_millis(500), ws_stream.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    match first {
        Message::Text(txt) => assert!(txt.contains("\"type\":\"theme\"")),
        _ => panic!("Expected theme text packet"),
    }

    // 2. Trigger /api/theme/reload
    let client = reqwest::Client::new();
    let res = client
        .post(format!("http://{addr}/api/theme/reload"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), reqwest::StatusCode::OK);

    // 3. Client receives pushed theme update over WebSocket without page reload
    let update_msg =
        tokio::time::timeout(tokio::time::Duration::from_millis(500), ws_stream.next())
            .await
            .expect("Timeout waiting for theme update")
            .expect("Stream ended")
            .expect("WS error");

    match update_msg {
        Message::Text(txt) => {
            assert!(txt.contains("\"type\":\"theme\""));
            assert!(txt.contains("\"palette\""));
        }
        _ => panic!("Expected theme update packet"),
    }

    ws_stream.close(None).await.unwrap();
}
