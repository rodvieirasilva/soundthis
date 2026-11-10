use crate::audio::{AudioBroadcaster, ParecAudioSource};
use crate::config::{Protocol, ServerConfig};
use crate::installer::Installer;
use crate::network::ip::get_lan_ip;
use crate::server;
use crate::state::AppState;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Parser, Debug)]
#[command(name = "soundthis")]
#[command(author = "rvs")]
#[command(version)]
#[command(about = "Real-time desktop audio streaming server for Omarchy", long_about = None)]
pub struct Cli {
    /// Path to custom config.toml
    #[arg(short, long, global = true)]
    pub config: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug, Clone)]
pub enum Commands {
    /// Start the audio streaming server
    Serve {
        #[arg(short, long)]
        port: Option<u16>,

        #[arg(long)]
        host: Option<String>,

        #[arg(long)]
        https: bool,
    },
    /// Check server status and connected listeners
    Status,
    /// Toggle audio streaming on/off
    Toggle,
    /// Enable audio streaming
    Enable,
    /// Disable audio streaming
    Disable,
    /// Open the web player in the default browser
    Open,
    /// Install systemd user service and Omarchy shell plugin
    Install,
    /// Uninstall systemd user service and Omarchy shell plugin
    Uninstall,
}

pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    run_with_cli(cli).await
}

pub async fn run_with_cli(cli: Cli) -> Result<(), Box<dyn std::error::Error>> {
    let mut config = ServerConfig::load_or_create(cli.config.as_deref())?;

    match cli.command.unwrap_or(Commands::Serve {
        port: None,
        host: None,
        https: false,
    }) {
        Commands::Serve { port, host, https } => {
            tracing_subscriber::registry()
                .with(
                    tracing_subscriber::EnvFilter::try_from_default_env()
                        .unwrap_or_else(|_| "info".into()),
                )
                .with(tracing_subscriber::fmt::layer())
                .init();

            if let Some(p) = port {
                config.port = p;
            }
            if let Some(h) = host {
                config.host = h;
            }
            if https {
                config.protocol = Protocol::Https;
            }

            let lan_ip = get_lan_ip();
            let state = AppState::new(config.port, lan_ip, config.protocol);
            let broadcaster = Arc::new(AudioBroadcaster::new());

            let source = ParecAudioSource::new();
            broadcaster.start_broadcasting(source, state.clone())?;

            server::run_server(state, broadcaster, &config).await?;
        }
        Commands::Status => {
            cmd_status(&config).await?;
        }
        Commands::Toggle => {
            cmd_toggle(&config).await?;
        }
        Commands::Enable => {
            cmd_set_enabled(&config, true).await?;
        }
        Commands::Disable => {
            cmd_set_enabled(&config, false).await?;
        }
        Commands::Open => {
            cmd_open(&config).await?;
        }
        Commands::Install => {
            Installer::install()?;
        }
        Commands::Uninstall => {
            Installer::uninstall()?;
        }
    }

    Ok(())
}

fn make_http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
}

pub async fn cmd_status(config: &ServerConfig) -> Result<(), Box<dyn std::error::Error>> {
    let scheme = config.protocol.scheme();
    let client = make_http_client();
    let res = client
        .get(format!("{scheme}://127.0.0.1:{}/api/status", config.port))
        .send()
        .await;

    match res {
        Ok(resp) if resp.status().is_success() => {
            let data: serde_json::Value = resp.json().await?;
            let enabled = data["enabled"].as_bool().unwrap_or(false);
            let lan_ip = data["lan_ip"].as_str().unwrap_or("127.0.0.1");
            let url = data["url"]
                .as_str()
                .map(|u| u.to_string())
                .unwrap_or_else(|| format!("{scheme}://{}:{}", lan_ip, config.port));
            let clients = data["clients"].as_array();
            let client_count = clients.map(|c| c.len()).unwrap_or(0);

            println!("SoundThis Server Status:");
            println!(
                "  State:     {}",
                if enabled {
                    "● Live Broadcasting"
                } else {
                    "⏸ Paused / Disabled"
                }
            );
            println!("  Web UI:    {}", url);
            println!("  Protocol:  {}", scheme.to_uppercase());
            println!("  Listeners: {}", client_count);

            if let Some(list) = clients {
                for c in list {
                    let name = c["name"].as_str().unwrap_or("Device");
                    let ip = c["ip"].as_str().unwrap_or("unknown");
                    let listening = c["listening"].as_bool().unwrap_or(true);
                    let state_str = if listening { "Listening" } else { "Paused" };
                    println!("    - {} ({}) [{}]", name, ip, state_str);
                }
            }
        }
        _ => {
            println!("SoundThis Server: ○ Offline");
            println!("  Run `soundthis serve` or `systemctl --user start soundthis` to start.");
        }
    }
    Ok(())
}

pub async fn cmd_toggle(config: &ServerConfig) -> Result<(), Box<dyn std::error::Error>> {
    let scheme = config.protocol.scheme();
    let client = make_http_client();
    let res = client
        .post(format!("{scheme}://127.0.0.1:{}/api/toggle", config.port))
        .send()
        .await;

    match res {
        Ok(resp) if resp.status().is_success() => {
            let data: serde_json::Value = resp.json().await?;
            let enabled = data["enabled"].as_bool().unwrap_or(false);
            println!(
                "SoundThis is now {}",
                if enabled {
                    "ENABLED (Broadcasting)"
                } else {
                    "DISABLED (Muted)"
                }
            );
        }
        _ => {
            eprintln!(
                "Error: SoundThis server is not running on port {}.",
                config.port
            );
            return Err(format!("SoundThis server is not running on port {}.", config.port).into());
        }
    }
    Ok(())
}

pub async fn cmd_set_enabled(
    config: &ServerConfig,
    enable: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let endpoint = if enable { "enable" } else { "disable" };
    let scheme = config.protocol.scheme();
    let client = make_http_client();
    let res = client
        .post(format!(
            "{scheme}://127.0.0.1:{}/api/{endpoint}",
            config.port
        ))
        .send()
        .await;

    match res {
        Ok(resp) if resp.status().is_success() => {
            println!(
                "SoundThis is now {}",
                if enable {
                    "ENABLED (Broadcasting)"
                } else {
                    "DISABLED (Muted)"
                }
            );
        }
        _ => {
            eprintln!(
                "Error: SoundThis server is not running on port {}.",
                config.port
            );
            return Err(format!("SoundThis server is not running on port {}.", config.port).into());
        }
    }
    Ok(())
}

pub async fn cmd_open(config: &ServerConfig) -> Result<(), Box<dyn std::error::Error>> {
    let lan_ip = get_lan_ip();
    let scheme = config.protocol.scheme();
    let url = format!("{scheme}://{}:{}", lan_ip, config.port);
    println!("Opening {}", url);
    let _ = Command::new("xdg-open").arg(url).status();
    Ok(())
}
