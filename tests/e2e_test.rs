use axum::http::StatusCode;
use soundthis::audio::AudioBroadcaster;
use soundthis::cli::{Cli, Commands};
use soundthis::config::{Protocol, ServerConfig};
use soundthis::server::build_app;
use soundthis::state::AppState;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;

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
            listening: true,
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

    std::env::set_var("SOUNDTHIS_TEST", "1");
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

#[tokio::test]
async fn test_server_memory_and_resource_bounds() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let state = AppState::new(addr.port(), "127.0.0.1".into(), Protocol::Http);
    let broadcaster = Arc::new(AudioBroadcaster::new());
    let config = ServerConfig::default();

    let app = build_app(state.clone(), broadcaster.clone(), &config);

    let handle = tokio::spawn(async move {
        let _ = axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await;
    });

    // Feed multiple audio chunks through broadcaster
    let dummy_chunk = Arc::new(vec![0u8; 3840]);
    for _ in 0..100 {
        broadcaster.broadcast_chunk(dummy_chunk.clone());
    }

    // Verify /proc/self/status VmRSS is bounded
    if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
        for line in status.lines() {
            if line.starts_with("VmRSS:") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if let Some(kb_str) = parts.get(1) {
                    if let Ok(kb) = kb_str.parse::<u64>() {
                        assert!(
                            kb < 100_000,
                            "Server memory exceeded safety threshold: {kb} kB"
                        );
                    }
                }
            }
        }
    }

    handle.abort();
}
