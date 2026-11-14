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
    /// Launch the native Omarchy desktop control window
    App,
    /// Stop the running audio streaming server
    Stop,
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

pub fn has_gui_support() -> bool {
    which_command_exists("qml6")
}

pub fn has_gui_binary() -> bool {
    has_gui_support()
}

pub fn which_command_exists(cmd: &str) -> bool {
    Command::new("which")
        .arg(cmd)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

pub fn init_logging() {
    let _ = tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .try_init();
}

pub async fn check_server_running(port: u16) -> bool {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_millis(300))
        .danger_accept_invalid_certs(true)
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());

    if let Ok(r) = client
        .get(format!("http://127.0.0.1:{port}/health"))
        .send()
        .await
    {
        if r.status().is_success() {
            return true;
        }
    }

    if let Ok(r) = client
        .get(format!("https://127.0.0.1:{port}/health"))
        .send()
        .await
    {
        if r.status().is_success() {
            return true;
        }
    }

    false
}

pub async fn run_with_cli(cli: Cli) -> Result<(), Box<dyn std::error::Error>> {
    init_logging();
    let mut config = ServerConfig::load_or_create(cli.config.as_deref())?;

    let command = cli.command.unwrap_or_else(|| {
        let in_graphical_session =
            std::env::var("WAYLAND_DISPLAY").is_ok() || std::env::var("DISPLAY").is_ok();
        if in_graphical_session && has_gui_support() {
            Commands::App
        } else {
            Commands::Serve {
                port: None,
                host: None,
                https: false,
            }
        }
    });

    match command {
        Commands::Serve { port, host, https } => {
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
        Commands::App => {
            cmd_app(&config).await?;
        }
        Commands::Stop => {
            cmd_stop(&config).await?;
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

pub async fn cmd_app(config: &ServerConfig) -> Result<(), Box<dyn std::error::Error>> {
    run_desktop_workflow(config).await
}

pub async fn run_desktop_workflow(config: &ServerConfig) -> Result<(), Box<dyn std::error::Error>> {
    let already_running = check_server_running(config.port).await;
    let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel::<()>();

    let server_task = if !already_running {
        println!(
            "Starting SoundThis audio streaming server on port {}...",
            config.port
        );
        let lan_ip = get_lan_ip();
        let state = AppState::new(config.port, lan_ip, config.protocol);
        let broadcaster = Arc::new(AudioBroadcaster::new());
        let source = ParecAudioSource::new();
        let _ = broadcaster.start_broadcasting(source, state.clone());

        let cfg = config.clone();
        Some(tokio::spawn(async move {
            tokio::select! {
                res = server::run_server(state, broadcaster, &cfg) => {
                    if let Err(e) = res {
                        tracing::error!("Server error: {e}");
                    }
                }
                _ = &mut shutdown_rx => {
                    tracing::info!("Server task shutting down gracefully.");
                }
            }
        }))
    } else {
        println!(
            "Connected to existing SoundThis streaming server on port {}.",
            config.port
        );
        None
    };

    if !already_running {
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
    }

    if which_command_exists("qml6") {
        let _ = crate::ui::run_desktop_ui(config.port);
    } else {
        eprintln!("Notice: qml6 not found in PATH. Opening web control player instead.");
        let _ = cmd_open(config).await;
    }

    let keep = crate::ui::read_gui_config().keep_streaming;
    if let Some(task) = server_task {
        if !keep {
            println!("SoundThis UI closed. Stopped server.");
            let _ = shutdown_tx.send(());
            task.abort();
        } else {
            println!("SoundThis UI closed. Streaming continues in background (Omarchy bar).");
            println!("Press Ctrl+C to stop streaming server.");
            let _ = tokio::signal::ctrl_c().await;
            println!("Shutting down SoundThis server...");
            let _ = shutdown_tx.send(());
            task.abort();
        }
    }

    Ok(())
}

pub async fn cmd_stop(config: &ServerConfig) -> Result<(), Box<dyn std::error::Error>> {
    // 1. Try to stop systemd user service if running
    let _ = Command::new("systemctl")
        .args(["--user", "stop", "soundthis.service"])
        .status();

    // 2. Try disabling active streaming via HTTP API
    let scheme = config.protocol.scheme();
    let client = make_http_client();
    let _ = client
        .post(format!("{scheme}://127.0.0.1:{}/api/disable", config.port))
        .send()
        .await;

    println!("SoundThis audio streaming service stopped.");
    Ok(())
}
