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
}
