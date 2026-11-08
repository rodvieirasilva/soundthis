use axum::http::StatusCode;
use futures_util::{SinkExt, StreamExt};
use soundthis::audio::AudioBroadcaster;
use soundthis::cli::{Cli, Commands};
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
async fn test_e2e_cli_remote_control() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let port = addr.port();

    let state = AppState::new(port, "127.0.0.1".into(), Protocol::Http);
    let broadcaster = Arc::new(AudioBroadcaster::new());
    let config = ServerConfig {
        port,
        host: "127.0.0.1".into(),
        ..Default::default()
    };

    let app = build_app(state.clone(), broadcaster.clone(), &config);

    let server_handle = tokio::spawn(async move {
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
        .unwrap();
    });

    // Add a mock client so cmd_status displays connected listeners
    state
        .add_client(soundthis::state::ClientInfo {
            id: "127.0.0.1:9999".into(),
            name: "Living Room Speaker".into(),
            ip: "127.0.0.1".into(),
            connected_at: 100,
        })
        .await;

    // Test direct CLI helper functions
    assert!(soundthis::cli::cmd_status(&config).await.is_ok());
    assert!(soundthis::cli::cmd_set_enabled(&config, false)
        .await
        .is_ok());
    assert!(!state.is_enabled());

    assert!(soundthis::cli::cmd_toggle(&config).await.is_ok());
    assert!(state.is_enabled());

    assert!(soundthis::cli::cmd_set_enabled(&config, true).await.is_ok());
    assert!(state.is_enabled());

    assert!(soundthis::cli::cmd_open(&config).await.is_ok());

    // Write a temporary config file pointing to this server's port
    let temp_dir = std::env::temp_dir().join(format!("soundthis_cli_test_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&temp_dir);
    let temp_cfg = temp_dir.join("test_config.toml");
    let cfg_content = format!(
        "port = {}\nhost = \"127.0.0.1\"\nprotocol = \"http\"\n",
        port
    );
    std::fs::write(&temp_cfg, cfg_content).unwrap();

    // Test run_with_cli with different subcommands
    let cli_status = Cli {
        config: Some(temp_cfg.clone()),
        command: Some(Commands::Status),
    };
    assert!(soundthis::cli::run_with_cli(cli_status).await.is_ok());

    let cli_toggle = Cli {
        config: Some(temp_cfg.clone()),
        command: Some(Commands::Toggle),
    };
    assert!(soundthis::cli::run_with_cli(cli_toggle).await.is_ok());

    let cli_enable = Cli {
        config: Some(temp_cfg.clone()),
        command: Some(Commands::Enable),
    };
    assert!(soundthis::cli::run_with_cli(cli_enable).await.is_ok());

    let cli_disable = Cli {
        config: Some(temp_cfg.clone()),
        command: Some(Commands::Disable),
    };
    assert!(soundthis::cli::run_with_cli(cli_disable).await.is_ok());

    let cli_open = Cli {
        config: Some(temp_cfg.clone()),
        command: Some(Commands::Open),
    };
    assert!(soundthis::cli::run_with_cli(cli_open).await.is_ok());

    let _ = std::fs::remove_dir_all(&temp_dir);
    server_handle.abort();
}

#[tokio::test]
async fn test_e2e_server_run_server_http_and_errors() {
    let invalid_cfg = ServerConfig {
        host: "invalid.host.address.that.fails".into(),
        ..Default::default()
    };

    let state = AppState::new(9697, "127.0.0.1".into(), Protocol::Http);
    let broadcaster = Arc::new(AudioBroadcaster::new());

    // Should fail with Network error on invalid bind address
    let res = soundthis::server::run_server(state.clone(), broadcaster.clone(), &invalid_cfg).await;
    assert!(res.is_err());

    // Valid HTTP run_server on dynamic free port
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener); // Free the port for run_server to bind

    let valid_cfg = ServerConfig {
        port,
        host: "127.0.0.1".into(),
        ..Default::default()
    };

    let valid_state = AppState::new(port, "127.0.0.1".into(), Protocol::Http);
    let srv_cfg = valid_cfg.clone();
    let srv_broadcaster = broadcaster.clone();
    let srv_state = valid_state.clone();

    let handle = tokio::spawn(async move {
        let _ = soundthis::server::run_server(srv_state, srv_broadcaster, &srv_cfg).await;
    });

    // Wait briefly for server startup
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let client = reqwest::Client::new();
    let resp = client
        .get(format!("http://127.0.0.1:{port}/health"))
        .send()
        .await;

    assert!(resp.is_ok());
    assert_eq!(resp.unwrap().status(), StatusCode::OK);

    handle.abort();
}

#[tokio::test]
async fn test_e2e_server_run_server_https() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);

    let temp_dir =
        std::env::temp_dir().join(format!("soundthis_https_test_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&temp_dir);

    let https_cfg = ServerConfig {
        port,
        host: "127.0.0.1".into(),
        protocol: Protocol::Https,
        tls: soundthis::config::TlsConfig {
            auto_generate: true,
            cert_path: Some(temp_dir.join("test_cert.pem")),
            key_path: Some(temp_dir.join("test_key.pem")),
        },
        ..Default::default()
    };

    let state = AppState::new(port, "127.0.0.1".into(), Protocol::Https);
    let broadcaster = Arc::new(AudioBroadcaster::new());

    let srv_cfg = https_cfg.clone();
    let srv_broadcaster = broadcaster.clone();
    let srv_state = state.clone();

    let handle = tokio::spawn(async move {
        let _ = soundthis::server::run_server(srv_state, srv_broadcaster, &srv_cfg).await;
    });

    // Wait briefly for TLS cert generation and bind
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    let client = reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .build()
        .unwrap();

    let resp = client
        .get(format!("https://127.0.0.1:{port}/health"))
        .send()
        .await;

    assert!(resp.is_ok());
    assert_eq!(resp.unwrap().status(), StatusCode::OK);

    handle.abort();
    let _ = std::fs::remove_dir_all(&temp_dir);
}
