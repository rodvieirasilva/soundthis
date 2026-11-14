use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct GuiConfig {
    #[serde(default)]
    pub keep_streaming: bool,
}

pub fn get_gui_config_path() -> PathBuf {
    crate::config::ServerConfig::default_config_path()
}

pub fn read_gui_config() -> GuiConfig {
    GuiConfig {
        keep_streaming: crate::config::ServerConfig::read_keep_streaming(),
    }
}

pub fn read_gui_config_from(path: &std::path::Path) -> GuiConfig {
    if let Ok(content) = std::fs::read_to_string(path) {
        if path.extension().and_then(|s| s.to_str()) == Some("toml") {
            if let Ok(cfg) = toml::from_str::<crate::config::ServerConfig>(&content) {
                return GuiConfig {
                    keep_streaming: cfg.desktop.keep_streaming,
                };
            }
        }
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        GuiConfig::default()
    }
}

pub fn save_gui_config(cfg: &GuiConfig) -> Result<(), std::io::Error> {
    crate::config::ServerConfig::set_keep_streaming(cfg.keep_streaming)
        .map_err(std::io::Error::other)
}

pub fn save_gui_config_to(path: &std::path::Path, cfg: &GuiConfig) -> Result<(), std::io::Error> {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if path.extension().and_then(|s| s.to_str()) == Some("toml") {
        let existing = std::fs::read_to_string(path).unwrap_or_default();
        let updated = crate::config::update_keep_streaming_in_toml(&existing, cfg.keep_streaming);
        std::fs::write(path, updated)
    } else {
        let json = serde_json::to_string_pretty(cfg).map_err(std::io::Error::other)?;
        std::fs::write(path, json)
    }
}

pub fn copy_to_clipboard(text: &str) -> bool {
    // 1. Try wl-copy (Wayland / Omarchy default)
    if let Ok(mut child) = Command::new("wl-copy").stdin(Stdio::piped()).spawn() {
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(text.as_bytes());
        }
        if let Ok(status) = child.wait() {
            if status.success() {
                return true;
            }
        }
    }

    // 2. Try xclip (X11 fallback)
    if let Ok(mut child) = Command::new("xclip")
        .args(["-selection", "clipboard"])
        .stdin(Stdio::piped())
        .spawn()
    {
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(text.as_bytes());
        }
        if let Ok(status) = child.wait() {
            if status.success() {
                return true;
            }
        }
    }

    false
}

pub fn get_gui_pid_file() -> PathBuf {
    if let Ok(runtime_dir) = std::env::var("XDG_RUNTIME_DIR") {
        PathBuf::from(runtime_dir).join("soundthis_gui.pid")
    } else {
        std::env::temp_dir().join("soundthis_gui.pid")
    }
}

pub fn is_gui_running() -> bool {
    is_gui_running_from(&get_gui_pid_file())
}

pub fn stop_gui_process() -> bool {
    stop_gui_process_from(&get_gui_pid_file())
}

pub fn stop_gui_process_from(pid_file: &std::path::Path) -> bool {
    let content = match std::fs::read_to_string(pid_file) {
        Ok(c) => c,
        Err(_) => return false,
    };
    let pid: u32 = match content.trim().parse() {
        Ok(p) => p,
        Err(_) => {
            let _ = std::fs::remove_file(pid_file);
            return false;
        }
    };

    if pid == std::process::id() {
        return false;
    }

    let _ = Command::new("kill").arg(pid.to_string()).status();
    let _ = std::fs::remove_file(pid_file);
    true
}

pub fn is_gui_running_from(pid_file: &std::path::Path) -> bool {
    let content = match std::fs::read_to_string(pid_file) {
        Ok(c) => c,
        Err(_) => return false,
    };
    let pid: u32 = match content.trim().parse() {
        Ok(p) => p,
        Err(_) => {
            let _ = std::fs::remove_file(pid_file);
            return false;
        }
    };

    let cmdline_path = format!("/proc/{pid}/cmdline");
    match std::fs::read_to_string(&cmdline_path) {
        Ok(cmdline) => {
            let matches = cmdline.contains("qml")
                || cmdline.contains("soundthis")
                || cmdline.contains("cargo")
                || cmdline.contains("test");
            if matches {
                true
            } else {
                let _ = std::fs::remove_file(pid_file);
                false
            }
        }
        Err(_) => {
            let _ = std::fs::remove_file(pid_file);
            false
        }
    }
}

pub fn focus_existing_window() -> bool {
    // 1. Try omarchy-hyprland-focus-app (native Omarchy helper)
    if let Ok(status) = Command::new("omarchy-hyprland-focus-app")
        .arg("SoundThis")
        .status()
    {
        if status.success() {
            return true;
        }
    }

    // 2. Try hyprctl dispatch hl.dsp.focus (Hyprland 0.56+ Lua API)
    if let Ok(status) = Command::new("hyprctl")
        .args(["dispatch", "hl.dsp.focus({ window = \"title:SoundThis\" })"])
        .status()
    {
        if status.success() {
            return true;
        }
    }

    // 3. Try hyprctl dispatch focuswindow (Legacy Hyprland dispatcher)
    if let Ok(status) = Command::new("hyprctl")
        .args(["dispatch", "focuswindow", "title:SoundThis"])
        .status()
    {
        if status.success() {
            return true;
        }
    }

    // 4. Try wmctrl (X11 / XWayland fallback)
    if let Ok(status) = Command::new("wmctrl")
        .args(["-F", "-a", "SoundThis"])
        .status()
    {
        if status.success() {
            return true;
        }
    }

    // 5. Try xdotool fallback
    if let Ok(status) = Command::new("xdotool")
        .args(["search", "--name", "^SoundThis$", "windowactivate"])
        .status()
    {
        if status.success() {
            return true;
        }
    }

    false
}

#[cfg(feature = "gui")]
pub mod gtk_app;

#[cfg(feature = "gui")]
pub fn run_desktop_ui(port: u16) -> Result<(), Box<dyn std::error::Error>> {
    gtk_app::run_gtk_app(port)
}

#[cfg(not(feature = "gui"))]
pub fn run_desktop_ui(_port: u16) -> Result<(), Box<dyn std::error::Error>> {
    eprintln!("Notice: SoundThis compiled without native GUI support.");
    Ok(())
}

pub async fn run_desktop_workflow(
    config: &crate::config::ServerConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    if is_gui_running() {
        println!("SoundThis desktop window is already open. Bringing window to focus.");
        focus_existing_window();
        return Ok(());
    }

    let mut already_running = crate::cli::check_server_running(config.port).await;
    let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel::<()>();
    let mut server_task = None;

    if !already_running {
        // Try starting native background systemd service first if available
        let systemd_started = Command::new("systemctl")
            .args(["--user", "start", "soundthis.service"])
            .status()
            .map(|s| s.success())
            .unwrap_or(false);

        if systemd_started {
            for _ in 0..10 {
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                if crate::cli::check_server_running(config.port).await {
                    already_running = true;
                    println!(
                        "Started SoundThis audio streaming server via systemd on port {}.",
                        config.port
                    );
                    break;
                }
            }
        }

        // If systemd service was not available or did not respond, run in-process server task
        if !already_running {
            println!(
                "Starting SoundThis audio streaming server on port {}...",
                config.port
            );
            let lan_ip = crate::network::ip::get_lan_ip();
            let state = crate::state::AppState::new(config.port, lan_ip, config.protocol);
            let broadcaster = std::sync::Arc::new(crate::audio::AudioBroadcaster::new());
            let source = crate::audio::ParecAudioSource::new();
            let _ = broadcaster.start_broadcasting(source, state.clone());

            let cfg = config.clone();
            server_task = Some(tokio::spawn(async move {
                tokio::select! {
                    res = crate::server::run_server(state, broadcaster, &cfg) => {
                        if let Err(e) = res {
                            tracing::error!("Server error: {e}");
                        }
                    }
                    _ = &mut shutdown_rx => {
                        tracing::info!("Server task shutting down gracefully.");
                    }
                }
            }));
            tokio::time::sleep(std::time::Duration::from_millis(150)).await;
        }
    } else {
        println!(
            "Connected to existing SoundThis streaming server on port {}.",
            config.port
        );
    }

    if crate::cli::has_gui_support() {
        let _ = run_desktop_ui(config.port);
    } else {
        eprintln!(
            "Notice: SoundThis compiled without GUI support. Opening web control player instead."
        );
        let _ = crate::cli::cmd_open(config).await;
    }

    let keep = read_gui_config().keep_streaming;
    if !keep {
        if let Some(task) = server_task {
            let _ = shutdown_tx.send(());
            task.abort();
        }
        let _ = crate::cli::cmd_stop(config).await;
        println!("SoundThis UI closed. Stopped server.");
    } else if let Some(task) = server_task {
        // Server was started in-process and keep is true.
        // Free port first before transferring to systemd if available:
        let _ = shutdown_tx.send(());
        task.abort();
        tokio::time::sleep(std::time::Duration::from_millis(80)).await;

        let systemd_started = Command::new("systemctl")
            .args(["--user", "start", "soundthis.service"])
            .status()
            .map(|s| s.success())
            .unwrap_or(false);

        if systemd_started && crate::cli::check_server_running(config.port).await {
            println!("SoundThis UI closed. Transferred streaming to background systemd service (Omarchy bar).");
        } else {
            // Keep running in-process if systemd was unavailable
            println!("SoundThis UI closed. Streaming continues in background (Omarchy bar).");
            println!("Press Ctrl+C to stop streaming server.");
            let lan_ip = crate::network::ip::get_lan_ip();
            let state = crate::state::AppState::new(config.port, lan_ip, config.protocol);
            let broadcaster = std::sync::Arc::new(crate::audio::AudioBroadcaster::new());
            let source = crate::audio::ParecAudioSource::new();
            let _ = broadcaster.start_broadcasting(source, state.clone());
            let cfg = config.clone();
            let s_task = tokio::spawn(async move {
                let _ = crate::server::run_server(state, broadcaster, &cfg).await;
            });
            let _ = tokio::signal::ctrl_c().await;
            println!("Shutting down SoundThis server...");
            s_task.abort();
        }
    } else {
        let _ = crate::cli::cmd_set_enabled(config, true).await;
        println!("SoundThis UI closed. Background streaming active in Omarchy bar.");
    }

    Ok(())
}
