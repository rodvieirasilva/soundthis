use clap::Parser;
use soundthis::cli::{self, Cli, Commands};
use soundthis::config::ServerConfig;

#[test]
fn test_cli_parsing() {
    let cli = Cli::parse_from(["soundthis", "status"]);
    assert!(matches!(cli.command, Some(Commands::Status)));

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
    if let Some(Commands::Serve { port, https, .. }) = cli2.command {
        assert_eq!(port, Some(1234));
        assert!(https);
    } else {
        panic!("Expected Serve subcommand");
    }

    let c1 = Cli::parse_from(["soundthis", "enable"]);
    assert!(matches!(c1.command, Some(Commands::Enable)));
    let c2 = Cli::parse_from(["soundthis", "disable"]);
    assert!(matches!(c2.command, Some(Commands::Disable)));
    let c3 = Cli::parse_from(["soundthis", "open"]);
    assert!(matches!(c3.command, Some(Commands::Open)));
    let c4 = Cli::parse_from(["soundthis", "toggle"]);
    assert!(matches!(c4.command, Some(Commands::Toggle)));
    let c5 = Cli::parse_from(["soundthis", "install"]);
    assert!(matches!(c5.command, Some(Commands::Install)));
    let c6 = Cli::parse_from(["soundthis", "uninstall"]);
    assert!(matches!(c6.command, Some(Commands::Uninstall)));
    let c7 = Cli::parse_from(["soundthis", "app"]);
    assert!(matches!(c7.command, Some(Commands::App)));
    let c8 = Cli::parse_from(["soundthis", "stop"]);
    assert!(matches!(c8.command, Some(Commands::Stop)));
}

#[tokio::test]
async fn test_cli_commands_offline_error_handling() {
    let offline_config = ServerConfig {
        port: 59123,
        ..Default::default()
    };

    // cmd_status handles offline gracefully and prints offline status
    assert!(cli::cmd_status(&offline_config).await.is_ok());

    // cmd_toggle and cmd_set_enabled return error when server unreachable
    assert!(cli::cmd_toggle(&offline_config).await.is_err());
    assert!(cli::cmd_set_enabled(&offline_config, true).await.is_err());

    // cmd_stop succeeds gracefully even when server is offline
    assert!(cli::cmd_stop(&offline_config).await.is_ok());
}

#[tokio::test]
async fn test_cli_helpers_and_detection() {
    cli::init_logging();
    assert!(cli::which_command_exists("sh"));
    assert!(!cli::which_command_exists("nonexistent_command_123456789"));
    let _ = cli::has_gui_support();
    let _ = cli::has_gui_binary();

    // Check server running on random offline port
    assert!(!cli::check_server_running(59124).await);

    // Test cmd_open
    std::env::set_var("SOUNDTHIS_TEST", "1");
    let config = ServerConfig::default();
    assert!(cli::cmd_open(&config).await.is_ok());
}

#[tokio::test]
async fn test_check_server_running_online() {
    use axum::routing::get;
    let app = axum::Router::new().route("/health", get(|| async { "OK" }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let handle = tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });

    // Give server a moment to accept connections
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    assert!(cli::check_server_running(port).await);
    handle.abort();
}

#[tokio::test]
async fn test_cli_commands_online() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();

    let state = soundthis::state::AppState::new(
        port,
        "127.0.0.1".into(),
        soundthis::config::Protocol::Http,
    );
    state
        .add_client(soundthis::state::ClientInfo {
            id: "test:1".into(),
            name: "TestDevice".into(),
            ip: "127.0.0.1".into(),
            connected_at: 100,
            listening: true,
        })
        .await;
    state
        .add_client(soundthis::state::ClientInfo {
            id: "test:2".into(),
            name: "PausedDevice".into(),
            ip: "127.0.0.2".into(),
            connected_at: 200,
            listening: false,
        })
        .await;
    let broadcaster = std::sync::Arc::new(soundthis::audio::AudioBroadcaster::new());
    let config = ServerConfig {
        port,
        host: "127.0.0.1".into(),
        ..Default::default()
    };
    let app = soundthis::server::build_app(state.clone(), broadcaster, &config);

    let handle = tokio::spawn(async move {
        let _ = axum::serve(
            listener,
            app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
        )
        .await;
    });

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Test cmd_status online
    assert!(cli::cmd_status(&config).await.is_ok());

    // Test cmd_toggle online
    assert!(cli::cmd_toggle(&config).await.is_ok());

    // Test cmd_set_enabled online
    assert!(cli::cmd_set_enabled(&config, true).await.is_ok());
    assert!(cli::cmd_set_enabled(&config, false).await.is_ok());

    // Test cmd_status when disabled
    assert!(cli::cmd_status(&config).await.is_ok());

    // Test run_with_cli online for subcommands
    let tmp_cfg =
        std::env::temp_dir().join(format!("soundthis_cli_online_{}.toml", std::process::id()));
    let _ = std::fs::write(&tmp_cfg, toml::to_string(&config).unwrap());

    assert!(cli::run_with_cli(Cli {
        config: Some(tmp_cfg.clone()),
        command: Some(Commands::Status),
    })
    .await
    .is_ok());

    assert!(cli::run_with_cli(Cli {
        config: Some(tmp_cfg.clone()),
        command: Some(Commands::Toggle),
    })
    .await
    .is_ok());

    assert!(cli::run_with_cli(Cli {
        config: Some(tmp_cfg.clone()),
        command: Some(Commands::Enable),
    })
    .await
    .is_ok());

    assert!(cli::run_with_cli(Cli {
        config: Some(tmp_cfg.clone()),
        command: Some(Commands::Disable),
    })
    .await
    .is_ok());

    let _ = std::fs::remove_file(tmp_cfg);

    handle.abort();
}

#[tokio::test]
async fn test_cmd_app_single_instance_detection() {
    let pid_file = soundthis::ui::get_gui_pid_file();
    // Write current process PID to simulate already-running GUI
    let _ = std::fs::write(&pid_file, std::process::id().to_string());

    let config = ServerConfig::default();
    // When GUI is already running, run_desktop_workflow should detect it and return Ok without spawning
    assert!(cli::run_desktop_workflow(&config).await.is_ok());

    let _ = std::fs::remove_file(pid_file);
}

#[tokio::test]
async fn test_run_with_cli_subcommands() {
    let offline_cfg = ServerConfig {
        port: 59125,
        ..Default::default()
    };
    let tmp_cfg =
        std::env::temp_dir().join(format!("soundthis_cli_run_{}.toml", std::process::id()));
    let _ = std::fs::write(&tmp_cfg, toml::to_string(&offline_cfg).unwrap());

    // Status subcommand via run_with_cli
    let cli_status = Cli {
        config: Some(tmp_cfg.clone()),
        command: Some(Commands::Status),
    };
    assert!(cli::run_with_cli(cli_status).await.is_ok());

    // Stop subcommand via run_with_cli
    let cli_stop = Cli {
        config: Some(tmp_cfg.clone()),
        command: Some(Commands::Stop),
    };
    assert!(cli::run_with_cli(cli_stop).await.is_ok());

    // Open subcommand via run_with_cli
    std::env::set_var("SOUNDTHIS_TEST", "1");
    let cli_open = Cli {
        config: Some(tmp_cfg.clone()),
        command: Some(Commands::Open),
    };
    assert!(cli::run_with_cli(cli_open).await.is_ok());

    // Install and Uninstall subcommands via run_with_cli in a temp home
    let temp_home =
        std::env::temp_dir().join(format!("soundthis_cli_install_run_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&temp_home);
    let orig_home = std::env::var("HOME").ok();
    std::env::set_var("HOME", &temp_home);

    let cli_install = Cli {
        config: Some(tmp_cfg.clone()),
        command: Some(Commands::Install),
    };
    assert!(cli::run_with_cli(cli_install).await.is_ok());

    let cli_uninstall = Cli {
        config: Some(tmp_cfg.clone()),
        command: Some(Commands::Uninstall),
    };
    assert!(cli::run_with_cli(cli_uninstall).await.is_ok());

    if let Some(h) = orig_home {
        std::env::set_var("HOME", h);
    }
    let _ = std::fs::remove_dir_all(&temp_home);

    let _ = std::fs::remove_file(tmp_cfg);
}
