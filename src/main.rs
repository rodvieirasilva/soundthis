pub mod audio;
pub mod config;
pub mod error;
pub mod installer;
pub mod network;
pub mod server;
pub mod state;

use audio::{AudioBroadcaster, ParecAudioSource};
use clap::{Parser, Subcommand};
use config::{DEFAULT_HOST, DEFAULT_PORT};
use installer::Installer;
use network::ip::get_lan_ip;
use state::AppState;
use std::process::Command;
use std::sync::Arc;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Parser)]
#[command(name = "soundthis")]
#[command(author = "rvs")]
#[command(version = "0.2.0")]
#[command(about = "Real-time desktop audio streaming server for Omarchy", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the audio streaming server
    Serve {
        #[arg(short, long, default_value_t = DEFAULT_PORT)]
        port: u16,

        #[arg(long, default_value = DEFAULT_HOST)]
        host: String,
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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command.unwrap_or(Commands::Serve {
        port: DEFAULT_PORT,
        host: DEFAULT_HOST.to_string(),
    }) {
        Commands::Serve { port, host } => {
            tracing_subscriber::registry()
                .with(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
                .with(tracing_subscriber::fmt::layer())
                .init();

            let lan_ip = get_lan_ip();
            let state = AppState::new(port, lan_ip);
            let broadcaster = Arc::new(AudioBroadcaster::new());

            let source = ParecAudioSource::new();
            broadcaster.start_broadcasting(source, state.clone())?;

            server::run_server(state, broadcaster, &host, port).await?;
        }
        Commands::Status => {
            cmd_status().await?;
        }
        Commands::Toggle => {
            cmd_toggle().await?;
        }
        Commands::Enable => {
            cmd_set_enabled(true).await?;
        }
        Commands::Disable => {
            cmd_set_enabled(false).await?;
        }
        Commands::Open => {
            cmd_open().await?;
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

async fn cmd_status() -> Result<(), Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();
    let res = client
        .get(format!("http://127.0.0.1:{DEFAULT_PORT}/api/status"))
        .send()
        .await;

    match res {
        Ok(resp) if resp.status().is_success() => {
            let data: serde_json::Value = resp.json().await?;
            let enabled = data["enabled"].as_bool().unwrap_or(false);
            let lan_ip = data["lan_ip"].as_str().unwrap_or("127.0.0.1");
            let port = data["port"].as_u64().unwrap_or(DEFAULT_PORT as u64);
            let clients = data["clients"].as_array();
            let client_count = clients.map(|c| c.len()).unwrap_or(0);

            println!("SoundThis Server Status:");
            println!("  State:     {}", if enabled { "● Live Broadcasting" } else { "⏸ Paused / Disabled" });
            println!("  Web UI:    http://{}:{}", lan_ip, port);
            println!("  Listeners: {}", client_count);

            if let Some(list) = clients {
                for c in list {
                    let name = c["name"].as_str().unwrap_or("Device");
                    let ip = c["ip"].as_str().unwrap_or("unknown");
                    println!("    - {} ({})", name, ip);
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

async fn cmd_toggle() -> Result<(), Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();
    let res = client
        .post(format!("http://127.0.0.1:{DEFAULT_PORT}/api/toggle"))
        .send()
        .await;

    match res {
        Ok(resp) if resp.status().is_success() => {
            let data: serde_json::Value = resp.json().await?;
            let enabled = data["enabled"].as_bool().unwrap_or(false);
            println!(
                "SoundThis is now {}",
                if enabled { "ENABLED (Broadcasting)" } else { "DISABLED (Muted)" }
            );
        }
        _ => {
            eprintln!("Error: SoundThis server is not running.");
            std::process::exit(1);
        }
    }
    Ok(())
}

async fn cmd_set_enabled(enable: bool) -> Result<(), Box<dyn std::error::Error>> {
    let endpoint = if enable { "enable" } else { "disable" };
    let client = reqwest::Client::new();
    let res = client
        .post(format!("http://127.0.0.1:{DEFAULT_PORT}/api/{endpoint}"))
        .send()
        .await;

    match res {
        Ok(resp) if resp.status().is_success() => {
            println!(
                "SoundThis is now {}",
                if enable { "ENABLED (Broadcasting)" } else { "DISABLED (Muted)" }
            );
        }
        _ => {
            eprintln!("Error: SoundThis server is not running.");
            std::process::exit(1);
        }
    }
    Ok(())
}

async fn cmd_open() -> Result<(), Box<dyn std::error::Error>> {
    let lan_ip = get_lan_ip();
    let url = format!("http://{}:{DEFAULT_PORT}", lan_ip);
    println!("Opening {}", url);
    let _ = Command::new("xdg-open").arg(url).status();
    Ok(())
}
