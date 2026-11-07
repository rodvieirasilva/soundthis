use axum::{
    body::Body,
    http::{Request, StatusCode},
};
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
use tower::ServiceExt;

#[test]
fn test_config_defaults_and_parsing() {
    let default_cfg = ServerConfig::default();
    assert_eq!(default_cfg.port, 9697);
    assert_eq!(default_cfg.host, "0.0.0.0");
    assert_eq!(default_cfg.protocol, Protocol::Http);
    assert!(!default_cfg.protocol.is_https());
    assert_eq!(default_cfg.protocol.scheme(), "http");
    assert_eq!(default_cfg.protocol.ws_scheme(), "ws");
    assert!(default_cfg.tls.auto_generate);
    assert_eq!(default_cfg.cors.allow_origins, vec!["*"]);
    assert_eq!(default_cfg.audio.sample_rate, 48000);
    assert_eq!(default_cfg.audio.channels, 2);

    // Custom TOML parsing
    let custom_toml = r#"
        port = 8888
        host = "127.0.0.1"
        protocol = "https"

        [tls]
        auto_generate = false

        [cors]
        allow_origins = ["https://myapp.com"]
        allow_methods = ["GET", "POST"]
        allow_headers = ["Authorization", "Content-Type"]

        [audio]
        sample_rate = 44100
        channels = 1
        chunk_size = 1024
        latency_ms = 10
    "#;

    let parsed: ServerConfig = toml::from_str(custom_toml).unwrap();
    assert_eq!(parsed.port, 8888);
    assert_eq!(parsed.host, "127.0.0.1");
    assert_eq!(parsed.protocol, Protocol::Https);
    assert!(parsed.protocol.is_https());
    assert_eq!(parsed.protocol.scheme(), "https");
    assert_eq!(parsed.protocol.ws_scheme(), "wss");
    assert!(!parsed.tls.auto_generate);
    assert_eq!(parsed.cors.allow_origins, vec!["https://myapp.com"]);
    assert_eq!(parsed.audio.sample_rate, 44100);
    assert_eq!(parsed.audio.channels, 1);

    // Default template validity
    let template = ServerConfig::default_toml_template();
    let from_template: Result<ServerConfig, _> = toml::from_str(template);
    assert!(from_template.is_ok());
}

#[test]
fn test_tls_self_signed_cert_generation() {
    let temp_dir =
        std::env::temp_dir().join(format!("soundthis_test_certs_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&temp_dir);

    let config = ServerConfig {
        protocol: Protocol::Https,
        tls: soundthis::config::TlsConfig {
            auto_generate: true,
            cert_path: Some(temp_dir.join("test_cert.pem")),
            key_path: Some(temp_dir.join("test_key.pem")),
        },
        ..Default::default()
    };

    // Should generate valid certs
    let cert_result = config.resolve_tls_pem("192.168.1.42");
    assert!(cert_result.is_ok());
    let (cert_pem, key_pem) = cert_result.unwrap();

    let cert_str = String::from_utf8(cert_pem).unwrap();
    let key_str = String::from_utf8(key_pem).unwrap();

    assert!(cert_str.contains("BEGIN CERTIFICATE"));
    assert!(key_str.contains("BEGIN PRIVATE KEY"));

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_installer_install_and_uninstall_lifecycle() {
    let temp_dir =
        std::env::temp_dir().join(format!("soundthis_install_test_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    let _ = std::fs::create_dir_all(&temp_dir);

    let shell_dir = temp_dir.join(".config/omarchy");
    let _ = std::fs::create_dir_all(&shell_dir);
    let initial_shell_json = serde_json::json!({
        "bar": {
            "layout": {
                "right": ["clock", "battery"]
            }
        }
    });
    std::fs::write(
        shell_dir.join("shell.json"),
        serde_json::to_string(&initial_shell_json).unwrap(),
    )
    .unwrap();

    let fake_bin = temp_dir.join("bin/soundthis");

    // Test install_to
    assert!(soundthis::installer::Installer::install_to(&temp_dir, &fake_bin, false).is_ok());

    let service_file = temp_dir.join(".config/systemd/user/soundthis.service");
    assert!(service_file.exists());
    let service_content = std::fs::read_to_string(&service_file).unwrap();
    assert!(service_content.contains(&fake_bin.to_string_lossy().to_string()));

    let manifest_file = temp_dir.join(".config/omarchy/plugins/soundthis/manifest.json");
    assert!(manifest_file.exists());

    let panel_file = temp_dir.join(".config/omarchy/plugins/soundthis/Panel.qml");
    assert!(panel_file.exists());

    let updated_shell: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(shell_dir.join("shell.json")).unwrap())
            .unwrap();
    let right_list = updated_shell["bar"]["layout"]["right"].as_array().unwrap();
    assert!(right_list.contains(&serde_json::Value::String("soundthis".into())));

    // Running install_to again should be idempotent
    assert!(soundthis::installer::Installer::install_to(&temp_dir, &fake_bin, false).is_ok());

    // Test uninstall_from
    assert!(soundthis::installer::Installer::uninstall_from(&temp_dir, false).is_ok());
    assert!(!service_file.exists());
    assert!(!manifest_file.exists());

    let uninstalled_shell: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(shell_dir.join("shell.json")).unwrap())
            .unwrap();
    let right_list = uninstalled_shell["bar"]["layout"]["right"]
        .as_array()
        .unwrap();
    assert!(!right_list.contains(&serde_json::Value::String("soundthis".into())));

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_config_load_or_create_file() {
    let temp_dir = std::env::temp_dir().join(format!("soundthis_cfg_test_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&temp_dir);
    let cfg_path = temp_dir.join("config.toml");

    // File doesn't exist -> returns default
    let cfg = ServerConfig::load_or_create(Some(&cfg_path)).unwrap();
    assert_eq!(cfg.port, 9697);

    // Write corrupted TOML
    std::fs::write(&cfg_path, "invalid [[ toml syntax").unwrap();
    let err = ServerConfig::load_or_create(Some(&cfg_path));
    assert!(err.is_err());

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_cli_parsing() {
    use clap::Parser;
    use soundthis::cli::Cli;

    let cli = Cli::parse_from(["soundthis", "status"]);
    assert!(matches!(
        cli.command,
        Some(soundthis::cli::Commands::Status)
    ));

    let cli2 = Cli::parse_from([
        "soundthis",
        "--config",
        "/tmp/cfg.toml",
        "serve",
        "--port",
        "1234",
        "--https",
    ]);
    assert_eq!(cli2.config, Some(std::path::PathBuf::from("/tmp/cfg.toml")));
    if let Some(soundthis::cli::Commands::Serve { port, https, .. }) = cli2.command {
        assert_eq!(port, Some(1234));
        assert!(https);
    } else {
        panic!("Expected Serve subcommand");
    }
}

#[test]
fn test_template_and_asset_validations() {
    // Verify external template validation
    assert!(soundthis::installer::Installer::validate_templates().is_ok());

    // Verify external asset validation
    assert!(soundthis::server::http::validate_assets().is_ok());
}

#[tokio::test]
async fn test_app_state_management() {
    let state = AppState::new(9697, "192.168.1.100".into(), Protocol::Http);
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

    let status = state.to_status_response().await;
    assert_eq!(status.url, "http://192.168.1.100:9697");
    assert_eq!(status.protocol, "http");

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

#[tokio::test]
async fn test_websocket_streaming_and_client_lifecycle() {
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
async fn test_cli_commands_and_run_with_cli() {
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
    use soundthis::cli::{Cli, Commands};

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
async fn test_cli_commands_offline_error_handling() {
    let offline_config = ServerConfig {
        port: 59123,
        ..Default::default()
    };

    // cmd_status handles offline gracefully and prints offline status
    assert!(soundthis::cli::cmd_status(&offline_config).await.is_ok());

    // cmd_toggle and cmd_set_enabled return error when server unreachable
    assert!(soundthis::cli::cmd_toggle(&offline_config).await.is_err());
    assert!(soundthis::cli::cmd_set_enabled(&offline_config, true)
        .await
        .is_err());
}

#[tokio::test]
async fn test_server_run_server_http_and_errors() {
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
async fn test_server_run_server_https() {
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

#[test]
fn test_config_tls_existing_files_and_missing_no_auto() {
    let temp_dir =
        std::env::temp_dir().join(format!("soundthis_tls_branch_test_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&temp_dir);

    let cert_file = temp_dir.join("cert.pem");
    let key_file = temp_dir.join("key.pem");
    std::fs::write(&cert_file, b"FAKE CERTIFICATE PEM DATA").unwrap();
    std::fs::write(&key_file, b"FAKE PRIVATE KEY PEM DATA").unwrap();

    let mut cfg = ServerConfig::default();
    cfg.tls.cert_path = Some(cert_file.clone());
    cfg.tls.key_path = Some(key_file.clone());
    cfg.tls.auto_generate = false;

    // Should read pre-existing cert and key
    let (cert, key) = cfg.resolve_tls_pem("127.0.0.1").unwrap();
    assert_eq!(cert, b"FAKE CERTIFICATE PEM DATA");
    assert_eq!(key, b"FAKE PRIVATE KEY PEM DATA");

    // When files do not exist and auto_generate is false
    let missing_cert = temp_dir.join("nonexistent_cert.pem");
    let missing_key = temp_dir.join("nonexistent_key.pem");
    cfg.tls.cert_path = Some(missing_cert);
    cfg.tls.key_path = Some(missing_key);

    let err = cfg.resolve_tls_pem("127.0.0.1");
    assert!(err.is_err());

    // Config directories
    assert!(ServerConfig::config_dir()
        .to_string_lossy()
        .contains(".config/soundthis"));
    assert!(ServerConfig::default_config_path()
        .to_string_lossy()
        .contains("config.toml"));
    assert!(ServerConfig::certs_dir()
        .to_string_lossy()
        .contains("certs"));

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_installer_helpers_and_systemd() {
    let home = soundthis::installer::Installer::home_dir();
    assert!(!home.as_os_str().is_empty());

    let bin = soundthis::installer::Installer::resolve_bin_path();
    assert!(!bin.as_os_str().is_empty());

    // Systemd daemon-reload & stop on linux system
    let reload = soundthis::installer::systemd::SystemdManager::daemon_reload();
    assert!(reload.is_ok());

    let stop = soundthis::installer::systemd::SystemdManager::stop_and_disable(
        "soundthis-dummy-test.service",
    );
    assert!(stop.is_ok());

    let start_err = soundthis::installer::systemd::SystemdManager::enable_and_start(
        "soundthis-dummy-test.service",
    );
    assert!(start_err.is_err());
}
